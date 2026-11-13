use sea_orm_migration::prelude::*;
use sea_orm::{DbBackend, Statement};

#[derive(DeriveMigrationName)]
pub struct Migration;

fn invalid(message: &str) -> DbErr {
    DbErr::Custom(format!("delivery convergence: {message}"))
}

fn body(sql: &str, name: &str) -> Result<String, DbErr> {
    let normalized = sql.replace("\r\n", "\n");
    let (_, function) = normalized
        .split_once(&format!("FUNCTION {name}()"))
        .ok_or_else(|| invalid("published function absent"))?;
    let (_, function) = function.split_once("AS $$")
        .ok_or_else(|| invalid("published function delimiter changed"))?;
    Ok(function.split_once("$$;").ok_or_else(|| invalid("published function end changed"))?.0.into())
}

fn replace_once(source: &str, old: &str, new: &str) -> Result<String, DbErr> {
    if source.matches(old).count() != 1 {
        return Err(invalid("published guard anchor changed"));
    }
    Ok(source.replacen(old, new, 1))
}

fn primary_head() -> Result<String, DbErr> {
    let mut head = body(include_str!("critical_pm_return.sql"), "sdlc_delivery_head_gate")?;
    let untimed = include_str!("untimed_delivery_binding.sql").replace("\r\n", "\n");
    let (_, patch) = untimed.split_once("source:=replace(pg_get_functiondef('sdlc_delivery_head_gate()'::regprocedure)")
        .ok_or_else(|| invalid("published head patch absent"))?;
    let anchors: Vec<_> = patch.split("$anchor$").skip(1).step_by(2).collect();
    if anchors.len() != 4 {
        return Err(invalid("published head patch structure changed"));
    }
    head = replace_once(&head, anchors[0], anchors[1])?;
    replace_once(&head, anchors[2], anchors[3])
}

fn require_guard(actual: &str, expected: &str) -> Result<(), DbErr> {
    if actual.replace("\r\n", "\n") != expected {
        return Err(invalid("foreign guard definition; no changes applied"));
    }
    Ok(())
}

async fn function<C: ConnectionTrait>(db: &C, name: &str) -> Result<String, DbErr> {
    db.query_one(Statement::from_sql_and_values(DbBackend::Postgres,
        "SELECT prosrc FROM pg_proc WHERE oid=to_regprocedure($1)", [format!("public.{name}()").into()]))
        .await?.ok_or_else(|| invalid("selected guard absent"))?.try_get("", "prosrc")
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        let row = db.query_one(Statement::from_string(DbBackend::Postgres,
            "SELECT EXISTS(SELECT 1 FROM seaql_migrations WHERE version='m20261113_0000118_delivery_unsent_stop') AS installed,
             EXISTS(SELECT 1 FROM seaql_migrations WHERE version='m20261111_0000118_plan_content_dispatch') AS published".to_owned()))
            .await?.ok_or_else(|| invalid("accepted migration history absent"))?;
        let installed: bool = row.try_get("", "installed")?;
        let published: bool = row.try_get("", "published")?;
        if installed == published {
            return Err(invalid("exactly one accepted 118 lineage required"));
        }
        let head = primary_head()?;
        let expected_head = if installed {
            body(include_str!("persistent_delivery_ownership.sql"), "sdlc_delivery_head_gate")?
        } else { head.clone() };
        let expected_reconciliation = body(if installed {
            include_str!("delivery_unsent_stop.sql")
        } else { include_str!("single_task_lifecycle.sql") }, "sdlc_delivery_reconciliation_gate")?;
        // Both complete definitions are checked before any schema or projection write.
        require_guard(&function(db, "sdlc_delivery_head_gate").await?, &expected_head)?;
        require_guard(&function(db, "sdlc_delivery_reconciliation_gate").await?, &expected_reconciliation)?;

        if installed {
            for sql in [include_str!("reviewer_terminal.sql"), include_str!("linear_delivery.sql"),
                include_str!("plan_content_dispatch.sql")] {
                db.execute_unprepared(&sql.replace("\r\n", "\n")).await?;
            }
            let signature = "public.sdlc_critical_blocker_insert_gate()";
            let row = db.query_one(Statement::from_sql_and_values(DbBackend::Postgres,
                "SELECT pg_get_functiondef(to_regprocedure($1)) AS definition", [signature.into()]))
                .await?.ok_or_else(|| invalid("critical blocker guard absent"))?;
            let source: String = row.try_get("", "definition")?;
            let source = replace_once(&source.replace("\r\n", "\n"),
                "BETWEEN 1 AND (head->'lease'->>'version')::bigint",
                "= (head->'lease'->>'version')::bigint")?;
            db.execute_unprepared(&source).await?;
        }
        // The same current guard is installed on both accepted histories. Historical
        // operation/result/assignment/head bytes and applied ledger entries are untouched.
        let stop = replace_once(&include_str!("delivery_unsent_stop.sql").replace("\r\n", "\n"),
            "BETWEEN 1 AND (head#>>'{lease,version}')::bigint",
            "= (head#>>'{lease,version}')::bigint")?;
        db.execute_unprepared(&stop).await?;
        let view = include_str!("untimed_delivery_binding.sql").split_once("DO $patch$")
            .ok_or_else(|| invalid("published view absent"))?.0;
        db.execute_unprepared(&view.replace("\r\n", "\n")).await?;
        db.execute_unprepared(&format!("CREATE OR REPLACE FUNCTION public.sdlc_delivery_head_gate() RETURNS trigger LANGUAGE plpgsql AS $current${head}$current$;"))
            .await?;
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Err(invalid("current delivery contract is forward-only; history is immutable"))
    }
}

#[cfg(test)]
#[path = "current_delivery_contract_tests.rs"]
mod tests;
