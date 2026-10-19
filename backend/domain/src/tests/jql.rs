use super::{BinaryOperator, Expr, Field, Value, parse};

#[test]
fn parses_boolean_precedence_and_parentheses() {
    let query =
        parse("project = TT AND (priority IN (high, urgent) OR assignee = currentUser())")
            .expect("valid JQL");

    assert_eq!(
        query,
        Expr::And(
            Box::new(Expr::Clause {
                field: Field::Project,
                operator: BinaryOperator::Equals,
                values: vec![Value::Text("TT".into())],
            }),
            Box::new(Expr::Or(
                Box::new(Expr::Clause {
                    field: Field::Priority,
                    operator: BinaryOperator::In,
                    values: vec![Value::Text("high".into()), Value::Text("urgent".into())],
                }),
                Box::new(Expr::Clause {
                    field: Field::Assignee,
                    operator: BinaryOperator::Equals,
                    values: vec![Value::Function("currentUser".into())],
                }),
            )),
        )
    );
}

#[test]
fn parses_not_empty_and_quoted_values() {
    let query = parse("NOT assignee IS EMPTY OR status = \"In Progress\"").expect("valid JQL");

    assert_eq!(
        query,
        Expr::Or(
            Box::new(Expr::Not(Box::new(Expr::IsEmpty {
                field: Field::Assignee,
                negated: false,
            }))),
            Box::new(Expr::Clause {
                field: Field::Status,
                operator: BinaryOperator::Equals,
                values: vec![Value::Text("In Progress".into())],
            }),
        )
    );
}

#[test]
fn rejects_unknown_field_with_position() {
    let error = parse("unknown = value").expect_err("unknown field must fail");

    assert_eq!(error.position, 0);
    assert!(error.message.contains("unknown field"));
}

#[test]
fn rejects_unclosed_value_list() {
    let error = parse("priority IN (high, urgent").expect_err("unclosed list must fail");

    assert!(error.message.contains("expected ')'"));
}

#[test]
fn parses_comparison_operators() {
    let expr = parse("created >= 2026-01-01").expect("valid JQL");
    assert_eq!(
        expr,
        Expr::Clause {
            field: Field::Created,
            operator: BinaryOperator::GreaterThanOrEqual,
            values: vec![Value::Text("2026-01-01".into())],
        }
    );

    let expr = parse("priority != low").expect("valid JQL");
    assert_eq!(
        expr,
        Expr::Clause {
            field: Field::Priority,
            operator: BinaryOperator::NotEquals,
            values: vec![Value::Text("low".into())],
        }
    );
}

#[test]
fn parses_not_in_operator() {
    let expr = parse("status NOT IN (Done, Closed)").expect("valid JQL");
    assert_eq!(
        expr,
        Expr::Clause {
            field: Field::Status,
            operator: BinaryOperator::NotIn,
            values: vec![Value::Text("Done".into()), Value::Text("Closed".into()),],
        }
    );
}

#[test]
fn parses_is_not_empty() {
    let expr = parse("assignee IS NOT EMPTY").expect("valid JQL");
    assert_eq!(
        expr,
        Expr::IsEmpty {
            field: Field::Assignee,
            negated: true,
        }
    );
}

#[test]
fn parses_contains_and_not_contains() {
    let expr = parse("summary ~ \"bug\"").expect("valid JQL");
    assert_eq!(
        expr,
        Expr::Clause {
            field: Field::Summary,
            operator: BinaryOperator::Contains,
            values: vec![Value::Text("bug".into())],
        }
    );

    let expr = parse("description !~ \"deprecated\"").expect("valid JQL");
    assert_eq!(
        expr,
        Expr::Clause {
            field: Field::Description,
            operator: BinaryOperator::NotContains,
            values: vec![Value::Text("deprecated".into())],
        }
    );
}

#[test]
fn parses_chained_and_or_without_parens() {
    // AND binds tighter than OR: a AND b OR c == (a AND b) OR c
    let expr = parse("project = TT AND priority = high OR status = Done").expect("valid JQL");
    assert_eq!(
        expr,
        Expr::Or(
            Box::new(Expr::And(
                Box::new(Expr::Clause {
                    field: Field::Project,
                    operator: BinaryOperator::Equals,
                    values: vec![Value::Text("TT".into())],
                }),
                Box::new(Expr::Clause {
                    field: Field::Priority,
                    operator: BinaryOperator::Equals,
                    values: vec![Value::Text("high".into())],
                }),
            )),
            Box::new(Expr::Clause {
                field: Field::Status,
                operator: BinaryOperator::Equals,
                values: vec![Value::Text("Done".into())],
            }),
        )
    );
}

#[test]
fn parses_nested_parentheses() {
    let expr = parse("((project = TT))").expect("valid JQL");
    assert_eq!(
        expr,
        Expr::Clause {
            field: Field::Project,
            operator: BinaryOperator::Equals,
            values: vec![Value::Text("TT".into())],
        }
    );
}

#[test]
fn rejects_empty_input() {
    assert!(parse("").is_err());
}

#[test]
fn rejects_field_without_operator() {
    let error = parse("project").expect_err("dangling field must fail");
    assert!(error.message.contains("expected operator"));
}

#[test]
fn rejects_unterminated_quote() {
    let error = parse("status = \"In Progress").expect_err("unterminated quote must fail");
    assert!(error.message.contains("unterminated quoted value"));
}

#[test]
fn rejects_trailing_token() {
    let error = parse("project = TT extra").expect_err("trailing token must fail");
    assert!(error.message.contains("unexpected token"));
}

#[test]
fn parses_case_insensitive_keywords() {
    let expr = parse("project = TT and priority = high").expect("valid JQL");
    assert!(matches!(expr, Expr::And(..)));

    let expr = parse("project = TT OR priority = high").expect("valid JQL");
    assert!(matches!(expr, Expr::Or(..)));

    let expr = parse("not assignee IS EMPTY").expect("valid JQL");
    assert!(matches!(expr, Expr::Not(..)));
}
