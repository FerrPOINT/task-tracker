use super::*;
use serde_json::json;

fn entry(n: u128) -> ProjectDirectoryEntry {
    ProjectDirectoryEntry {
        id: Uuid::from_u128(n),
        key: format!("P{n}"),
        name: format!("Project {n}"),
    }
}

#[test]
fn directory_prefix_uses_lookahead_and_required_null_without_other_metadata() {
    for (rows, limit, expected_len, cursor) in [
        (0, 1, 0, None),
        (1, 1, 1, None),
        (2, 1, 1, Some(1)),
        (2, 2, 2, None),
        (3, 2, 2, Some(2)),
        (100, 100, 100, None),
        (101, 100, 100, Some(100)),
    ] {
        let page = ProjectDirectory::from_prefix(
            "tracker".into(),
            (1..=rows).map(entry).collect(),
            None,
            limit,
        )
        .unwrap();
        assert_eq!(page.projects.len(), expected_len);
        assert_eq!(page.next_cursor, cursor.map(Uuid::from_u128));
        let wire = serde_json::to_value(&page).unwrap();
        assert_eq!(wire.as_object().unwrap().len(), 4);
        assert!(wire.get("next_cursor").is_some());
        for project in wire["projects"].as_array().unwrap() {
            assert_eq!(project.as_object().unwrap().len(), 3);
        }
        serde_json::from_value::<ProjectDirectory>(wire.clone()).unwrap();
        let mut missing = wire;
        missing.as_object_mut().unwrap().remove("next_cursor");
        assert!(serde_json::from_value::<ProjectDirectory>(missing).is_err());
    }
    let mut wire = serde_json::to_value(
        ProjectDirectory::from_prefix("tracker".into(), vec![entry(1)], None, 1).unwrap(),
    )
    .unwrap();
    wire["count"] = json!(1);
    assert!(serde_json::from_value::<ProjectDirectory>(wire).is_err());
    let mut wire = serde_json::to_value(entry(1)).unwrap();
    wire["description"] = json!("not exposed");
    assert!(serde_json::from_value::<ProjectDirectoryEntry>(wire).is_err());
}

#[test]
fn directory_rejects_invalid_bounds_and_corrupt_or_noncontiguous_prefix() {
    for limit in [0, 101, u16::MAX] {
        assert!(ProjectDirectory::from_prefix("tracker".into(), vec![], None, limit).is_err());
    }
    assert!(
        ProjectDirectory::from_prefix("tracker".into(), vec![], Some(Uuid::nil()), 1).is_err()
    );
    for rows in [
        vec![entry(0)],
        vec![entry(1), entry(1)],
        vec![entry(2), entry(1)],
        vec![entry(1), entry(2), entry(3)],
    ] {
        assert!(ProjectDirectory::from_prefix("tracker".into(), rows, None, 1).is_err());
    }
    assert!(
        ProjectDirectory::from_prefix(
            "tracker".into(),
            vec![entry(1)],
            Some(Uuid::from_u128(1)),
            1
        )
        .is_err()
    );
    assert!(
        ProjectDirectory::from_prefix(
            "tracker".into(),
            vec![entry(2)],
            Some(Uuid::from_u128(1)),
            1
        )
        .is_ok()
    );
}
