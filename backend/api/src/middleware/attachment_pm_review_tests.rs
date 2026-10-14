use super::*;
use axum::{
    http::{StatusCode, header},
    response::IntoResponse,
};

#[test]
fn pm_download_success_and_denials_never_cache() {
    for result in [
        Ok(StatusCode::OK.into_response()),
        Err(AppError::Forbidden),
        Err(AppError::conflict("synthetic changed scope")),
        Err(AppError::Unavailable("synthetic disabled".into())),
    ] {
        let response = pm_no_store(result);
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    }
}
