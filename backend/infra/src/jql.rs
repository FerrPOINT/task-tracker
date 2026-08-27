use domain::jql::{BinaryOperator, Expr, Field, Value};
use shared::UserId;
use std::fmt;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledJql {
    pub predicate: String,
    pub parameters: Vec<JqlParameter>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JqlParameter {
    Text(String),
    Uuid(Uuid),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JqlCompileError(String);

impl fmt::Display for JqlCompileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for JqlCompileError {}

pub fn compile(expression: &Expr, current_user: UserId) -> Result<CompiledJql, JqlCompileError> {
    let mut compiler = Compiler {
        current_user,
        parameters: Vec::new(),
    };
    let predicate = compiler.expression(expression)?;
    Ok(CompiledJql {
        predicate,
        parameters: compiler.parameters,
    })
}

struct Compiler {
    current_user: UserId,
    parameters: Vec<JqlParameter>,
}

impl Compiler {
    fn expression(&mut self, expression: &Expr) -> Result<String, JqlCompileError> {
        match expression {
            Expr::And(left, right) => Ok(format!(
                "({} AND {})",
                self.expression(left)?,
                self.expression(right)?
            )),
            Expr::Or(left, right) => Ok(format!(
                "({} OR {})",
                self.expression(left)?,
                self.expression(right)?
            )),
            Expr::Not(expression) => Ok(format!("NOT ({})", self.expression(expression)?)),
            Expr::IsEmpty { field, negated } => self.is_empty(*field, *negated),
            Expr::Clause {
                field,
                operator,
                values,
            } => self.clause(*field, *operator, values),
        }
    }

    fn clause(
        &mut self,
        field: Field,
        operator: BinaryOperator,
        values: &[Value],
    ) -> Result<String, JqlCompileError> {
        if values.is_empty() {
            return Err(JqlCompileError(
                "a JQL clause needs at least one value".into(),
            ));
        }

        match field {
            Field::Text => self.full_text(operator, values),
            Field::Status => self.status_clause("s.name", operator, values),
            Field::StatusCategory => self.status_clause("s.category", operator, values),
            Field::Labels => self.label_clause(operator, values),
            Field::Sprint => self.sprint_clause(operator, values),
            Field::Assignee | Field::Reporter => self.user_clause(field, operator, values),
            Field::Project | Field::ProjectKey => self.scalar_clause("p.key", operator, values),
            Field::Key => self.scalar_clause("i.key", operator, values),
            Field::Summary => self.text_clause("i.summary", operator, values),
            Field::Description => self.text_clause("i.description", operator, values),
            Field::IssueType => self.scalar_clause("i.issue_type", operator, values),
            Field::Priority => self.scalar_clause("i.priority", operator, values),
            Field::Created => self.timestamp_clause("i.created_at", operator, values),
            Field::Updated => self.timestamp_clause("i.updated_at", operator, values),
            Field::DueDate => self.timestamp_clause("i.due_date", operator, values),
        }
    }

    fn scalar_clause(
        &mut self,
        column: &str,
        operator: BinaryOperator,
        values: &[Value],
    ) -> Result<String, JqlCompileError> {
        self.comparison(column, operator, values, false)
    }

    fn text_clause(
        &mut self,
        column: &str,
        operator: BinaryOperator,
        values: &[Value],
    ) -> Result<String, JqlCompileError> {
        self.comparison(column, operator, values, true)
    }

    fn timestamp_clause(
        &mut self,
        column: &str,
        operator: BinaryOperator,
        values: &[Value],
    ) -> Result<String, JqlCompileError> {
        if matches!(
            operator,
            BinaryOperator::Contains | BinaryOperator::NotContains
        ) {
            return Err(JqlCompileError(
                "text operators are not valid for date fields".into(),
            ));
        }
        self.comparison_with_placeholders(column, operator, values, |compiler, value| {
            let placeholder = compiler.text(value)?;
            Ok(format!("{placeholder}::timestamptz"))
        })
    }

    fn full_text(
        &mut self,
        operator: BinaryOperator,
        values: &[Value],
    ) -> Result<String, JqlCompileError> {
        if values.len() != 1
            || !matches!(
                operator,
                BinaryOperator::Contains | BinaryOperator::NotContains
            )
        {
            return Err(JqlCompileError(
                "text supports only '~' and '!~' with one value".into(),
            ));
        }
        let placeholder = self.text(&values[0])?;
        let predicate = format!("i.tsv_search @@ websearch_to_tsquery('simple', {placeholder})");
        Ok(if matches!(operator, BinaryOperator::NotContains) {
            format!("NOT ({predicate})")
        } else {
            format!("({predicate})")
        })
    }

    fn status_clause(
        &mut self,
        column: &str,
        operator: BinaryOperator,
        values: &[Value],
    ) -> Result<String, JqlCompileError> {
        let predicate = self.comparison(column, operator, values, false)?;
        Ok(format!(
            "EXISTS (SELECT 1 FROM statuses s WHERE s.id = i.status_id AND {predicate})"
        ))
    }

    fn label_clause(
        &mut self,
        operator: BinaryOperator,
        values: &[Value],
    ) -> Result<String, JqlCompileError> {
        let predicate = self.comparison("l.name", operator, values, false)?;
        Ok(format!(
            "EXISTS (SELECT 1 FROM issue_labels il JOIN labels l ON l.id = il.label_id WHERE il.issue_id = i.id AND {predicate})"
        ))
    }

    fn sprint_clause(
        &mut self,
        operator: BinaryOperator,
        values: &[Value],
    ) -> Result<String, JqlCompileError> {
        let predicate = self.comparison("sp.name", operator, values, false)?;
        Ok(format!(
            "EXISTS (SELECT 1 FROM sprints sp WHERE sp.id = i.sprint_id AND {predicate})"
        ))
    }

    fn user_clause(
        &mut self,
        field: Field,
        operator: BinaryOperator,
        values: &[Value],
    ) -> Result<String, JqlCompileError> {
        let column = match field {
            Field::Assignee => "i.assignee_id",
            Field::Reporter => "i.reporter_id",
            _ => unreachable!("only user fields call user_clause"),
        };
        let mut placeholders = Vec::with_capacity(values.len());
        for value in values {
            match value {
                Value::Function(name) if name.eq_ignore_ascii_case("currentUser") => {
                    placeholders.push(self.uuid(self.current_user.as_uuid()));
                }
                Value::Function(name) => {
                    return Err(JqlCompileError(format!(
                        "unsupported JQL function '{name}'"
                    )));
                }
                Value::Text(value) => {
                    let uuid = Uuid::parse_str(value).map_err(|_| {
                        JqlCompileError(format!("{field:?} must be a user UUID or currentUser()"))
                    })?;
                    placeholders.push(self.uuid(uuid));
                }
            }
        }
        self.comparison_from_placeholders(column, operator, placeholders)
    }

    fn comparison(
        &mut self,
        column: &str,
        operator: BinaryOperator,
        values: &[Value],
        allow_contains: bool,
    ) -> Result<String, JqlCompileError> {
        self.comparison_with_placeholders(column, operator, values, |compiler, value| {
            compiler.text(value)
        })
        .and_then(|predicate| {
            if allow_contains
                || !matches!(
                    operator,
                    BinaryOperator::Contains | BinaryOperator::NotContains
                )
            {
                Ok(predicate)
            } else {
                Err(JqlCompileError(
                    "text operators are not valid for this field".into(),
                ))
            }
        })
    }

    fn comparison_with_placeholders<F>(
        &mut self,
        column: &str,
        operator: BinaryOperator,
        values: &[Value],
        mut parameter: F,
    ) -> Result<String, JqlCompileError>
    where
        F: FnMut(&mut Self, &Value) -> Result<String, JqlCompileError>,
    {
        let placeholders = values
            .iter()
            .map(|value| parameter(self, value))
            .collect::<Result<Vec<_>, _>>()?;
        self.comparison_from_placeholders(column, operator, placeholders)
    }

    fn comparison_from_placeholders(
        &self,
        column: &str,
        operator: BinaryOperator,
        placeholders: Vec<String>,
    ) -> Result<String, JqlCompileError> {
        let one_value = || {
            (placeholders.len() == 1)
                .then(|| placeholders[0].as_str())
                .ok_or_else(|| JqlCompileError("operator accepts exactly one value".into()))
        };
        let predicate = match operator {
            BinaryOperator::Equals => format!("{column} = {}", one_value()?),
            BinaryOperator::NotEquals => format!("{column} != {}", one_value()?),
            BinaryOperator::LessThan => format!("{column} < {}", one_value()?),
            BinaryOperator::LessThanOrEqual => format!("{column} <= {}", one_value()?),
            BinaryOperator::GreaterThan => format!("{column} > {}", one_value()?),
            BinaryOperator::GreaterThanOrEqual => format!("{column} >= {}", one_value()?),
            BinaryOperator::Contains => format!(
                "{column} ILIKE '%' || replace(replace({}, '%', '\\%'), '_', '\\_') || '%'",
                one_value()?
            ),
            BinaryOperator::NotContains => format!(
                "{column} NOT ILIKE '%' || replace(replace({}, '%', '\\%'), '_', '\\_') || '%'",
                one_value()?
            ),
            BinaryOperator::In => format!("{column} IN ({})", placeholders.join(", ")),
            BinaryOperator::NotIn => format!("{column} NOT IN ({})", placeholders.join(", ")),
        };
        Ok(format!("({predicate})"))
    }

    fn is_empty(&self, field: Field, negated: bool) -> Result<String, JqlCompileError> {
        let column = match field {
            Field::Assignee => "i.assignee_id",
            Field::Sprint => "i.sprint_id",
            Field::Description => "i.description",
            Field::DueDate => "i.due_date",
            _ => {
                return Err(JqlCompileError(format!(
                    "{field:?} does not support IS EMPTY"
                )));
            }
        };
        let operator = if negated { "IS NOT NULL" } else { "IS NULL" };
        Ok(format!("({column} {operator})"))
    }

    fn text(&mut self, value: &Value) -> Result<String, JqlCompileError> {
        let Value::Text(value) = value else {
            return Err(JqlCompileError(
                "function is not valid for this field".into(),
            ));
        };
        self.parameters.push(JqlParameter::Text(value.clone()));
        Ok(format!("${}", self.parameters.len()))
    }

    fn uuid(&mut self, value: Uuid) -> String {
        self.parameters.push(JqlParameter::Uuid(value));
        format!("${}", self.parameters.len())
    }
}

#[cfg(test)]
mod tests {
    use super::{JqlParameter, compile};
    use domain::jql::parse;
    use shared::UserId;
    use uuid::Uuid;

    #[test]
    fn compiles_nested_query_to_parameterized_sql() {
        let expression =
            parse("project = TT AND (priority IN (high, urgent) OR assignee = currentUser())")
                .expect("valid JQL");
        let user_id = UserId::from_uuid(
            Uuid::parse_str("11111111-1111-1111-1111-111111111111").expect("valid UUID"),
        );

        let query = compile(&expression, user_id).expect("supported query");

        assert_eq!(
            query.predicate,
            "((p.key = $1) AND ((i.priority IN ($2, $3)) OR (i.assignee_id = $4)))"
        );
        assert_eq!(
            query.parameters,
            vec![
                JqlParameter::Text("TT".into()),
                JqlParameter::Text("high".into()),
                JqlParameter::Text("urgent".into()),
                JqlParameter::Uuid(user_id.as_uuid()),
            ]
        );
    }

    #[test]
    fn compiles_status_and_text_without_interpolating_user_input() {
        let expression =
            parse("status = \"In Progress\" AND text ~ \"a' OR true --\"").expect("valid JQL");

        let query = compile(&expression, UserId::new()).expect("supported query");

        assert_eq!(
            query.predicate,
            "(EXISTS (SELECT 1 FROM statuses s WHERE s.id = i.status_id AND (s.name = $1)) AND (i.tsv_search @@ websearch_to_tsquery('simple', $2)))"
        );
        assert_eq!(
            query.parameters,
            vec![
                JqlParameter::Text("In Progress".into()),
                JqlParameter::Text("a' OR true --".into()),
            ]
        );
    }

    #[test]
    fn rejects_current_user_for_non_user_fields() {
        let expression = parse("project = currentUser()").expect("valid syntax");

        let error = compile(&expression, UserId::new()).expect_err("invalid semantic query");

        assert!(error.to_string().contains("function"));
    }

    #[test]
    fn compiles_is_empty_and_is_not_empty() {
        let expr = parse("assignee IS EMPTY").expect("valid JQL");
        let query = compile(&expr, UserId::new()).expect("compiles");
        assert_eq!(query.predicate, "(i.assignee_id IS NULL)");
        assert!(query.parameters.is_empty());

        let expr = parse("sprint IS NOT EMPTY").expect("valid JQL");
        let query = compile(&expr, UserId::new()).expect("compiles");
        assert_eq!(query.predicate, "(i.sprint_id IS NOT NULL)");
    }

    #[test]
    fn compiles_not_expression() {
        let expr = parse("NOT (project = TT)").expect("valid JQL");
        let query = compile(&expr, UserId::new()).expect("compiles");
        assert_eq!(query.predicate, "NOT ((p.key = $1))");
        assert_eq!(query.parameters, vec![JqlParameter::Text("TT".into())]);
    }

    #[test]
    fn compiles_labels_clause() {
        let expr = parse("labels = backend").expect("valid JQL");
        let query = compile(&expr, UserId::new()).expect("compiles");
        assert!(
            query
                .predicate
                .contains("EXISTS (SELECT 1 FROM issue_labels")
        );
        assert!(query.predicate.contains("l.name = $1"));
        assert_eq!(query.parameters, vec![JqlParameter::Text("backend".into())]);
    }

    #[test]
    fn compiles_sprint_clause() {
        let expr = parse("sprint = \"Sprint 1\"").expect("valid JQL");
        let query = compile(&expr, UserId::new()).expect("compiles");
        assert!(query.predicate.contains("EXISTS (SELECT 1 FROM sprints sp"));
        assert!(query.predicate.contains("sp.name = $1"));
    }

    #[test]
    fn compiles_status_category_clause() {
        let expr = parse("statusCategory = done").expect("valid JQL");
        let query = compile(&expr, UserId::new()).expect("compiles");
        assert!(query.predicate.contains(
            "EXISTS (SELECT 1 FROM statuses s WHERE s.id = i.status_id AND (s.category = $1))"
        ));
        assert_eq!(query.parameters, vec![JqlParameter::Text("done".into())]);
    }

    #[test]
    fn compiles_key_and_issue_type_clauses() {
        let expr = parse("key = TT-42 AND issueType = Bug").expect("valid JQL");
        let query = compile(&expr, UserId::new()).expect("compiles");
        assert!(query.predicate.contains("i.key = $1"));
        assert!(query.predicate.contains("i.issue_type = $2"));
    }

    #[test]
    fn compiles_timestamp_with_cast() {
        let expr = parse("created >= 2026-01-01").expect("valid JQL");
        let query = compile(&expr, UserId::new()).expect("compiles");
        assert!(query.predicate.contains("i.created_at >= $1::timestamptz"));
    }

    #[test]
    fn rejects_contains_on_date_field() {
        let expr = parse("created ~ \"text\"").expect("valid JQL syntax");
        let error = compile(&expr, UserId::new()).expect_err("date fields don't support ~");
        assert!(error.to_string().contains("date fields"));
    }

    #[test]
    fn rejects_is_empty_on_non_nullable_field() {
        let expr = parse("priority IS EMPTY").expect("valid JQL syntax");
        let error = compile(&expr, UserId::new()).expect_err("priority can't be empty");
        assert!(error.to_string().contains("IS EMPTY"));
    }

    #[test]
    fn compiles_in_and_not_in() {
        let expr = parse("priority IN (high, medium, low)").expect("valid JQL");
        let query = compile(&expr, UserId::new()).expect("compiles");
        assert!(query.predicate.contains("i.priority IN ($1, $2, $3)"));
        assert_eq!(query.parameters.len(), 3);

        let expr = parse("priority NOT IN (high, urgent)").expect("valid JQL");
        let query = compile(&expr, UserId::new()).expect("compiles");
        assert!(query.predicate.contains("i.priority NOT IN ($1, $2)"));
    }

    #[test]
    fn compiles_reporter_with_uuid() {
        let expr = parse("reporter = 22222222-2222-2222-2222-222222222222").expect("valid JQL");
        let query = compile(&expr, UserId::new()).expect("compiles");
        assert!(query.predicate.contains("i.reporter_id = $1"));
        assert_eq!(
            query.parameters,
            vec![JqlParameter::Uuid(
                Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap()
            )]
        );
    }

    #[test]
    fn rejects_invalid_uuid_for_user_field() {
        let expr = parse("assignee = not-a-uuid").expect("valid JQL syntax");
        let error = compile(&expr, UserId::new()).expect_err("invalid UUID");
        assert!(error.to_string().contains("UUID"));
    }
}
#[cfg(test)]
mod wildcard_tests {
    use crate::jql::{JqlParameter, compile};
    use domain::jql::parse;
    use shared::UserId;

    #[test]
    fn contains_operator_escapes_like_wildcards_in_bound_value() {
        let expression = parse("summary ~ \"100%\"").expect("valid JQL");
        let query = compile(&expression, UserId::new()).expect("supported query");
        // The predicate must neutralize LIKE metacharacters inside the bound parameter.
        assert!(
            query.predicate.contains("replace("),
            "expected like_escape/replace wrapping, got: {}",
            query.predicate
        );
        let params: Vec<&JqlParameter> = query.parameters.iter().collect();
        assert!(!params.is_empty());
    }
}
