use super::*;
use sea_orm::{MockDatabase, Transaction, Value, sea_query::ArrayType};
use std::collections::BTreeMap;

fn row(project: ProjectId, category: &str, count: i64) -> BTreeMap<String, Value> {
    BTreeMap::from([
        ("project_id".into(), project.as_uuid().into()),
        ("category".into(), category.into()),
        ("count".into(), count.into()),
    ])
}

fn repo(rows: Vec<BTreeMap<String, Value>>) -> IssueRepo {
    IssueRepo {
        db: Arc::new(crate::guarded_db::GuardedDb::new(
            MockDatabase::new(DbBackend::Postgres)
                .append_query_results([rows])
                .into_connection(),
        )),
    }
}

#[test]
fn status_mapping_recognizes_retained_in_progress_without_defaulting_to_todo() {
    for (category, expected) in [
        ("todo", StatusCategory::Todo),
        ("inprogress", StatusCategory::InProgress),
        ("in_progress", StatusCategory::InProgress),
        ("done", StatusCategory::Done),
    ] {
        let mapped = map_status(status::Model {
            id: Uuid::new_v4(),
            name: "Fixture".into(),
            category: category.into(),
            position: 0,
            is_default: false,
            is_closed: false,
            created_at: shared::now(),
        });
        assert_eq!(mapped.category, expected);
    }
    assert_eq!(map_status_category("foreign"), None);
}

#[test]
fn status_mapping_round_trips_current_writer_and_wire_categories() {
    for category in [
        StatusCategory::Todo,
        StatusCategory::InProgress,
        StatusCategory::Done,
    ] {
        assert_eq!(
            map_status_category(&format!("{category:?}")),
            Some(category)
        );
        let wire = serde_json::to_value(category).unwrap();
        assert_eq!(map_status_category(wire.as_str().unwrap()), Some(category));
    }
}

#[tokio::test]
async fn batch_counts_sum_both_in_progress_spellings_and_keep_other_projects_separate() {
    let first = ProjectId::new();
    let second = ProjectId::new();
    for aliases in [
        [("inprogress", 3), ("in_progress", 4)],
        [("in_progress", 4), ("inprogress", 3)],
        [("InProgress", 3), ("in_progress", 4)],
        [("in_progress", 4), ("InProgress", 3)],
        [("INPROGRESS", 3), ("in_progress", 4)],
    ] {
        let mut rows = vec![row(first, "Todo", 2), row(first, "Done", 5)];
        rows.extend(aliases.map(|(category, count)| row(first, category, count)));
        rows.push(row(second, "in_progress", 1));
        let repository = repo(rows);
        let counts = repository
            .status_category_counts(&[first, second], &[])
            .await
            .unwrap();
        assert_eq!(counts.len(), 2);
        assert_eq!(counts[&first], [2, 7, 5]);
        assert_eq!(counts[&second], [0, 1, 0]);

        // Mock execution checks the actual query/bindings, not PostgreSQL filtering.
        let db = Arc::try_unwrap(repository.db)
            .unwrap_or_else(|_| panic!("fixture retains a database reference"));
        assert_eq!(
            db.into_transaction_log(),
            vec![Transaction::from_sql_and_values(
                DbBackend::Postgres,
                "SELECT i.project_id,s.category,count(*)::bigint AS count FROM issues i JOIN statuses s ON s.id=i.status_id WHERE i.project_id=ANY($1) AND i.deleted_at IS NULL GROUP BY i.project_id,s.category",
                [Value::Array(
                    ArrayType::Uuid,
                    Some(Box::new(vec![
                        first.as_uuid().into(),
                        second.as_uuid().into()
                    ])),
                )],
            )]
        );
    }
}

#[tokio::test]
async fn batch_counts_reject_unknown_category_instead_of_silently_counting_it_as_todo() {
    let project = ProjectId::new();
    let error = repo(vec![row(project, "foreign", 1)])
        .status_category_counts(&[project], &[])
        .await
        .unwrap_err();
    assert!(matches!(error, AppError::Internal(reason) if reason == "invalid_status_category"));
}

#[tokio::test]
async fn batch_counts_empty_result_remains_empty() {
    let counts = repo(Vec::new())
        .status_category_counts(&[], &[])
        .await
        .unwrap();
    assert!(counts.is_empty());
}
