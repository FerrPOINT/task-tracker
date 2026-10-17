const SQL: &str = include_str!("../src/critical_pm_review_lifecycle.sql");

#[test]
fn pending_096_begin_checks_count_both_eols_without_normalizing_source() {
    let sql = SQL.replace("\r\n", "\n");
    for pattern in [
        "(length(source)-length(replace(source,E'BEGIN\\n','')))/length(E'BEGIN\\n')",
        "+ (length(source)-length(replace(source,E'BEGIN\\r\\n','')))/length(E'BEGIN\\r\\n')<>1",
        "replacement:=replace(replacement,E'\\n',E'\\r\\n')",
    ] {
        assert_eq!(sql.matches(pattern).count(), 2, "{pattern}");
    }
    assert_eq!(
        sql.matches("EXECUTE replace(source,anchor,replacement)")
            .count(),
        2
    );
    assert!(!sql.contains("source:=replace(source,E'\\r\\n'"));
    assert!(!sql.contains("source:=replace(source,E'\\n'"));
    assert!(sql.contains("RAISE EXCEPTION 'SDLC guard anchor differs %',name"));
    assert!(sql.contains("RAISE EXCEPTION 'SDLC PM ownership guard anchor differs'"));
}
