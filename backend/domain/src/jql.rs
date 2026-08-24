use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Expr {
    Clause {
        field: Field,
        operator: BinaryOperator,
        values: Vec<Value>,
    },
    IsEmpty {
        field: Field,
        negated: bool,
    },
    Not(Box<Expr>),
    And(Box<Expr>, Box<Expr>),
    Or(Box<Expr>, Box<Expr>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Field {
    Key,
    Summary,
    Description,
    Text,
    Project,
    ProjectKey,
    Status,
    StatusCategory,
    IssueType,
    Assignee,
    Reporter,
    Priority,
    Labels,
    Sprint,
    Created,
    Updated,
    DueDate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum BinaryOperator {
    Equals,
    NotEquals,
    LessThan,
    LessThanOrEqual,
    GreaterThan,
    GreaterThanOrEqual,
    Contains,
    NotContains,
    In,
    NotIn,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Value {
    Text(String),
    Function(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub position: usize,
    pub message: String,
}

impl fmt::Display for ParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "JQL parse error at {}: {}",
            self.position, self.message
        )
    }
}

impl std::error::Error for ParseError {}

#[derive(Debug, Clone, PartialEq, Eq)]
enum TokenKind {
    Word(String),
    Quoted(String),
    Equals,
    NotEquals,
    LessThan,
    LessThanOrEqual,
    GreaterThan,
    GreaterThanOrEqual,
    Contains,
    NotContains,
    LeftParen,
    RightParen,
    Comma,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Token {
    kind: TokenKind,
    position: usize,
}

pub fn parse(input: &str) -> Result<Expr, ParseError> {
    let tokens = lex(input)?;
    let mut parser = Parser { tokens, cursor: 0 };
    let expression = parser.parse_or()?;
    if let Some(token) = parser.peek() {
        return Err(parser.error(token.position, "unexpected token"));
    }
    Ok(expression)
}

fn lex(input: &str) -> Result<Vec<Token>, ParseError> {
    let mut tokens = Vec::new();
    let mut cursor = 0;
    let bytes = input.as_bytes();

    while cursor < bytes.len() {
        if bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
            continue;
        }

        let position = cursor;
        let kind = match bytes[cursor] {
            b'(' => {
                cursor += 1;
                TokenKind::LeftParen
            }
            b')' => {
                cursor += 1;
                TokenKind::RightParen
            }
            b',' => {
                cursor += 1;
                TokenKind::Comma
            }
            b'=' => {
                cursor += 1;
                TokenKind::Equals
            }
            b'~' => {
                cursor += 1;
                TokenKind::Contains
            }
            b'!' => {
                cursor += 1;
                match bytes.get(cursor) {
                    Some(b'=') => {
                        cursor += 1;
                        TokenKind::NotEquals
                    }
                    Some(b'~') => {
                        cursor += 1;
                        TokenKind::NotContains
                    }
                    _ => {
                        return Err(ParseError {
                            position,
                            message: "expected '=' or '~' after '!'".into(),
                        });
                    }
                }
            }
            b'<' => {
                cursor += 1;
                if bytes.get(cursor) == Some(&b'=') {
                    cursor += 1;
                    TokenKind::LessThanOrEqual
                } else {
                    TokenKind::LessThan
                }
            }
            b'>' => {
                cursor += 1;
                if bytes.get(cursor) == Some(&b'=') {
                    cursor += 1;
                    TokenKind::GreaterThanOrEqual
                } else {
                    TokenKind::GreaterThan
                }
            }
            b'"' => {
                cursor += 1;
                let start = cursor;
                let mut value = String::new();
                let mut closed = false;
                while cursor < bytes.len() {
                    match bytes[cursor] {
                        b'\\' if bytes.get(cursor + 1) == Some(&b'"') => {
                            value.push_str(&input[start..cursor]);
                            value.push('"');
                            cursor += 2;
                            let remainder = cursor;
                            while cursor < bytes.len() && bytes[cursor] != b'"' {
                                if bytes[cursor] == b'\\' && bytes.get(cursor + 1) == Some(&b'"') {
                                    value.push_str(&input[remainder..cursor]);
                                    value.push('"');
                                    cursor += 2;
                                    break;
                                }
                                cursor += 1;
                            }
                        }
                        b'"' => {
                            value.push_str(&input[start..cursor]);
                            cursor += 1;
                            closed = true;
                            break;
                        }
                        _ => cursor += 1,
                    }
                }
                if !closed {
                    return Err(ParseError {
                        position,
                        message: "unterminated quoted value".into(),
                    });
                }
                TokenKind::Quoted(value)
            }
            _ => {
                let start = cursor;
                while cursor < bytes.len()
                    && !bytes[cursor].is_ascii_whitespace()
                    && !matches!(
                        bytes[cursor],
                        b'(' | b')' | b',' | b'=' | b'!' | b'~' | b'<' | b'>'
                    )
                {
                    cursor += 1;
                }
                if start == cursor {
                    return Err(ParseError {
                        position,
                        message: "unexpected character".into(),
                    });
                }
                TokenKind::Word(input[start..cursor].to_string())
            }
        };
        tokens.push(Token { kind, position });
    }

    Ok(tokens)
}

struct Parser {
    tokens: Vec<Token>,
    cursor: usize,
}

impl Parser {
    fn parse_or(&mut self) -> Result<Expr, ParseError> {
        let mut expression = self.parse_and()?;
        while self.consume_keyword("OR") {
            expression = Expr::Or(Box::new(expression), Box::new(self.parse_and()?));
        }
        Ok(expression)
    }

    fn parse_and(&mut self) -> Result<Expr, ParseError> {
        let mut expression = self.parse_unary()?;
        while self.consume_keyword("AND") {
            expression = Expr::And(Box::new(expression), Box::new(self.parse_unary()?));
        }
        Ok(expression)
    }

    fn parse_unary(&mut self) -> Result<Expr, ParseError> {
        if self.consume_keyword("NOT") {
            return Ok(Expr::Not(Box::new(self.parse_unary()?)));
        }
        if self.consume_kind(&TokenKind::LeftParen) {
            let expression = self.parse_or()?;
            self.expect_kind(&TokenKind::RightParen, "expected ')' to close expression")?;
            return Ok(expression);
        }
        self.parse_clause()
    }

    fn parse_clause(&mut self) -> Result<Expr, ParseError> {
        let field_token = self
            .next()
            .ok_or_else(|| self.error_at_end("expected field"))?;
        let field_name = match &field_token.kind {
            TokenKind::Word(value) | TokenKind::Quoted(value) => value,
            _ => return Err(self.error(field_token.position, "expected field")),
        };
        let field = Field::parse(field_name).ok_or_else(|| {
            self.error(
                field_token.position,
                format!("unknown field '{field_name}'"),
            )
        })?;

        if self.consume_keyword("IS") {
            let negated = self.consume_keyword("NOT");
            self.expect_keyword("EMPTY", "expected EMPTY after IS")?;
            return Ok(Expr::IsEmpty { field, negated });
        }

        let operator = if self.consume_keyword("NOT") {
            self.expect_keyword("IN", "expected IN after NOT")?;
            BinaryOperator::NotIn
        } else if self.consume_keyword("IN") {
            BinaryOperator::In
        } else {
            let token = self
                .next()
                .ok_or_else(|| self.error_at_end("expected operator"))?;
            match token.kind {
                TokenKind::Equals => BinaryOperator::Equals,
                TokenKind::NotEquals => BinaryOperator::NotEquals,
                TokenKind::LessThan => BinaryOperator::LessThan,
                TokenKind::LessThanOrEqual => BinaryOperator::LessThanOrEqual,
                TokenKind::GreaterThan => BinaryOperator::GreaterThan,
                TokenKind::GreaterThanOrEqual => BinaryOperator::GreaterThanOrEqual,
                TokenKind::Contains => BinaryOperator::Contains,
                TokenKind::NotContains => BinaryOperator::NotContains,
                _ => return Err(self.error(token.position, "expected comparison operator")),
            }
        };

        let values = if matches!(operator, BinaryOperator::In | BinaryOperator::NotIn) {
            self.parse_list()?
        } else {
            vec![self.parse_value()?]
        };
        Ok(Expr::Clause {
            field,
            operator,
            values,
        })
    }

    fn parse_list(&mut self) -> Result<Vec<Value>, ParseError> {
        self.expect_kind(&TokenKind::LeftParen, "expected '(' after IN")?;
        let mut values = vec![self.parse_value()?];
        while self.consume_kind(&TokenKind::Comma) {
            values.push(self.parse_value()?);
        }
        self.expect_kind(&TokenKind::RightParen, "expected ')' after value list")?;
        Ok(values)
    }

    fn parse_value(&mut self) -> Result<Value, ParseError> {
        let token = self
            .next()
            .ok_or_else(|| self.error_at_end("expected value"))?;
        let value = match token.kind {
            TokenKind::Quoted(value) => Value::Text(value),
            TokenKind::Word(value) => {
                if self.consume_kind(&TokenKind::LeftParen) {
                    self.expect_kind(
                        &TokenKind::RightParen,
                        "only zero-argument functions are supported",
                    )?;
                    Value::Function(value)
                } else {
                    Value::Text(value)
                }
            }
            _ => return Err(self.error(token.position, "expected value")),
        };
        Ok(value)
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.cursor)
    }

    fn next(&mut self) -> Option<Token> {
        let token = self.tokens.get(self.cursor).cloned();
        self.cursor += usize::from(token.is_some());
        token
    }

    fn consume_keyword(&mut self, keyword: &str) -> bool {
        let Some(Token {
            kind: TokenKind::Word(value),
            ..
        }) = self.peek()
        else {
            return false;
        };
        if value.eq_ignore_ascii_case(keyword) {
            self.cursor += 1;
            true
        } else {
            false
        }
    }

    fn expect_keyword(&mut self, keyword: &str, message: &str) -> Result<(), ParseError> {
        if self.consume_keyword(keyword) {
            Ok(())
        } else {
            Err(self.error_at_current(message))
        }
    }

    fn consume_kind(&mut self, expected: &TokenKind) -> bool {
        if self.peek().is_some_and(|token| &token.kind == expected) {
            self.cursor += 1;
            true
        } else {
            false
        }
    }

    fn expect_kind(&mut self, expected: &TokenKind, message: &str) -> Result<(), ParseError> {
        if self.consume_kind(expected) {
            Ok(())
        } else {
            Err(self.error_at_current(message))
        }
    }

    fn error_at_current(&self, message: impl Into<String>) -> ParseError {
        let message = message.into();
        if let Some(token) = self.peek() {
            self.error(token.position, message)
        } else {
            self.error_at_end(message)
        }
    }

    fn error_at_end(&self, message: impl Into<String>) -> ParseError {
        ParseError {
            position: self.tokens.last().map_or(0, |token| token.position + 1),
            message: message.into(),
        }
    }

    fn error(&self, position: usize, message: impl Into<String>) -> ParseError {
        ParseError {
            position,
            message: message.into(),
        }
    }
}

impl Field {
    fn parse(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().as_str() {
            "key" => Some(Self::Key),
            "summary" => Some(Self::Summary),
            "description" => Some(Self::Description),
            "text" => Some(Self::Text),
            "project" => Some(Self::Project),
            "projectkey" => Some(Self::ProjectKey),
            "status" => Some(Self::Status),
            "statuscategory" => Some(Self::StatusCategory),
            "issuetype" => Some(Self::IssueType),
            "assignee" => Some(Self::Assignee),
            "reporter" => Some(Self::Reporter),
            "priority" => Some(Self::Priority),
            "labels" => Some(Self::Labels),
            "sprint" => Some(Self::Sprint),
            "created" => Some(Self::Created),
            "updated" => Some(Self::Updated),
            "duedate" => Some(Self::DueDate),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
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
}
