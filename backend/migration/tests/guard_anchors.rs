const SQL: &str = include_str!("../src/critical_pm_review_lifecycle.sql");

#[test]
fn pending_096_begin_checks_count_both_eols_without_normalizing_source() {
    let sql = SQL.replace("\r\n", "\n");
    // Resume-input rewrites share this DO block; inspect only the BEGIN insertions.
    let gates = sql
        .split_once("-- Preserve legacy function bodies; checked anchors insert narrow current-transaction branches only.\n")
        .unwrap()
        .1
        .split_once("    END LOOP;")
        .unwrap()
        .0;
    let ownership = sql
        .split_once("-- Existing PM ownership/write permits are preserved. This exact journal is the ONLY new branch.\n")
        .unwrap()
        .1
        .split_once("END $$;")
        .unwrap()
        .0;
    for guarded in [gates, ownership] {
        for pattern in [
            "(length(source)-length(replace(source,E'BEGIN\\n','')))/length(E'BEGIN\\n')",
            "+ (length(source)-length(replace(source,E'BEGIN\\r\\n','')))/length(E'BEGIN\\r\\n')<>1",
            "replacement:=replace(replacement,E'\\n',E'\\r\\n')",
        ] {
            assert_eq!(guarded.matches(pattern).count(), 1, "{pattern}");
        }
        assert_eq!(
            guarded.matches("EXECUTE replace(source,anchor,replacement)")
                .count(),
            1
        );
    }
    assert!(!sql.contains("source:=replace(source,E'\\r\\n'"));
    assert!(!sql.contains("source:=replace(source,E'\\n'"));
    assert!(sql.contains("RAISE EXCEPTION 'SDLC guard anchor differs %',name"));
    assert!(sql.contains("RAISE EXCEPTION 'SDLC PM ownership guard anchor differs'"));
}
