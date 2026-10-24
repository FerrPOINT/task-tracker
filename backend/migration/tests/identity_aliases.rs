//! Populated upgrade and immutable identity provenance in real isolated PostgreSQL.
use migration::{Migrator, MigratorTrait};
use sea_orm::{ConnectionTrait, Database, DatabaseBackend, Statement, TransactionTrait};

#[tokio::test]
#[ignore = "requires dedicated tasktracker_migration_test PostgreSQL database"]
async fn populated_identity_aliases_survive_delete_and_reject_rebinding() {
    let url =
        std::env::var("TT_MIGRATION_TEST_DATABASE_URL").expect("dedicated migration database");
    let db = Database::connect(url).await.unwrap();
    let name: String = db
        .query_one(Statement::from_string(
            DatabaseBackend::Postgres,
            "SELECT current_database() AS name",
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get("", "name")
        .unwrap();
    assert_eq!(
        name, "tasktracker_migration_test",
        "refuse application database"
    );
    let tx = db.begin().await.unwrap();
    let schema: String = tx
        .query_one(Statement::from_string(
            DatabaseBackend::Postgres,
            "SELECT 'identity_aliases_'||replace(gen_random_uuid()::text,'-','') AS name",
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get("", "name")
        .unwrap();
    tx.execute_unprepared(&format!(
        "CREATE SCHEMA {schema}; SET LOCAL search_path TO {schema}"
    ))
    .await
    .unwrap();
    Migrator::up(&tx, Some((Migrator::migrations().len() - 1) as u32))
        .await
        .unwrap();
    tx.execute_unprepared("INSERT INTO users(id,email,username,display_name,password_hash,central_sub)
        VALUES('00000000-0000-4000-8000-000000000001','synthetic@example.test','synthetic-old','Synthetic','!',
        '00000000-0000-4000-8000-000000000002')").await.unwrap();
    Migrator::up(&tx, None).await.unwrap();
    let count = |sql| Statement::from_string(DatabaseBackend::Postgres, sql);
    let aliases: i64 = tx
        .query_one(count("SELECT count(*) AS n FROM tracker_identity_aliases"))
        .await
        .unwrap()
        .unwrap()
        .try_get("", "n")
        .unwrap();
    assert_eq!(aliases, 1, "populated binding must be backfilled");
    tx.execute_unprepared("UPDATE users SET username='synthetic-renamed' WHERE id='00000000-0000-4000-8000-000000000001'")
        .await.unwrap();
    for (sql, reason) in [
        (
            "UPDATE users SET central_sub='00000000-0000-4000-8000-000000000003' WHERE id='00000000-0000-4000-8000-000000000001'",
            "identity_alias_rebind_denied",
        ),
        (
            "UPDATE users SET central_sub=NULL WHERE id='00000000-0000-4000-8000-000000000001'",
            "identity_alias_rebind_denied",
        ),
        (
            "UPDATE users SET id='00000000-0000-4000-8000-000000000004' WHERE id='00000000-0000-4000-8000-000000000001'",
            "identity_local_id_immutable",
        ),
        (
            "DELETE FROM tracker_identity_aliases",
            "identity_alias_immutable",
        ),
    ] {
        tx.execute_unprepared("SAVEPOINT rejected_identity_write")
            .await
            .unwrap();
        let error = tx.execute_unprepared(sql).await.unwrap_err();
        assert!(
            error.to_string().contains(reason),
            "expected classified identity invariant"
        );
        tx.execute_unprepared("ROLLBACK TO SAVEPOINT rejected_identity_write; RELEASE SAVEPOINT rejected_identity_write")
            .await.unwrap();
    }
    tx.execute_unprepared("DELETE FROM users WHERE id='00000000-0000-4000-8000-000000000001'")
        .await
        .unwrap();
    let alias: String = tx
        .query_one(count(
            "SELECT central_subject FROM tracker_identity_aliases",
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get("", "central_subject")
        .unwrap();
    assert_eq!(alias, "00000000-0000-4000-8000-000000000002");
    tx.execute_unprepared("SAVEPOINT rejected_uuid_reuse")
        .await
        .unwrap();
    let error = tx.execute_unprepared("INSERT INTO users(id,email,username,display_name,password_hash)
        VALUES('00000000-0000-4000-8000-000000000001','other@example.test','synthetic-other','Other','!')")
        .await.unwrap_err();
    assert!(error.to_string().contains("identity_alias_rebind_denied"));
    tx.execute_unprepared(
        "ROLLBACK TO SAVEPOINT rejected_uuid_reuse; RELEASE SAVEPOINT rejected_uuid_reuse",
    )
    .await
    .unwrap();
    // A rollback removes the profile and its captured binding together.
    tx.execute_unprepared("SAVEPOINT provisional_binding;
        INSERT INTO users(id,email,username,display_name,password_hash,central_sub)
        VALUES('00000000-0000-4000-8000-000000000005','rollback@example.test','synthetic-rollback','Rollback','!',
        '00000000-0000-4000-8000-000000000006');
        ROLLBACK TO SAVEPOINT provisional_binding; RELEASE SAVEPOINT provisional_binding")
        .await.unwrap();
    let aliases: i64 = tx
        .query_one(count("SELECT count(*) AS n FROM tracker_identity_aliases"))
        .await
        .unwrap()
        .unwrap()
        .try_get("", "n")
        .unwrap();
    assert_eq!(aliases, 1);
    // An operator-provided alias that contradicts a later profile must deny
    // insertion, rather than silently rebinding historical local references.
    tx.execute_unprepared(
        "INSERT INTO tracker_identity_aliases(local_user_id,central_subject)
        VALUES('00000000-0000-4000-8000-000000000008','00000000-0000-4000-8000-000000000009');
        SAVEPOINT conflicting_alias_profile",
    )
    .await
    .unwrap();
    let error=tx.execute_unprepared("INSERT INTO users(id,email,username,display_name,password_hash,central_sub)
        VALUES('00000000-0000-4000-8000-000000000008','conflict@example.test','synthetic-conflict','Conflict','!',
        '00000000-0000-4000-8000-000000000010')").await.unwrap_err();
    assert!(error.to_string().contains("identity_alias_rebind_denied"));
    tx.execute_unprepared("ROLLBACK TO SAVEPOINT conflicting_alias_profile; RELEASE SAVEPOINT conflicting_alias_profile").await.unwrap();
    let absent: i64 = tx
        .query_one(count(
            "SELECT count(*) AS n FROM users WHERE id='00000000-0000-4000-8000-000000000008'",
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get("", "n")
        .unwrap();
    assert_eq!(
        absent, 0,
        "conflicting alias cannot create a partially bound profile"
    );
    let unchanged:String=tx.query_one(count("SELECT central_subject FROM tracker_identity_aliases WHERE local_user_id='00000000-0000-4000-8000-000000000008'"))
        .await.unwrap().unwrap().try_get("","central_subject").unwrap();
    assert_eq!(unchanged, "00000000-0000-4000-8000-000000000009");
    // A new local projection for the same exact central subject keeps both
    // aliases, so old audit/history references remain attributable after JIT.
    tx.execute_unprepared("INSERT INTO users(id,email,username,display_name,password_hash,central_sub)
        VALUES('00000000-0000-4000-8000-000000000007','recreated@example.test','synthetic-recreated','Recreated','!',
        '00000000-0000-4000-8000-000000000002')").await.unwrap();
    let aliases:i64=tx.query_one(count("SELECT count(*) AS n FROM tracker_identity_aliases WHERE central_subject='00000000-0000-4000-8000-000000000002'"))
        .await.unwrap().unwrap().try_get("","n").unwrap();
    assert_eq!(
        aliases, 2,
        "new projection must not erase the deleted profile alias"
    );
    tx.rollback().await.unwrap();
    db.close().await.unwrap();
}
