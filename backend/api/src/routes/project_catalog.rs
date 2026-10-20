//! Human project catalog: space visibility/search/count are applied before pagination.
use super::projects::map_project_response;
use crate::{dto::ProjectResponse, middleware::auth::UserClaims};
use axum::{
    Extension,
    extract::{Query, State},
    response::Response,
};
use shared::{AppError, UserId, space_access::*};
use std::sync::Arc;

#[derive(serde::Deserialize, utoipa::IntoParams)]
#[serde(deny_unknown_fields)]
pub struct VisibleProjectQuery {
    #[param(minimum = 1, maximum = 100)]
    pub limit: Option<u64>,
    pub offset: Option<u64>,
    #[param(max_length = 200)]
    pub q: Option<String>,
    pub project_key: Option<String>,
    pub registry_instance_id: Option<uuid::Uuid>,
    pub space_id: Option<uuid::Uuid>,
}
impl VisibleProjectQuery {
    fn checked(
        &self,
    ) -> Result<
        (
            domain::project_spaces::ProjectCatalogQuery,
            Option<SpaceRef>,
        ),
        AppError,
    > {
        if self.q.as_ref().is_some_and(|q| q.chars().count() > 200) {
            return Err(AppError::invalid_input("invalid_project_catalog_search"));
        }
        let query = domain::project_spaces::ProjectCatalogQuery {
            limit: self.limit.unwrap_or(50),
            offset: self.offset.unwrap_or(0),
            search: self.q.as_deref().unwrap_or("").trim().into(),
            project_key: self.project_key.as_deref().map(shared::ProjectKey::new),
        };
        query.validate()?;
        let space = match (self.registry_instance_id, self.space_id) {
            (None, None) => None,
            (Some(registry_instance_id), Some(space_id)) => {
                let space = SpaceRef {
                    registry_instance_id,
                    space_id,
                };
                if !space.valid() {
                    return Err(AppError::invalid_input("invalid_space_context"));
                }
                Some(space)
            }
            _ => return Err(AppError::invalid_input("incomplete_space_context")),
        };
        Ok((query, space))
    }
}
#[derive(serde::Serialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct VisibleProjectCatalogV2 {
    pub schema_version: u32,
    pub tracker_instance_id: uuid::Uuid,
    pub identity: CentralIdentity,
    pub policy_revision: i64,
    pub validity: AdmissionValidity,
    pub space: Option<SpaceRef>,
    pub total: u64,
    pub offset: u64,
    pub limit: u64,
    pub has_more: bool,
    pub projects: Vec<ProjectResponse>,
}
#[utoipa::path(operation_id="get_visible_projects_v2",get,path="/api/v2/projects",
params(VisibleProjectQuery),
responses((status=200,body=VisibleProjectCatalogV2),(status=400,description="Invalid bounds or explicit space context"),
(status=401,description="Central human required"),(status=403,description="Project or selected space reading denied"),
(status=503,description="Current owner confirmation unavailable")),security(("bearer"=[])))]
pub async fn visible(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(claims): Extension<UserClaims>,
    headers: axum::http::HeaderMap,
    Query(query): Query<VisibleProjectQuery>,
) -> Result<Response, AppError> {
    let (query, space) = query.checked()?;
    let user = claims
        .sub
        .parse::<UserId>()
        .map_err(|_| AppError::Unauthorized)?;
    let frame = app::space_access::current_request_for(user)?;
    let actor = if let Some(actor) = frame.as_ref().and_then(|f| f.spaces()) {
        actor.clone()
    } else {
        let refs = ctx.repos.projects.space_catalog_refs().await?;
        super::project_spaces::actor(&ctx, &headers, &claims, "GET", &refs).await?
    };
    let (projects, total) = ctx
        .services
        .project
        .visible_catalog(query.clone(), space.as_ref(), &actor)
        .await?;
    let value = VisibleProjectCatalogV2 {
        schema_version: 2,
        tracker_instance_id: super::project_contexts::instance()?,
        identity: actor.identity().clone(),
        policy_revision: actor.policy_revision(),
        validity: actor.read_validity()?,
        space,
        total,
        offset: query.offset,
        limit: query.limit,
        has_more: query.offset.saturating_add(projects.len() as u64) < total,
        projects: projects.into_iter().map(map_project_response).collect(),
    };
    super::project_spaces::no_store(value, &actor)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn context_is_an_explicit_pair_and_page_bounds_are_not_clamped() {
        for value in [
            serde_json::json!({"space_id":uuid::Uuid::new_v4()}),
            serde_json::json!({"registry_instance_id":uuid::Uuid::nil(),"space_id":uuid::Uuid::new_v4()}),
            serde_json::json!({"limit":0}),
            serde_json::json!({"limit":101}),
            serde_json::json!({"offset":u64::MAX}),
            serde_json::json!({"q":"я".repeat(201)}),
        ] {
            assert!(
                serde_json::from_value::<VisibleProjectQuery>(value)
                    .unwrap()
                    .checked()
                    .is_err()
            );
        }
        assert!(
            serde_json::from_value::<VisibleProjectQuery>(
                serde_json::json!({"namespace_id":"legacy"})
            )
            .is_err()
        );
        let (query, space) =
            serde_json::from_value::<VisibleProjectQuery>(serde_json::json!({"q":"  %_ '  "}))
                .unwrap()
                .checked()
                .unwrap();
        assert_eq!(query.search, "%_ '");
        assert!(space.is_none());
        assert_eq!(query.limit, 50);
    }
}
