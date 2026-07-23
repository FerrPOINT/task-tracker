#[cfg(test)]
mod tests {
    use crate::{
        AttachmentId, BoardId, CommentId, IssueId, IssueKey, IssueType, LabelId, Priority,
        ProjectId, ProjectKey, SprintId, StatusId, UserId,
    };
    use std::str::FromStr;

    #[test]
    fn all_uuid_ids_roundtrip_and_nil() {
        let cases: Vec<(Box<dyn Fn() -> String>, Box<dyn Fn(&str) -> bool>)> = vec![
            (
                Box::new(|| UserId::new().to_string()),
                Box::new(|s| UserId::from_str(s).is_ok()),
            ),
            (
                Box::new(|| ProjectId::new().to_string()),
                Box::new(|s| ProjectId::from_str(s).is_ok()),
            ),
            (
                Box::new(|| IssueId::new().to_string()),
                Box::new(|s| IssueId::from_str(s).is_ok()),
            ),
            (
                Box::new(|| CommentId::new().to_string()),
                Box::new(|s| CommentId::from_str(s).is_ok()),
            ),
            (
                Box::new(|| AttachmentId::new().to_string()),
                Box::new(|s| AttachmentId::from_str(s).is_ok()),
            ),
            (
                Box::new(|| LabelId::new().to_string()),
                Box::new(|s| LabelId::from_str(s).is_ok()),
            ),
            (
                Box::new(|| SprintId::new().to_string()),
                Box::new(|s| SprintId::from_str(s).is_ok()),
            ),
            (
                Box::new(|| BoardId::new().to_string()),
                Box::new(|s| BoardId::from_str(s).is_ok()),
            ),
            (
                Box::new(|| StatusId::new().to_string()),
                Box::new(|s| StatusId::from_str(s).is_ok()),
            ),
        ];
        for (maker, parse) in cases {
            let s = maker();
            assert!(parse(&s), "roundtrip failed for {}", s);
        }
        assert_eq!(
            UserId::nil().to_string(),
            "00000000-0000-0000-0000-000000000000"
        );
    }

    #[test]
    fn uuid_id_rejects_invalid() {
        assert!(UserId::from_str("not-a-uuid").is_err());
        assert!(ProjectId::from_str("").is_err());
    }

    #[test]
    fn project_key_edge_cases() {
        assert!(ProjectKey::from_str("A").is_ok());
        assert!(ProjectKey::from_str("1234567890").is_ok());
        assert!(ProjectKey::from_str("toolongkey123").is_err());
        assert!(ProjectKey::from_str("with space").is_err());
        assert!(ProjectKey::from_str("under_score").is_err());
        assert_eq!(ProjectKey::new("TT").to_string(), "TT");
    }

    #[test]
    fn issue_key_edge_cases() {
        assert!(IssueKey::parse("").is_err());
        assert!(IssueKey::parse("ABC").is_err());
        assert!(IssueKey::parse("ABC-").is_err());
        assert!(IssueKey::parse("ABC-xyz").is_err());
        assert!(IssueKey::parse("-5").is_err());
        let key = IssueKey::new(ProjectKey::new("XX"), 99);
        assert_eq!(key.to_string(), "XX-99");
        assert_eq!(format!("{}", key), "XX-99");
    }

    #[test]
    fn issue_type_all_variants() {
        assert_eq!(IssueType::from_str("Bug").unwrap(), IssueType::Bug);
        assert_eq!(IssueType::from_str("STORY").unwrap(), IssueType::Story);
        assert_eq!(IssueType::from_str("эпик").unwrap(), IssueType::Epic);
        assert_eq!(
            IssueType::from_str("подзадача").unwrap(),
            IssueType::SubTask
        );
        assert!(IssueType::from_str("").is_err());
    }

    #[test]
    fn priority_all_variants_and_as_str() {
        assert_eq!(Priority::from_str("Lowest").unwrap(), Priority::Lowest);
        assert_eq!(Priority::from_str("LOW").unwrap(), Priority::Low);
        assert_eq!(Priority::from_str("medium").unwrap(), Priority::Medium);
        assert_eq!(Priority::from_str("HIGH").unwrap(), Priority::High);
        assert_eq!(Priority::from_str("Highest").unwrap(), Priority::Highest);
        assert!(Priority::from_str("").is_err());
        assert_eq!(Priority::Lowest.as_str(), "Lowest");
        assert_eq!(Priority::Highest.as_str(), "Highest");
    }
}
