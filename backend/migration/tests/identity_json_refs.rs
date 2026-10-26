//! Extractor qualification and an independent inventory from the actual PG
//! foreign-key catalog. This does not substitute for every business writer test.
use migration::{Migrator, MigratorTrait};
use sea_orm::{ConnectionTrait, Database, DatabaseBackend, Statement, TransactionTrait};
use serde_json::json;

fn sql(query: &str, args: impl IntoIterator<Item = sea_orm::Value>) -> Statement {
    Statement::from_sql_and_values(DatabaseBackend::Postgres, query, args)
}

#[tokio::test]
#[ignore = "requires dedicated tasktracker_migration_test PostgreSQL database"]
async fn typed_json_and_actual_user_foreign_keys_have_identity_guards_without_scanning_content() {
    let url =
        std::env::var("TT_MIGRATION_TEST_DATABASE_URL").expect("dedicated migration PostgreSQL");
    let db = Database::connect(url).await.unwrap();
    let name: String = db
        .query_one(sql("SELECT current_database() AS name", []))
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
        .query_one(sql(
            "SELECT 'identity_json_'||replace(gen_random_uuid()::text,'-','') AS name",
            [],
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
    Migrator::up(&tx, None).await.unwrap();
    tx.execute_unprepared("INSERT INTO tracker_identity_legacy_issuer(issuer) VALUES('https://auth.example.test');
        INSERT INTO users(id,email,username,display_name,password_hash,central_sub)
        VALUES('00000000-0000-4000-8000-000000000001','json@example.test','json-fixture','Synthetic','!','exact-central')")
        .await.unwrap();

    // All real foreign-key-to-users columns, not a copy of the extractor's
    // hand-maintained source list. A new table/column must not silently escape.
    let columns=tx.query_all(sql("SELECT table_class.relname AS source,attribute.attname AS column,
        EXISTS(SELECT 1 FROM pg_trigger t WHERE t.tgrelid=constraint_row.conrelid AND NOT t.tgisinternal
            AND t.tgname='a_identity_reference_lock' AND t.tgenabled IN ('O','A')
            AND t.tgfoid='tracker_identity_write_lock'::regproc AND (t.tgtype & 31)=30) AS guarded,
        EXISTS(SELECT 1 FROM pg_trigger t WHERE t.tgrelid=constraint_row.conrelid AND NOT t.tgisinternal
            AND t.tgname='z_identity_reference_guard' AND t.tgenabled IN ('O','A')
            AND t.tgfoid='tracker_identity_scalar_write_guard'::regproc AND (t.tgtype & 23)=23) AS row_guarded
        FROM pg_constraint constraint_row
        JOIN pg_class table_class ON table_class.oid=constraint_row.conrelid
        JOIN pg_attribute attribute ON attribute.attrelid=constraint_row.conrelid
            AND attribute.attnum=ANY(constraint_row.conkey)
        WHERE constraint_row.contype='f' AND constraint_row.confrelid='users'::regclass
        ORDER BY table_class.relname,attribute.attname",[])).await.unwrap();
    assert!(
        !columns.is_empty(),
        "must inspect the fully migrated actual schema"
    );
    for column in columns {
        let source: String = column.try_get("", "source").unwrap();
        let key: String = column.try_get("", "column").unwrap();
        assert!(
            column.try_get::<bool>("", "guarded").unwrap(),
            "actual user FK writer missing statement guard: {source}.{key}"
        );
        assert!(
            column.try_get::<bool>("", "row_guarded").unwrap(),
            "actual user FK writer missing active row guard: {source}.{key}"
        );
        let mut fields = serde_json::Map::new();
        fields.insert(key.clone(), json!("00000000-0000-4000-8000-000000000001"));
        let document = serde_json::Value::Object(fields);
        let matched:bool=tx.query_one(sql("SELECT EXISTS(SELECT 1 FROM tracker_identity_row_refs($1,$2) r
            WHERE r.resolved AND r.issuer='https://auth.example.test' AND r.subject='exact-central') AS matched",
            [source.clone().into(),document.into()])).await.unwrap().unwrap().try_get("","matched").unwrap();
        assert!(
            matched,
            "actual user FK missing exact alias extraction: {source}.{key}"
        );
    }

    let typed = json!({"task":{"owner_subject":"task-owner","routing":{"policy":{"author_subject":"policy-author"}}},
        "assignment":{"machine_subject":"machine"},"questions":[{"author_subject":"question-author",
        "answer":{"author_subject":"answer-author"}}],"revisions":[{"author_subject":"revision-author"}],
        "evidence":[{"verifier_subject":"verifier"}],"actor_identity":{"issuer":"https://other.example.test","subject":"foreign-subject"},
        "content":{"owner_subject":"not-an-identity"},"metadata":{"subject":"not-an-identity"},
        "document":{"subject":"not-an-identity"},"description":"owner_subject=not-an-identity"});
    let rows=tx.query_all(sql("SELECT issuer,subject,resolved FROM tracker_identity_json_refs($1,'https://auth.example.test') ORDER BY subject",
        [typed.into()])).await.unwrap();
    let mut subjects = Vec::new();
    for row in rows {
        assert!(row.try_get::<bool>("", "resolved").unwrap());
        let subject: String = row.try_get("", "subject").unwrap();
        let issuer: String = row.try_get("", "issuer").unwrap();
        assert_eq!(
            issuer,
            if subject == "foreign-subject" {
                "https://other.example.test"
            } else {
                "https://auth.example.test"
            }
        );
        subjects.push(subject);
    }
    assert_eq!(
        subjects,
        vec![
            "answer-author",
            "foreign-subject",
            "machine",
            "policy-author",
            "question-author",
            "revision-author",
            "task-owner",
            "verifier"
        ]
    );
    let unresolved:bool=tx.query_one(sql("SELECT EXISTS(SELECT 1 FROM tracker_identity_json_refs($1,'https://auth.example.test') WHERE NOT resolved) AS unknown",
        [json!({"assignment":{"machine_subject":17}}).into()])).await.unwrap().unwrap().try_get("","unknown").unwrap();
    assert!(
        unresolved,
        "malformed typed subject must not become zero references"
    );
    for malformed in [
        json!({"actor_identity":{"issuer":17,"subject":"exact-central"}}),
        json!({"actor_identity":{"issuer":null,"subject":"exact-central"}}),
        json!({"actor_identity":{"issuer":" ","subject":"exact-central"}}),
        json!({"actor_identity":{"issuer":"https://auth.example.test","subject":null}}),
        json!({"owner_issuer":17,"owner_subject":"exact-central"}),
        json!({"actor_issuer":{},"actor_subject":"exact-central"}),
    ] {
        let unknown:bool=tx.query_one(sql("SELECT EXISTS(SELECT 1 FROM tracker_identity_json_refs($1,'https://auth.example.test') WHERE NOT resolved) AS unknown",
            [malformed.into()])).await.unwrap().unwrap().try_get("","unknown").unwrap();
        assert!(
            unknown,
            "malformed explicit issuer cannot hide a typed identity or fall back to legacy issuer"
        );
    }
    tx.rollback().await.unwrap();
    db.close().await.unwrap();
}
