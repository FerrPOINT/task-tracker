#[cfg(test)]
mod tests {
    use crate::{AppError, Timestamp, now};

    #[test]
    fn error_constructors() {
        let e = AppError::not_found("issue", "TT-15");
        assert!(matches!(e, AppError::NotFound(_)));
        assert_eq!(e.to_string(), "not found: issue TT-15 not found");

        let e = AppError::invalid_input("bad request");
        assert!(matches!(e, AppError::InvalidInput(_)));

        let e = AppError::conflict("duplicate");
        assert!(matches!(e, AppError::Conflict(_)));

        let e = AppError::database("pg timeout");
        assert!(matches!(e, AppError::Database(_)));

        let e = AppError::internal("boom");
        assert!(matches!(e, AppError::Internal(_)));
    }

    #[test]
    fn now_returns_timestamp() {
        let t: Timestamp = now();
        assert!(t.timestamp() > 1_700_000_000);
    }
}
