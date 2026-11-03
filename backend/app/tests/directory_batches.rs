//! Public issue-list boundary: sparse directory enrichment must be batched and fail truthfully.
use domain::{IssueRepository, ProjectRepository, UserRepository};
use shared::{AppError, UserId};
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};
#[derive(Default)]
struct Users {
    rows: Mutex<HashMap<UserId, domain::User>>,
    batches: AtomicUsize,
    points: AtomicUsize,
    fail: AtomicBool,
    wanted: Mutex<Vec<UserId>>,
}
#[async_trait::async_trait]
impl UserRepository for Users {
    async fn get_by_id(&self, id: UserId) -> Result<domain::User, AppError> {
        self.points.fetch_add(1, Ordering::SeqCst);
        if self.fail.load(Ordering::SeqCst) {
            return Err(AppError::Unavailable("directory unavailable".into()));
        }
        self.rows
            .lock()
            .unwrap()
            .get(&id)
            .cloned()
            .ok_or_else(|| AppError::not_found("user", id))
    }
    async fn get_by_ids(&self, ids: &[UserId]) -> Result<Vec<domain::User>, AppError> {
        self.batches.fetch_add(1, Ordering::SeqCst);
        *self.wanted.lock().unwrap() = ids.to_vec();
        if self.fail.load(Ordering::SeqCst) {
            return Err(AppError::Unavailable("directory unavailable".into()));
        }
        let rows = self.rows.lock().unwrap();
        Ok(ids.iter().filter_map(|id| rows.get(id).cloned()).collect())
    }
    async fn get_by_email(&self, _: &str) -> Result<domain::User, AppError> {
        Err(AppError::Unauthorized)
    }
    async fn get_by_refresh_token(&self, _: &str) -> Result<domain::User, AppError> {
        Err(AppError::Unauthorized)
    }
    async fn rotate_refresh_token(&self, _: UserId, _: &str, _: &str) -> Result<(), AppError> {
        Err(AppError::Unauthorized)
    }
    async fn clear_refresh_token(&self, _: UserId) -> Result<(), AppError> {
        Ok(())
    }
    async fn save(&self, user: &domain::User) -> Result<UserId, AppError> {
        self.rows.lock().unwrap().insert(user.id, user.clone());
        Ok(user.id)
    }
    async fn list(&self) -> Result<Vec<domain::User>, AppError> {
        panic!("an issue page must not read the whole user directory")
    }
}
#[tokio::test]
async fn issue_page_uses_one_sparse_batch_and_propagates_storage_failure() {
    let users = Arc::new(Users::default());
    let now = shared::now();
    let owner = UserId::new();
    let missing = UserId::new();
    users
        .save(&domain::User {
            id: owner,
            email: "batch@example.test".into(),
            username: "batch".into(),
            display_name: "Batch".into(),
            password_hash: "!".into(),
            refresh_token_hash: None,
            is_system_admin: false,
            is_active: true,
            created_at: now,
            updated_at: now,
        })
        .await
        .unwrap();
    let projects = Arc::new(domain::MemoryProjectRepository::default());
    let project = domain::Project {
        id: shared::ProjectId::new(),
        key: shared::ProjectKey::new("BATCH"),
        name: "Batch".into(),
        description: None,
        owner_id: owner,
        default_board_id: shared::BoardId::new(),
        created_at: now,
        updated_at: now,
    };
    projects.save(&project).await.unwrap();
    let issues = Arc::new(domain::MemoryIssueRepository::default());
    for number in 1..=30 {
        let mut issue = domain::Issue::create(
            &project,
            number,
            shared::IssueType::Task,
            shared::StatusId::new(),
            "Batch issue",
            None,
            owner,
            shared::Priority::Medium,
        );
        issue.assignee_id = Some(missing);
        issues.save(&issue).await.unwrap();
    }
    let repos = Arc::new(domain::Repositories {
        users: users.clone(),
        projects,
        issues,
        ..Default::default()
    });
    let ctx = app::AppContext::new(
        Arc::new(shared::AppConfig::default()),
        repos,
        Arc::new(domain::InMemoryStorage::default()),
    );
    let page = ctx
        .services
        .issue
        .search(Default::default(), owner)
        .await
        .unwrap();
    assert_eq!(page.len(), 30);
    assert!(
        page.iter()
            .all(|issue| issue.reporter_name.as_deref() == Some("Batch")
                && issue.assignee_name.is_none())
    );
    assert_eq!(users.batches.load(Ordering::SeqCst), 1);
    assert_eq!(users.points.load(Ordering::SeqCst), 0);
    let mut wanted = users.wanted.lock().unwrap().clone();
    wanted.sort();
    let mut expected = vec![owner, missing];
    expected.sort();
    assert_eq!(wanted, expected);
    users.fail.store(true, Ordering::SeqCst);
    assert!(
        matches!(
            ctx.services.issue.search(Default::default(), owner).await,
            Err(AppError::Unavailable(_))
        ),
        "directory storage failure must not become an absent author"
    );
}
