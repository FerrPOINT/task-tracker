use sea_orm_migration::prelude::*;
use sea_orm::{DbBackend, Statement};

#[derive(DeriveMigrationName)]
pub struct Migration;

fn invalid(message: &str) -> DbErr {
    DbErr::Custom(format!("explicit delivery recovery: {message}"))
}

fn previous_guard() -> Result<String, DbErr> {
    // Both accepted lineages were normalized by119;120 does not alter this owner.
    let sql = include_str!("delivery_unsent_stop.sql").replace("\r\n", "\n");
    let (_, function) = sql.split_once("FUNCTION sdlc_delivery_reconciliation_gate()")
        .ok_or_else(|| invalid("published recovery guard absent"))?;
    let (_, body) = function.split_once("AS $$")
        .ok_or_else(|| invalid("published recovery delimiter changed"))?;
    let (body, _) = body.split_once("$$;")
        .ok_or_else(|| invalid("published recovery terminator changed"))?;
    let old = "BETWEEN 1 AND (head#>>'{lease,version}')::bigint";
    if body.matches(old).count() != 1 {
        return Err(invalid("published fence anchor changed"));
    }
    Ok(body.replacen(old, "= (head#>>'{lease,version}')::bigint", 1))
}

fn current_guard(actual: &str) -> Result<String, DbErr> {
    let expected = previous_guard()?;
    if actual.replace("\r\n", "\n") != expected {
        return Err(invalid("foreign recovery guard; no changes applied"));
    }
    let old = "IF NEW.payload#>>'{recovery,disposition}'='retry_unsubmitted' THEN";
    if expected.matches(old).count() != 1 {
        return Err(invalid("published recovery admission anchor changed"));
    }
    Ok(expected.replacen(old,
        "IF NEW.payload#>>'{recovery,disposition}' IS DISTINCT FROM 'stop' THEN", 1)
        .replace("SDLC ownership does not expire or retry automatically",
            "SDLC fresh reconciliation requires explicit Stop"))
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        let row = db.query_one(Statement::from_string(DbBackend::Postgres,
            "SELECT prosrc,pg_get_functiondef(oid) AS definition FROM pg_proc WHERE oid=to_regprocedure('public.sdlc_delivery_reconciliation_gate()')".to_owned()))
            .await?.ok_or_else(|| invalid("selected recovery guard absent"))?;
        let source: String = row.try_get("", "prosrc")?;
        let body = current_guard(&source)?;
        let definition: String = row.try_get("", "definition")?;
        if definition.matches(source.as_str()).count() != 1 {
            return Err(invalid("selected recovery definition body is not unique"));
        }
        // Preserve the actual header/attributes, function identity, ACL and trigger.
        db.execute_unprepared(&definition.replacen(source.as_str(), &body, 1)).await?;
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Err(invalid("recovery is forward-only; original operation history is immutable"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_recovery_changes_only_fresh_stop_admission() {
        let before = previous_guard().unwrap();
        let after = current_guard(&before).unwrap();
        assert!(after.contains("IS DISTINCT FROM 'stop'"));
        assert!(after.contains("= (head#>>'{lease,version}')::bigint"));
        assert!(after.contains("SDLC unsent Stop requires the unbound original reservation"));
        let restored = after.replace(" IS DISTINCT FROM 'stop'", "='retry_unsubmitted'")
            .replace("SDLC fresh reconciliation requires explicit Stop",
                "SDLC ownership does not expire or retry automatically");
        assert_eq!(restored, before);
    }

    #[test]
    fn foreign_recovery_guard_is_rejected_without_replacement() {
        let before = previous_guard().unwrap();
        assert!(current_guard(&before.replace("FOR UPDATE", "")).is_err());
        assert!(current_guard("BEGIN RETURN NEW; END").is_err());
    }
}
