use async_trait::async_trait;
use std::str::FromStr;
use std::sync::Arc;

use crate::commands::{
    CreateCommentCommand, CreateIssueCommand, CreateWorklogCommand, UpdateCommentCommand,
    UpdateIssueCommand, UpdateWorklogCommand,
};
use crate::context::SearchFilters;
use crate::dto::{
    BacklogDto, BoardColumnDto, BoardDto, CommentDto, DashboardDto, IssueDto, ProjectDto,
    ProjectMemberDto, SprintDto, WorklogDto,
};
use domain::{
    Board, BoardRepository, Issue, IssueQuery, IssueRepository, IssueTypeEntity,
    IssueTypeRepository, ProjectMember, ProjectMemberRepository, ProjectRepository, ProjectRole,
    SprintRepository, StatusCategory, StatusRepository, UserRepository, WorkflowTransition,
    WorkflowTransitionRepository,
};
use shared::{
    AppError, BoardId, IssueId, IssueKey, ProjectId, ProjectKey, SprintId, StatusId, UserId,
};

mod helpers;

pub mod admin;
pub use admin::AdminServiceImpl;

#[cfg(test)]
mod tests;

pub struct ProjectServiceImpl {
    projects: Arc<dyn ProjectRepository>,
    issues: Arc<dyn IssueRepository>,
    users: Arc<dyn domain::UserRepository>,
    boards: Arc<dyn domain::BoardRepository>,
}

impl ProjectServiceImpl {
    pub fn new(
        projects: Arc<dyn ProjectRepository>,
        issues: Arc<dyn IssueRepository>,
        users: Arc<dyn domain::UserRepository>,
        boards: Arc<dyn domain::BoardRepository>,
    ) -> Self {
        Self {
            projects,
            issues,
            users,
            boards,
        }
    }
}

#[async_trait]
impl crate::context::ProjectService for ProjectServiceImpl {
    async fn create(
        &self,
        cmd: crate::commands::CreateProjectCommand,
    ) -> Result<ProjectDto, AppError> {
        let owner = self.users.get_by_id(cmd.owner_id).await?;
        let board_id = BoardId::new();
        let project = domain::Project {
            id: ProjectId::new(),
            key: cmd.key,
            name: cmd.name.into(),
            description: cmd.description.map(Into::into),
            owner_id: owner.id,
            default_board_id: board_id,
            created_at: shared::now(),
            updated_at: shared::now(),
        };
        let board = Board {
            id: board_id,
            project_id: project.id,
            name: "Board".into(),
            columns: helpers::default_board_columns(),
        };
        self.projects.save(&project).await?;
        self.boards.save(&board).await?;
        Ok(ProjectDto::from_project(project, 0, 0, 0))
    }

    async fn list(
        &self,
        _query: crate::commands::ProjectQueryDto,
    ) -> Result<Vec<ProjectDto>, AppError> {
        let projects = self
            .projects
            .list(domain::ProjectQuery {
                owner_id: None,
                limit: 100,
                offset: 0,
            })
            .await?;
        let mut dtos = Vec::new();
        for project in projects {
            let counts = self.issues.list(IssueQuery::project(project.id)).await?;
            let (todo, in_progress, done) = helpers::count_by_status(&counts);
            dtos.push(ProjectDto::from_project(project, todo, in_progress, done));
        }
        Ok(dtos)
    }

    async fn get_by_key(&self, key: &ProjectKey) -> Result<ProjectDto, AppError> {
        let project = self.projects.get_by_key(key).await?;
        let counts = self.issues.list(IssueQuery::project(project.id)).await?;
        let (todo, in_progress, done) = helpers::count_by_status(&counts);
        Ok(ProjectDto::from_project(project, todo, in_progress, done))
    }

    async fn update(
        &self,
        key: &ProjectKey,
        cmd: crate::commands::UpdateProjectCommand,
        requester_id: UserId,
    ) -> Result<ProjectDto, AppError> {
        let mut project = self.projects.get_by_key(key).await?;
        if project.owner_id != requester_id {
            return Err(AppError::Forbidden);
        }
        if let Some(name) = cmd.name {
            project.name = name.into();
            project.updated_at = shared::now();
        }
        if let Some(description) = cmd.description {
            project.description = description.map(Into::into);
            project.updated_at = shared::now();
        }
        self.projects.save(&project).await?;
        let counts = self.issues.list(IssueQuery::project(project.id)).await?;
        let (todo, in_progress, done) = helpers::count_by_status(&counts);
        Ok(ProjectDto::from_project(project, todo, in_progress, done))
    }

    async fn delete(&self, key: &ProjectKey, requester_id: UserId) -> Result<(), AppError> {
        let project = self.projects.get_by_key(key).await?;
        if project.owner_id != requester_id {
            return Err(AppError::Forbidden);
        }
        self.projects.delete(project.id).await
    }
}

pub struct IssueServiceImpl {
    issues: Arc<dyn IssueRepository>,
    projects: Arc<dyn ProjectRepository>,
    boards: Arc<dyn BoardRepository>,
    users: Arc<dyn domain::UserRepository>,
    statuses: Arc<dyn StatusRepository>,
    transitions: Arc<dyn WorkflowTransitionRepository>,
    events: crate::context::EventBus,
    notifications: Arc<dyn domain::NotificationRepository>,
}

impl IssueServiceImpl {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        issues: Arc<dyn IssueRepository>,
        projects: Arc<dyn ProjectRepository>,
        boards: Arc<dyn BoardRepository>,
        users: Arc<dyn domain::UserRepository>,
        statuses: Arc<dyn StatusRepository>,
        transitions: Arc<dyn WorkflowTransitionRepository>,
        events: crate::context::EventBus,
        notifications: Arc<dyn domain::NotificationRepository>,
    ) -> Self {
        Self {
            issues,
            projects,
            events,
            boards,
            users,
            statuses,
            transitions,
            notifications,
        }
    }

    /// Create a notification and publish a real-time SSE event.
    async fn create_notification(&self, notification: domain::Notification) {
        let recipient_id = notification.recipient_id;
        if let Ok(_id) = self.notifications.save(&notification).await {
            self.events
                .publish(shared::TrackerEvent::NotificationCreated {
                    recipient_id: recipient_id.to_string(),
                });
        }
    }
}

#[async_trait]
impl crate::context::IssueService for IssueServiceImpl {
    async fn create(&self, cmd: CreateIssueCommand) -> Result<IssueDto, AppError> {
        let project = self.projects.get_by_key(&cmd.project_key).await?;
        let status_id = StatusId::from_uuid(
            cmd.status_id
                .parse()
                .map_err(|_| AppError::invalid_input("status_id"))?,
        );
        // Retry on key conflicts: concurrent creators may compute the same next number.
        let mut issue = None;
        for _ in 0..5 {
            let number = self.projects.next_issue_number(project.id).await?;
            let mut candidate = Issue::create(
                &project,
                number,
                cmd.issue_type,
                status_id,
                cmd.summary.clone(),
                cmd.description.clone().map(domain::RichText::from),
                cmd.reporter_id,
                cmd.priority,
            );
            if let Some(assignee_id) = cmd.assignee_id {
                candidate.assign(Some(assignee_id));
            }
            match self.issues.save(&candidate).await {
                Ok(_) => {
                    issue = Some(candidate);
                    break;
                }
                Err(AppError::Database(msg)) if msg.contains("issues_key_key") => continue,
                Err(e) => return Err(e),
            }
        }
        let issue = issue.ok_or_else(|| {
            AppError::conflict("could not allocate a unique issue key, try again")
        })?;
        let statuses = self.statuses.list_all().await.unwrap_or_default();
        let column = statuses
            .iter()
            .find(|s| s.id == issue.status_id)
            .map(|s| s.name.as_ref().to_string())
            .unwrap_or_else(|| helpers::issue_status_column(issue.status_id));
        let (assignee_name, reporter_name) =
            helpers::resolve_names(self.users.clone(), &issue).await;
        self.events.publish(shared::TrackerEvent::IssueCreated {
            issue_id: issue.id.to_string(),
            project_key: project.key.to_string(),
        });
        // Notify assignee if assigned and not the reporter
        if let Some(assignee_id) = issue.assignee_id {
            if assignee_id != cmd.reporter_id {
                let key = issue.key.to_string();
                self.create_notification(domain::Notification {
                    id: shared::NotificationId::new(),
                    recipient_id: assignee_id,
                    event_type: "issue_assigned".into(),
                    entity_type: "issue".into(),
                    entity_id: Some(issue.id.as_uuid()),
                    actor_id: Some(cmd.reporter_id),
                    title: format!("You were assigned to {}", key).into(),
                    body: Some(issue.summary.as_ref().to_string().into()),
                    is_read: false,
                    read_at: None,
                    action_url: Some(
                        format!("/projects/{}/issues/{}", project.key, issue.id).into(),
                    ),
                    metadata: serde_json::json!({"issue_key": key}),
                    created_at: shared::now(),
                })
                .await;
            }
        }
        Ok(IssueDto::from_issue(
            issue,
            project.name.as_ref().to_string(),
            column,
            assignee_name,
            reporter_name,
        ))
    }

    async fn transition(
        &self,
        cmd: crate::commands::TransitionIssueCommand,
    ) -> Result<IssueDto, AppError> {
        let issue = self.issues.get_by_id(cmd.issue_id).await?;
        let board = self.boards.get_default_by_project(issue.project_id).await?;
        let statuses = self.statuses.list_all().await.unwrap_or_default();
        let valid = statuses.iter().any(|s| s.id == cmd.target_status_id)
            || board.columns.iter().any(|c| c.id == cmd.target_status_id);
        if !valid {
            return Err(AppError::invalid_input("invalid target status"));
        }
        let allowed = self
            .transitions
            .is_allowed(issue.status_id, cmd.target_status_id)
            .await?;
        if !allowed {
            return Err(AppError::invalid_input("workflow transition not allowed"));
        }
        let mut updated = issue.clone();
        updated.status_id = cmd.target_status_id;
        updated.updated_at = shared::now();
        self.issues.save(&updated).await?;
        let project = self.projects.get_by_id(updated.project_id).await?;
        let status = statuses
            .iter()
            .find(|s| s.id == updated.status_id)
            .map(|s| s.name.as_ref().to_string())
            .unwrap_or_else(|| {
                board
                    .columns
                    .iter()
                    .find(|c| c.id == updated.status_id)
                    .map(|c| c.name.as_ref().to_string())
                    .unwrap_or_default()
            });
        let (assignee_name, reporter_name) =
            helpers::resolve_names(self.users.clone(), &updated).await;
        self.events.publish(shared::TrackerEvent::IssueMoved {
            issue_id: updated.id.to_string(),
            project_key: project.key.to_string(),
        });
        // Notify reporter of status change
        if updated.reporter_id != cmd.actor_id {
            let key = updated.key.to_string();
            self.create_notification(domain::Notification {
                id: shared::NotificationId::new(),
                recipient_id: updated.reporter_id,
                event_type: "issue_moved".into(),
                entity_type: "issue".into(),
                entity_id: Some(updated.id.as_uuid()),
                actor_id: Some(cmd.actor_id),
                title: format!("{} moved to {}", key, status).into(),
                body: None,
                is_read: false,
                read_at: None,
                action_url: Some(format!("/projects/{}/issues/{}", project.key, updated.id).into()),
                metadata: serde_json::json!({"issue_key": key, "status": status}),
                created_at: shared::now(),
            })
            .await;
        }
        Ok(IssueDto::from_issue(
            updated,
            project.name.as_ref().to_string(),
            status,
            assignee_name,
            reporter_name,
        ))
    }

    async fn get_by_id(&self, id: IssueId) -> Result<IssueDto, AppError> {
        let issue = self.issues.get_by_id(id).await?;
        let name = helpers::project_name(self.projects.clone(), issue.project_id).await?;
        Ok(helpers::build_issue_dto(self.users.clone(), issue, name.as_str()).await)
    }

    async fn update(&self, id: IssueId, cmd: UpdateIssueCommand) -> Result<IssueDto, AppError> {
        let mut issue = self.issues.get_by_id(id).await?;
        let project = self.projects.get_by_id(issue.project_id).await?;

        if let Some(summary) = cmd.summary {
            issue.summary = summary.into();
            issue.updated_at = shared::now();
        }
        if let Some(description) = cmd.description {
            issue.description = description.map(domain::RichText::from);
            issue.updated_at = shared::now();
        }
        if let Some(priority) = cmd.priority {
            issue.priority = priority;
            issue.updated_at = shared::now();
        }
        if let Some(status_id) = cmd.status_id {
            let sid = status_id
                .parse()
                .map_err(|_| AppError::invalid_input("status_id"))?;
            let target = StatusId::from_uuid(sid);
            let allowed = self.transitions.is_allowed(issue.status_id, target).await?;
            if !allowed {
                return Err(AppError::invalid_input("workflow transition not allowed"));
            }
            issue.change_status(target);
        }
        if let Some(assignee_id) = cmd.assignee_id {
            issue.assign(assignee_id);
        }
        if let Some(sprint_id) = cmd.sprint_id {
            issue.sprint_id = sprint_id;
        }
        if let Some(component_id) = cmd.component_id {
            issue.component_id = component_id;
            issue.updated_at = shared::now();
        }
        if let Some(affected_version_id) = cmd.affected_version_id {
            issue.affected_version_id = affected_version_id;
            issue.updated_at = shared::now();
        }
        if let Some(fix_version_id) = cmd.fix_version_id {
            issue.fix_version_id = fix_version_id;
            issue.updated_at = shared::now();
        }

        self.issues.save(&issue).await?;
        let statuses = self.statuses.list_all().await.unwrap_or_default();
        let column = statuses
            .iter()
            .find(|s| s.id == issue.status_id)
            .map(|s| s.name.as_ref().to_string())
            .unwrap_or_else(|| helpers::issue_status_column(issue.status_id));
        let (assignee_name, reporter_name) =
            helpers::resolve_names(self.users.clone(), &issue).await;
        self.events.publish(shared::TrackerEvent::IssueUpdated {
            issue_id: issue.id.to_string(),
            project_key: project.key.to_string(),
        });
        // Notify assignee if assignment changed
        if let Some(new_assignee) = cmd.assignee_id.flatten() {
            if new_assignee != issue.reporter_id {
                let key = issue.key.to_string();
                self.create_notification(domain::Notification {
                    id: shared::NotificationId::new(),
                    recipient_id: new_assignee,
                    event_type: "issue_assigned".into(),
                    entity_type: "issue".into(),
                    entity_id: Some(issue.id.as_uuid()),
                    actor_id: Some(cmd.actor_id),
                    title: format!("You were assigned to {}", key).into(),
                    body: Some(issue.summary.as_ref().to_string().into()),
                    is_read: false,
                    read_at: None,
                    action_url: Some(
                        format!("/projects/{}/issues/{}", project.key, issue.id).into(),
                    ),
                    metadata: serde_json::json!({"issue_key": key}),
                    created_at: shared::now(),
                })
                .await;
            }
        }
        Ok(IssueDto::from_issue(
            issue,
            project.name.as_ref().to_string(),
            column,
            assignee_name,
            reporter_name,
        ))
    }

    async fn search(
        &self,
        filters: crate::context::SearchFilters,
    ) -> Result<Vec<IssueDto>, AppError> {
        let mut query = IssueQuery::default();
        if let Some(q) = filters.q.as_deref().filter(|s| !s.is_empty()) {
            query.search_text = Some(q.to_string());
        }
        if let Some(priority) = filters.priority.as_deref().filter(|s| !s.is_empty()) {
            query.priority = Some(priority.to_string());
        }
        if let Some(sort_by) = filters.sort_by.as_deref() {
            query.sort_by = Some(sort_by.to_string());
            query.sort_order = filters.sort_order.clone();
        }
        if let Some(project_key) = filters.project_key.as_deref().filter(|s| !s.is_empty()) {
            let key: ProjectKey = project_key
                .parse()
                .map_err(|e: String| AppError::invalid_input(e))?;
            let project = self.projects.get_by_key(&key).await?;
            query.project_id = Some(project.id);
        }
        if let Some(assignee_id) = filters.assignee_id.as_deref().filter(|s| !s.is_empty()) {
            let uuid = uuid::Uuid::parse_str(assignee_id)
                .map_err(|e| AppError::invalid_input(e.to_string()))?;
            query.assignee_id = Some(UserId::from_uuid(uuid));
        }
        let issues = self.issues.list(query).await?;
        helpers::build_issue_dtos_with_projects(
            Arc::clone(&self.projects),
            Arc::clone(&self.users),
            issues,
        )
        .await
    }

    async fn delete(&self, id: IssueId) -> Result<(), AppError> {
        self.issues.delete(id).await
    }

    async fn restore(&self, id: IssueId) -> Result<IssueDto, AppError> {
        self.issues.restore(id).await?;
        let issue = self.issues.get_by_id(id).await?;
        helpers::build_issue_dtos_with_projects(
            Arc::clone(&self.projects),
            Arc::clone(&self.users),
            vec![issue],
        )
        .await
        .map(|mut v| v.remove(0))
    }

    async fn purge(&self, id: IssueId) -> Result<(), AppError> {
        self.issues.purge(id).await
    }

    async fn list_trash(&self, project_key: &ProjectKey) -> Result<Vec<IssueDto>, AppError> {
        let project = self
            .projects
            .get_by_key(project_key)
            .await
            .map_err(|_| AppError::not_found("project", project_key))?;
        let query = IssueQuery {
            project_id: Some(project.id),
            deleted_only: true,
            ..Default::default()
        };
        let issues = self.issues.list(query).await?;
        helpers::build_issue_dtos_with_projects(
            Arc::clone(&self.projects),
            Arc::clone(&self.users),
            issues,
        )
        .await
    }
}

pub struct BoardServiceImpl {
    boards: Arc<dyn domain::BoardRepository>,
    issues: Arc<dyn IssueRepository>,
    sprints: Arc<dyn SprintRepository>,
    users: Arc<dyn domain::UserRepository>,
    statuses: Arc<dyn StatusRepository>,
    transitions: Arc<dyn WorkflowTransitionRepository>,
}

impl BoardServiceImpl {
    pub fn new(
        boards: Arc<dyn domain::BoardRepository>,
        issues: Arc<dyn IssueRepository>,
        sprints: Arc<dyn SprintRepository>,
        users: Arc<dyn domain::UserRepository>,
        statuses: Arc<dyn StatusRepository>,
        transitions: Arc<dyn WorkflowTransitionRepository>,
    ) -> Self {
        Self {
            boards,
            issues,
            sprints,
            users,
            statuses,
            transitions,
        }
    }

    async fn build_board_dto(&self, project_key: &ProjectKey) -> Result<BoardDto, AppError> {
        let board = self.boards.get_default_by_project_key(project_key).await?;
        let sprint = self.sprints.get_active_by_project(board.project_id).await?;
        let issues = self
            .issues
            .list(IssueQuery {
                project_id: Some(board.project_id),
                ..Default::default()
            })
            .await?;

        let db_statuses = self.statuses.list_all().await.unwrap_or_default();
        let columns: Vec<BoardColumnDto> = if board.columns.iter().all(|c| c.id.as_uuid().is_nil())
        {
            db_statuses
                .iter()
                .map(|s| BoardColumnDto {
                    id: s.id.to_string(),
                    name: s.name.as_ref().to_string(),
                    wip_limit: None,
                    issue_ids: issues
                        .iter()
                        .filter(|i| i.status_id == s.id)
                        .map(|i| i.id.to_string())
                        .collect(),
                })
                .collect()
        } else {
            board
                .columns
                .iter()
                .map(|c| {
                    // Statuses are the single source of truth for names.
                    let name = db_statuses
                        .iter()
                        .find(|s| s.id == c.id)
                        .map(|s| s.name.as_ref().to_string())
                        .unwrap_or_else(|| c.name.as_ref().to_string());
                    BoardColumnDto {
                        id: c.id.to_string(),
                        name,
                        wip_limit: c.wip_limit,
                        issue_ids: issues
                            .iter()
                            .filter(|i| i.status_id == c.id)
                            .map(|i| i.id.to_string())
                            .collect(),
                    }
                })
                .collect()
        };

        let issue_dtos = helpers::build_issue_dtos(
            Arc::clone(&self.users),
            issues,
            project_key.to_string().as_str(),
        )
        .await?;

        let sprint_dto = sprint
            .map(|s| SprintDto::from_sprint(s, issue_dtos.iter().map(|i| i.id.clone()).collect()))
            .unwrap_or_else(|| SprintDto {
                id: "none".to_string(),
                name: "Backlog".to_string(),
                goal: String::new(),
                state: "future".to_string(),
                velocity: 0,
                remaining_days: None,
                issue_ids: vec![],
                start_date: None,
                end_date: None,
            });

        Ok(BoardDto {
            project_id: board.project_id.to_string(),
            project_key: project_key.to_string(),
            columns,
            issues: issue_dtos,
            sprint: sprint_dto,
        })
    }
}

#[async_trait]
impl crate::context::BoardService for BoardServiceImpl {
    async fn get_board(&self, project_key: &ProjectKey) -> Result<BoardDto, AppError> {
        self.build_board_dto(project_key).await
    }

    async fn get_backlog(&self, project_key: &ProjectKey) -> Result<BacklogDto, AppError> {
        let board = self.boards.get_default_by_project_key(project_key).await?;
        let sprint = self.sprints.get_active_by_project(board.project_id).await?;
        let all_issues = self
            .issues
            .list(IssueQuery {
                project_id: Some(board.project_id),
                ..Default::default()
            })
            .await?;

        let db_statuses = self.statuses.list_all().await.unwrap_or_default();
        let todo_status = db_statuses
            .iter()
            .find(|s| s.category == StatusCategory::Todo)
            .map(|s| s.id)
            .unwrap_or_else(|| {
                board
                    .columns
                    .iter()
                    .find(|c| c.category == StatusCategory::Todo)
                    .map(|c| c.id)
                    .unwrap_or(StatusId::from_uuid(uuid::Uuid::nil()))
            });

        let sprint_issues_raw: Vec<_> = all_issues
            .clone()
            .into_iter()
            .filter(|i| i.sprint_id.is_some() || i.status_id != todo_status)
            .collect();
        let backlog_issues_raw: Vec<_> = all_issues
            .into_iter()
            .filter(|i| i.sprint_id.is_none() && i.status_id == todo_status)
            .collect();

        let sprint_dto = sprint
            .map(|s| {
                SprintDto::from_sprint(
                    s,
                    sprint_issues_raw.iter().map(|i| i.id.to_string()).collect(),
                )
            })
            .unwrap_or_else(|| SprintDto {
                id: "none".to_string(),
                name: "Backlog".to_string(),
                goal: String::new(),
                state: "future".to_string(),
                velocity: 0,
                remaining_days: None,
                issue_ids: vec![],
                start_date: None,
                end_date: None,
            });

        let project_label = project_key.to_string();
        let sprint_issues = helpers::build_issue_dtos(
            Arc::clone(&self.users),
            sprint_issues_raw,
            project_label.as_str(),
        )
        .await?;
        let backlog_issues = helpers::build_issue_dtos(
            Arc::clone(&self.users),
            backlog_issues_raw,
            project_label.as_str(),
        )
        .await?;

        Ok(BacklogDto {
            project_id: board.project_id.to_string(),
            project_key: project_key.to_string(),
            sprint: sprint_dto,
            sprint_issues,
            backlog_issues,
        })
    }

    async fn move_issue(
        &self,
        project_key: &ProjectKey,
        issue_id: IssueId,
        status_id: StatusId,
    ) -> Result<BoardDto, AppError> {
        let issue = self.issues.get_by_id(issue_id).await?;
        let allowed = self
            .transitions
            .is_allowed(issue.status_id, status_id)
            .await?;
        if !allowed {
            return Err(AppError::invalid_input("workflow transition not allowed"));
        }
        let mut updated = issue.clone();
        updated.change_status(status_id);
        self.issues.save(&updated).await?;
        self.build_board_dto(project_key).await
    }
}

pub struct DashboardServiceImpl {
    issues: Arc<dyn IssueRepository>,
    projects: Arc<dyn ProjectRepository>,
    users: Arc<dyn domain::UserRepository>,
}

#[async_trait]
pub trait SprintService: Send + Sync {
    async fn create(
        &self,
        cmd: crate::commands::CreateSprintCommand,
    ) -> Result<SprintDto, AppError>;
    async fn list(&self, project_id: ProjectId) -> Result<Vec<SprintDto>, AppError>;
    async fn get_by_id(&self, id: SprintId) -> Result<SprintDto, AppError>;
    async fn update(
        &self,
        id: SprintId,
        cmd: crate::commands::UpdateSprintCommand,
    ) -> Result<SprintDto, AppError>;
    async fn start(&self, id: SprintId) -> Result<SprintDto, AppError>;
    async fn close(&self, id: SprintId) -> Result<SprintDto, AppError>;
    async fn move_issue(
        &self,
        cmd: crate::commands::MoveIssueToSprintCommand,
    ) -> Result<IssueDto, AppError>;
}

pub struct SprintServiceImpl {
    sprints: Arc<dyn domain::SprintRepository>,
    issues: Arc<dyn IssueRepository>,
    projects: Arc<dyn ProjectRepository>,
    users: Arc<dyn domain::UserRepository>,
}

impl SprintServiceImpl {
    pub fn new(
        sprints: Arc<dyn domain::SprintRepository>,
        issues: Arc<dyn IssueRepository>,
        projects: Arc<dyn ProjectRepository>,
        users: Arc<dyn domain::UserRepository>,
    ) -> Self {
        Self {
            sprints,
            issues,
            projects,
            users,
        }
    }

    async fn sprint_dto(&self, sprint: domain::Sprint) -> Result<SprintDto, AppError> {
        let issues = self
            .issues
            .list(IssueQuery {
                sprint_id: Some(sprint.id),
                ..Default::default()
            })
            .await?;
        Ok(SprintDto::from_sprint(
            sprint,
            issues.into_iter().map(|i| i.id.to_string()).collect(),
        ))
    }
}

#[async_trait]
impl SprintService for SprintServiceImpl {
    async fn create(
        &self,
        cmd: crate::commands::CreateSprintCommand,
    ) -> Result<SprintDto, AppError> {
        let sprint = domain::Sprint {
            id: SprintId::new(),
            project_id: cmd.project_id,
            name: cmd.name.into(),
            goal: cmd.goal.map(Into::into),
            state: domain::SprintState::Future,
            start_date: cmd.start_date,
            end_date: cmd.end_date,
            velocity: None,
        };
        self.sprints.save(&sprint).await?;
        self.sprint_dto(sprint).await
    }

    async fn list(&self, project_id: ProjectId) -> Result<Vec<SprintDto>, AppError> {
        let sprints = self.sprints.list_by_project(project_id).await?;
        let mut result = Vec::with_capacity(sprints.len());
        for s in sprints {
            result.push(self.sprint_dto(s).await?);
        }
        Ok(result)
    }

    async fn get_by_id(&self, id: SprintId) -> Result<SprintDto, AppError> {
        let sprint = self.sprints.get_by_id(id).await?;
        self.sprint_dto(sprint).await
    }

    async fn update(
        &self,
        id: SprintId,
        cmd: crate::commands::UpdateSprintCommand,
    ) -> Result<SprintDto, AppError> {
        let mut sprint = self.sprints.get_by_id(id).await?;
        if let Some(name) = cmd.name {
            sprint.name = name.into();
        }
        if let Some(goal) = cmd.goal {
            sprint.goal = goal.map(Into::into);
        }
        if let Some(start_date) = cmd.start_date {
            sprint.start_date = start_date;
        }
        if let Some(end_date) = cmd.end_date {
            sprint.end_date = end_date;
        }
        self.sprints.save(&sprint).await?;
        self.sprint_dto(sprint).await
    }

    async fn start(&self, id: SprintId) -> Result<SprintDto, AppError> {
        let mut sprint = self.sprints.get_by_id(id).await?;
        if sprint.state != domain::SprintState::Future {
            return Err(AppError::invalid_input("sprint is not in future state"));
        }
        sprint.state = domain::SprintState::Active;
        sprint.start_date = Some(sprint.start_date.unwrap_or_else(shared::now));
        self.sprints.save(&sprint).await?;
        self.sprint_dto(sprint).await
    }

    async fn close(&self, id: SprintId) -> Result<SprintDto, AppError> {
        let mut sprint = self.sprints.get_by_id(id).await?;
        if sprint.state != domain::SprintState::Active {
            return Err(AppError::invalid_input("sprint is not active"));
        }
        sprint.state = domain::SprintState::Closed;
        sprint.end_date = Some(sprint.end_date.unwrap_or_else(shared::now));
        self.sprints.save(&sprint).await?;
        self.sprint_dto(sprint).await
    }

    async fn move_issue(
        &self,
        cmd: crate::commands::MoveIssueToSprintCommand,
    ) -> Result<IssueDto, AppError> {
        let mut issue = self.issues.get_by_id(cmd.issue_id).await?;
        if let Some(sprint_id) = cmd.sprint_id {
            let _ = self.sprints.get_by_id(sprint_id).await?;
            issue.sprint_id = Some(sprint_id);
        } else {
            issue.sprint_id = None;
        }
        self.issues.save(&issue).await?;
        let name = helpers::project_name(self.projects.clone(), issue.project_id).await?;
        Ok(helpers::build_issue_dto(self.users.clone(), issue, name.as_str()).await)
    }
}

impl DashboardServiceImpl {
    pub fn new(
        issues: Arc<dyn IssueRepository>,
        projects: Arc<dyn ProjectRepository>,
        users: Arc<dyn domain::UserRepository>,
    ) -> Self {
        Self {
            issues,
            projects,
            users,
        }
    }
}

#[async_trait]
impl crate::context::DashboardService for DashboardServiceImpl {
    async fn get_dashboard(&self, user_id: UserId) -> Result<DashboardDto, AppError> {
        let issues = self.issues.list(IssueQuery::assignee(user_id)).await?;
        let dtos = helpers::build_issue_dtos_for_dashboard(
            Arc::clone(&self.projects),
            Arc::clone(&self.users),
            issues,
        )
        .await?;
        Ok(DashboardDto {
            assigned_issues: dtos,
        })
    }
}

pub struct SearchServiceImpl {
    issues: Arc<dyn IssueRepository>,
    projects: Arc<dyn ProjectRepository>,
    users: Arc<dyn domain::UserRepository>,
}

impl SearchServiceImpl {
    pub fn new(
        issues: Arc<dyn IssueRepository>,
        projects: Arc<dyn ProjectRepository>,
        users: Arc<dyn domain::UserRepository>,
    ) -> Self {
        Self {
            issues,
            projects,
            users,
        }
    }
}

#[async_trait]
impl crate::context::SearchService for SearchServiceImpl {
    async fn search(&self, filters: SearchFilters) -> Result<Vec<IssueDto>, AppError> {
        let mut query = IssueQuery::default();
        if let Some(q) = filters.q.as_deref().filter(|s| !s.is_empty()) {
            query.search_text = Some(q.to_string());
        }
        if let Some(priority) = filters.priority.as_deref().filter(|s| !s.is_empty()) {
            query.priority = Some(priority.to_string());
        }
        if let Some(sort_by) = filters.sort_by.as_deref() {
            query.sort_by = Some(sort_by.to_string());
            query.sort_order = filters.sort_order.clone();
        }
        if let Some(project_key) = filters.project_key.as_deref().filter(|s| !s.is_empty()) {
            let key: ProjectKey = project_key
                .parse()
                .map_err(|e: String| AppError::invalid_input(e))?;
            let project = self.projects.get_by_key(&key).await?;
            query.project_id = Some(project.id);
        }
        if let Some(assignee_id) = filters.assignee_id.as_deref().filter(|s| !s.is_empty()) {
            let uuid = uuid::Uuid::parse_str(assignee_id)
                .map_err(|e| AppError::invalid_input(e.to_string()))?;
            query.assignee_id = Some(UserId::from_uuid(uuid));
        }
        if let Some(jql_str) = filters.jql.as_deref().filter(|s| !s.is_empty()) {
            let expr =
                domain::jql::parse(jql_str).map_err(|e| AppError::invalid_input(e.to_string()))?;
            query.jql = Some(expr);
            if let Some(uid_str) = filters.user_id.as_deref().filter(|s| !s.is_empty()) {
                let uuid = uuid::Uuid::parse_str(uid_str)
                    .map_err(|e| AppError::invalid_input(e.to_string()))?;
                query.jql_user_id = Some(UserId::from_uuid(uuid));
            }
        }
        let issues = self.issues.list(query).await?;
        helpers::build_issue_dtos_with_projects(
            Arc::clone(&self.projects),
            Arc::clone(&self.users),
            issues,
        )
        .await
    }
}

#[derive(Clone)]
pub struct CommentServiceImpl {
    comments: Arc<dyn domain::CommentRepository>,
    users: Arc<dyn domain::UserRepository>,
    issues: Arc<dyn domain::IssueRepository>,
    projects: Arc<dyn ProjectRepository>,
    events: crate::context::EventBus,
    notifications: Arc<dyn domain::NotificationRepository>,
}

impl CommentServiceImpl {
    pub fn new(
        comments: Arc<dyn domain::CommentRepository>,
        users: Arc<dyn domain::UserRepository>,
        issues: Arc<dyn domain::IssueRepository>,
        projects: Arc<dyn ProjectRepository>,
        events: crate::context::EventBus,
        notifications: Arc<dyn domain::NotificationRepository>,
    ) -> Self {
        Self {
            comments,
            users,
            issues,
            projects,
            events,
            notifications,
        }
    }

    /// Create a notification and publish a real-time SSE event.
    async fn create_notification(&self, notification: domain::Notification) {
        let recipient_id = notification.recipient_id;
        if let Ok(_id) = self.notifications.save(&notification).await {
            self.events
                .publish(shared::TrackerEvent::NotificationCreated {
                    recipient_id: recipient_id.to_string(),
                });
        }
    }
}

#[async_trait]
impl crate::context::CommentService for CommentServiceImpl {
    async fn list(
        &self,
        issue_id: IssueId,
        _requester: UserId,
    ) -> Result<Vec<CommentDto>, AppError> {
        self.issues.get_by_id(issue_id).await?;
        let comments = self.comments.list_by_issue(issue_id).await?;
        let mut result = Vec::with_capacity(comments.len());
        for c in comments {
            let user = self.users.get_by_id(c.author_id).await.ok();
            result.push(CommentDto::from_comment(
                c,
                user.map(|u| u.display_name.as_ref().to_string()),
            ));
        }
        Ok(result)
    }

    async fn create(&self, cmd: CreateCommentCommand) -> Result<CommentDto, AppError> {
        self.issues.get_by_id(cmd.issue_id).await?;
        let comment = domain::Comment {
            id: shared::CommentId::new(),
            issue_id: cmd.issue_id,
            author_id: cmd.author_id,
            body: domain::value_objects::RichText::new(cmd.body),
            created_at: shared::now(),
            updated_at: shared::now(),
        };
        self.comments.save(&comment).await?;
        let user = self.users.get_by_id(cmd.author_id).await.ok();
        if let Ok(issue) = self.issues.get_by_id(cmd.issue_id).await {
            if let Ok(project) = self.projects.get_by_id(issue.project_id).await {
                self.events.publish(shared::TrackerEvent::IssueCommented {
                    issue_id: cmd.issue_id.to_string(),
                    project_key: project.key.to_string(),
                });
                // Notify reporter and assignee about new comment (if different from author)
                let key = issue.key.to_string();
                let action_url = format!("/projects/{}/issues/{}", project.key, issue.id);
                for recipient in [
                    issue.reporter_id,
                    issue.assignee_id.unwrap_or(issue.reporter_id),
                ] {
                    if recipient != cmd.author_id {
                        self.create_notification(domain::Notification {
                            id: shared::NotificationId::new(),
                            recipient_id: recipient,
                            event_type: "issue_commented".into(),
                            entity_type: "issue".into(),
                            entity_id: Some(issue.id.as_uuid()),
                            actor_id: Some(cmd.author_id),
                            title: format!("New comment on {}", key).into(),
                            body: None,
                            is_read: false,
                            read_at: None,
                            action_url: Some(action_url.clone().into()),
                            metadata: serde_json::json!({"issue_key": key}),
                            created_at: shared::now(),
                        })
                        .await;
                    }
                }
            }
        }
        Ok(CommentDto::from_comment(
            comment,
            user.map(|u| u.display_name.as_ref().to_string()),
        ))
    }

    async fn update(
        &self,
        id: shared::CommentId,
        cmd: UpdateCommentCommand,
        requester: UserId,
    ) -> Result<CommentDto, AppError> {
        let mut comment = self.comments.get_by_id(id).await?;
        if comment.author_id != requester {
            return Err(AppError::Unauthorized);
        }
        if let Some(body) = cmd.body {
            comment.body = domain::value_objects::RichText::new(body);
            comment.updated_at = shared::now();
        }
        self.comments.save(&comment).await?;
        let user = self.users.get_by_id(comment.author_id).await.ok();
        Ok(CommentDto::from_comment(
            comment,
            user.map(|u| u.display_name.as_ref().to_string()),
        ))
    }

    async fn delete(&self, id: shared::CommentId, requester: UserId) -> Result<(), AppError> {
        let comment = self.comments.get_by_id(id).await?;
        if comment.author_id != requester {
            return Err(AppError::Unauthorized);
        }
        self.comments.delete(id).await
    }
}

#[derive(Clone)]
pub struct WorklogServiceImpl {
    worklogs: Arc<dyn domain::WorklogRepository>,
    users: Arc<dyn domain::UserRepository>,
    issues: Arc<dyn domain::IssueRepository>,
}

impl WorklogServiceImpl {
    pub fn new(
        worklogs: Arc<dyn domain::WorklogRepository>,
        users: Arc<dyn domain::UserRepository>,
        issues: Arc<dyn domain::IssueRepository>,
    ) -> Self {
        Self {
            worklogs,
            users,
            issues,
        }
    }
}

#[async_trait]
impl crate::context::WorklogService for WorklogServiceImpl {
    async fn list(
        &self,
        issue_id: IssueId,
        _requester: UserId,
    ) -> Result<Vec<WorklogDto>, AppError> {
        self.issues.get_by_id(issue_id).await?;
        let worklogs = self.worklogs.list_by_issue(issue_id).await?;
        let mut result = Vec::with_capacity(worklogs.len());
        for w in worklogs {
            let user = self.users.get_by_id(w.author_id).await.ok();
            result.push(WorklogDto::from_worklog(
                w,
                user.map(|u| u.display_name.as_ref().to_string()),
            ));
        }
        Ok(result)
    }

    async fn create(&self, cmd: CreateWorklogCommand) -> Result<WorklogDto, AppError> {
        self.issues.get_by_id(cmd.issue_id).await?;
        let worklog = domain::Worklog {
            id: shared::WorklogId::new(),
            issue_id: cmd.issue_id,
            author_id: cmd.author_id,
            started_at: cmd.started_at,
            duration_seconds: cmd.duration_seconds,
            description: cmd.description.map(|d| d.into()),
            created_at: shared::now(),
            updated_at: shared::now(),
        };
        self.worklogs.save(&worklog).await?;
        let user = self.users.get_by_id(cmd.author_id).await.ok();
        Ok(WorklogDto::from_worklog(
            worklog,
            user.map(|u| u.display_name.as_ref().to_string()),
        ))
    }

    async fn update(
        &self,
        id: shared::WorklogId,
        cmd: UpdateWorklogCommand,
        requester: UserId,
    ) -> Result<WorklogDto, AppError> {
        let mut worklog = self.worklogs.get_by_id(id).await?;
        if worklog.author_id != requester {
            return Err(AppError::Unauthorized);
        }
        if let Some(started_at) = cmd.started_at {
            worklog.started_at = started_at;
        }
        if let Some(duration) = cmd.duration_seconds {
            worklog.duration_seconds = duration;
        }
        if let Some(description) = cmd.description {
            worklog.description = description.map(|d| d.into());
        }
        worklog.updated_at = shared::now();
        self.worklogs.save(&worklog).await?;
        let user = self.users.get_by_id(worklog.author_id).await.ok();
        Ok(WorklogDto::from_worklog(
            worklog,
            user.map(|u| u.display_name.as_ref().to_string()),
        ))
    }

    async fn delete(&self, id: shared::WorklogId, requester: UserId) -> Result<(), AppError> {
        let worklog = self.worklogs.get_by_id(id).await?;
        if worklog.author_id != requester {
            return Err(AppError::Unauthorized);
        }
        self.worklogs.delete(id).await
    }
}

#[async_trait]
pub trait ProjectMemberService: Send + Sync {
    async fn list(&self, project_id: ProjectId) -> Result<Vec<ProjectMemberDto>, AppError>;
    async fn add(
        &self,
        cmd: crate::commands::AddProjectMemberCommand,
    ) -> Result<ProjectMemberDto, AppError>;
    async fn remove(&self, project_id: ProjectId, user_id: UserId) -> Result<(), AppError>;
}

pub struct ProjectMemberServiceImpl {
    members: Arc<dyn ProjectMemberRepository>,
    users: Arc<dyn UserRepository>,
}

impl ProjectMemberServiceImpl {
    pub fn new(members: Arc<dyn ProjectMemberRepository>, users: Arc<dyn UserRepository>) -> Self {
        Self { members, users }
    }
}

#[async_trait]
impl ProjectMemberService for ProjectMemberServiceImpl {
    async fn list(&self, project_id: ProjectId) -> Result<Vec<ProjectMemberDto>, AppError> {
        let members = self.members.list_by_project(project_id).await?;
        let mut dtos = Vec::with_capacity(members.len());
        for m in members {
            let role = m.role.as_str().to_string();
            dtos.push(ProjectMemberDto {
                project_id: m.project_id.to_string(),
                user_id: m.user_id.to_string(),
                role,
                joined_at: m.joined_at,
            });
        }
        Ok(dtos)
    }

    async fn add(
        &self,
        cmd: crate::commands::AddProjectMemberCommand,
    ) -> Result<ProjectMemberDto, AppError> {
        let role = ProjectRole::from_str(&cmd.role).unwrap_or_default();
        let _ = self.users.get_by_id(cmd.user_id).await?;
        // Re-adding an existing member upserts the role (repo save is idempotent)
        // and preserves the original joined_at.
        let joined_at = match self.members.get(cmd.project_id, cmd.user_id).await {
            Ok(existing) => existing.joined_at,
            Err(_) => shared::now(),
        };
        let member = ProjectMember {
            project_id: cmd.project_id,
            user_id: cmd.user_id,
            role,
            joined_at,
        };
        self.members.save(&member).await?;
        Ok(ProjectMemberDto {
            project_id: member.project_id.to_string(),
            user_id: member.user_id.to_string(),
            role: member.role.as_str().to_string(),
            joined_at: member.joined_at,
        })
    }

    async fn remove(&self, project_id: ProjectId, user_id: UserId) -> Result<(), AppError> {
        self.members.delete(project_id, user_id).await
    }
}

pub struct StatusServiceImpl {
    statuses: Arc<dyn StatusRepository>,
}

impl StatusServiceImpl {
    pub fn new(statuses: Arc<dyn StatusRepository>) -> Self {
        Self { statuses }
    }
}

#[async_trait]
impl crate::context::StatusService for StatusServiceImpl {
    async fn list_statuses(&self) -> Result<Vec<domain::Status>, AppError> {
        self.statuses.list_all().await
    }
}

pub struct WorkflowServiceImpl {
    transitions: Arc<dyn WorkflowTransitionRepository>,
}

impl WorkflowServiceImpl {
    pub fn new(transitions: Arc<dyn WorkflowTransitionRepository>) -> Self {
        Self { transitions }
    }
}

#[async_trait]
impl crate::context::WorkflowService for WorkflowServiceImpl {
    async fn list_transitions(&self) -> Result<Vec<WorkflowTransition>, AppError> {
        self.transitions.list_all().await
    }

    async fn is_transition_allowed(
        &self,
        from_status_id: StatusId,
        to_status_id: StatusId,
    ) -> Result<bool, AppError> {
        self.transitions
            .is_allowed(from_status_id, to_status_id)
            .await
    }
}

pub struct IssueTypeServiceImpl {
    issue_types: Arc<dyn IssueTypeRepository>,
}

impl IssueTypeServiceImpl {
    pub fn new(issue_types: Arc<dyn IssueTypeRepository>) -> Self {
        Self { issue_types }
    }
}

#[async_trait]
impl crate::context::IssueTypeService for IssueTypeServiceImpl {
    async fn list_issue_types(&self) -> Result<Vec<IssueTypeEntity>, AppError> {
        self.issue_types.list_all().await
    }
}

pub struct AttachmentServiceImpl {
    attachments: Arc<dyn domain::AttachmentRepository>,
    issues: Arc<dyn IssueRepository>,
    storage: Arc<dyn domain::FileStorage>,
}

impl AttachmentServiceImpl {
    pub fn new(
        attachments: Arc<dyn domain::AttachmentRepository>,
        issues: Arc<dyn IssueRepository>,
        storage: Arc<dyn domain::FileStorage>,
    ) -> Self {
        Self {
            attachments,
            issues,
            storage,
        }
    }

    fn to_dto(a: &domain::Attachment) -> crate::context::AttachmentDto {
        crate::context::AttachmentDto {
            id: a.id.to_string(),
            issue_id: a.issue_id.to_string(),
            author_id: a.author_id.to_string(),
            file_name: a.file_name.as_ref().to_string(),
            content_type: a.content_type.as_ref().to_string(),
            size_bytes: a.size_bytes,
            created_at: a.created_at.to_rfc3339(),
        }
    }
}

#[async_trait]
impl crate::context::AttachmentService for AttachmentServiceImpl {
    async fn upload(
        &self,
        issue_id: IssueId,
        author_id: UserId,
        file_name: &str,
        content_type: &str,
        bytes: Vec<u8>,
    ) -> Result<crate::context::AttachmentDto, AppError> {
        let issue = self.issues.get_by_id(issue_id).await?;
        let key = format!("{}-{}", uuid::Uuid::new_v4(), file_name);
        self.storage.put(&issue.id.to_string(), &key, bytes).await?;
        let attachment = domain::Attachment {
            id: shared::AttachmentId::new(),
            issue_id: issue.id,
            author_id,
            file_name: file_name.into(),
            content_type: content_type.into(),
            size_bytes: 0, // corrected below from stored file
            storage_key: key.as_str().into(),
            created_at: shared::now(),
        };
        // size from the uploaded bytes (validated in storage)
        let mut a = attachment;
        a.size_bytes = self.storage.get(&a.issue_id.to_string(), &key).await?.len() as i64;
        self.attachments.save(&a).await?;
        Ok(Self::to_dto(&a))
    }

    async fn list_by_issue(
        &self,
        issue_id: IssueId,
    ) -> Result<Vec<crate::context::AttachmentDto>, AppError> {
        let items = self.attachments.list_by_issue(issue_id).await?;
        Ok(items.iter().map(Self::to_dto).collect())
    }

    async fn download(
        &self,
        attachment_id: shared::AttachmentId,
    ) -> Result<(crate::context::AttachmentDto, Vec<u8>), AppError> {
        let a = self.attachments.get_by_id(attachment_id).await?;
        let bytes = self
            .storage
            .get(&a.issue_id.to_string(), a.storage_key.as_ref())
            .await?;
        Ok((Self::to_dto(&a), bytes))
    }

    async fn delete(
        &self,
        attachment_id: shared::AttachmentId,
        _requester: UserId,
    ) -> Result<(), AppError> {
        let a = self.attachments.get_by_id(attachment_id).await?;
        self.storage
            .delete(&a.issue_id.to_string(), a.storage_key.as_ref())
            .await?;
        self.attachments.delete(attachment_id).await?;
        Ok(())
    }
}

pub struct LabelServiceImpl {
    labels: Arc<dyn domain::LabelRepository>,
    projects: Arc<dyn ProjectRepository>,
    issues: Arc<dyn IssueRepository>,
}

impl LabelServiceImpl {
    pub fn new(
        labels: Arc<dyn domain::LabelRepository>,
        projects: Arc<dyn ProjectRepository>,
        issues: Arc<dyn IssueRepository>,
    ) -> Self {
        Self {
            labels,
            projects,
            issues,
        }
    }

    fn to_dto(l: &domain::Label) -> crate::context::LabelDto {
        crate::context::LabelDto {
            id: l.id.to_string(),
            project_id: l.project_id.to_string(),
            name: l.name.as_ref().to_string(),
            color: l.color.as_ref().to_string(),
        }
    }
}

#[async_trait]
impl crate::context::LabelService for LabelServiceImpl {
    async fn create(
        &self,
        project_key: &ProjectKey,
        name: &str,
        color: &str,
        _requester: UserId,
    ) -> Result<crate::context::LabelDto, AppError> {
        let project = self.projects.get_by_key(project_key).await?;
        if name.trim().is_empty() {
            return Err(AppError::invalid_input("label name must not be empty"));
        }
        let label = domain::Label {
            id: shared::LabelId::new(),
            project_id: project.id,
            name: name.trim().to_string().into(),
            color: color.to_string().into(),
        };
        self.labels.save(&label).await?;
        Ok(Self::to_dto(&label))
    }

    async fn list_by_project(
        &self,
        project_key: &ProjectKey,
    ) -> Result<Vec<crate::context::LabelDto>, AppError> {
        let project = self.projects.get_by_key(project_key).await?;
        let items = self.labels.list_by_project(project.id).await?;
        Ok(items.iter().map(Self::to_dto).collect())
    }

    async fn update(
        &self,
        label_id: shared::LabelId,
        name: &str,
        color: &str,
        _requester: UserId,
    ) -> Result<crate::context::LabelDto, AppError> {
        let mut label = self.labels.get_by_id(label_id).await?;
        if !name.trim().is_empty() {
            label.name = name.trim().to_string().into();
        }
        label.color = color.to_string().into();
        self.labels.save(&label).await?;
        Ok(Self::to_dto(&label))
    }

    async fn delete(&self, label_id: shared::LabelId, _requester: UserId) -> Result<(), AppError> {
        self.labels.delete(label_id).await?;
        Ok(())
    }

    async fn list_for_issue(
        &self,
        issue_id: IssueId,
    ) -> Result<Vec<crate::context::LabelDto>, AppError> {
        let ids = self.labels.list_ids_by_issue(issue_id).await?;
        let mut out = Vec::with_capacity(ids.len());
        for id in ids {
            let l = self.labels.get_by_id(id).await?;
            out.push(Self::to_dto(&l));
        }
        Ok(out)
    }

    async fn attach(
        &self,
        issue_id: IssueId,
        label_id: shared::LabelId,
        _requester: UserId,
    ) -> Result<(), AppError> {
        let _issue = self.issues.get_by_id(issue_id).await?;
        let _label = self.labels.get_by_id(label_id).await?;
        self.labels.attach(issue_id, label_id).await?;
        Ok(())
    }

    async fn detach(
        &self,
        issue_id: IssueId,
        label_id: shared::LabelId,
        _requester: UserId,
    ) -> Result<(), AppError> {
        self.labels.detach(issue_id, label_id).await?;
        Ok(())
    }
}

pub struct IssueLinkServiceImpl {
    links: Arc<dyn domain::IssueLinkRepository>,
    issues: Arc<dyn IssueRepository>,
}

impl IssueLinkServiceImpl {
    pub fn new(
        links: Arc<dyn domain::IssueLinkRepository>,
        issues: Arc<dyn IssueRepository>,
    ) -> Self {
        Self { links, issues }
    }
}

#[async_trait]
impl crate::context::IssueLinkService for IssueLinkServiceImpl {
    async fn create(
        &self,
        source_id: IssueId,
        target_key: &str,
        link_type: &str,
        _requester: UserId,
    ) -> Result<crate::context::IssueLinkDto, AppError> {
        let source = self.issues.get_by_id(source_id).await?;
        // Validate the link type before resolving the target so bad input is 400, not 404.
        let lt: domain::LinkType = link_type.parse().map_err(AppError::invalid_input)?;
        let target_key_vo = IssueKey::parse(target_key)
            .map_err(|_| AppError::invalid_input("invalid target issue key"))?;
        let target = self.issues.get_by_key(&target_key_vo).await?;
        if source.id == target.id {
            return Err(AppError::invalid_input("cannot link an issue to itself"));
        }
        let link = domain::IssueLink {
            id: shared::IssueLinkId::new(),
            source_id: source.id,
            target_id: target.id,
            link_type: lt,
        };
        self.links.save(&link).await?;
        Ok(crate::context::IssueLinkDto {
            id: link.id.to_string(),
            source_id: source.id.to_string(),
            source_key: source.key.to_string(),
            target_id: target.id.to_string(),
            target_key: target.key.to_string(),
            link_type: link.link_type.as_str().to_string(),
        })
    }

    async fn list_by_issue(
        &self,
        issue_id: IssueId,
    ) -> Result<Vec<crate::context::IssueLinkDto>, AppError> {
        let links = self.links.list_by_issue(issue_id).await?;
        let mut out = Vec::with_capacity(links.len());
        for link in links {
            let source = self.issues.get_by_id(link.source_id).await?;
            let target = self.issues.get_by_id(link.target_id).await?;
            out.push(crate::context::IssueLinkDto {
                id: link.id.to_string(),
                source_id: link.source_id.to_string(),
                source_key: source.key.to_string(),
                target_id: link.target_id.to_string(),
                target_key: target.key.to_string(),
                link_type: link.link_type.as_str().to_string(),
            });
        }
        Ok(out)
    }

    async fn delete(
        &self,
        link_id: shared::IssueLinkId,
        _requester: UserId,
    ) -> Result<(), AppError> {
        self.links.delete(link_id).await?;
        Ok(())
    }
}
pub struct WatcherServiceImpl {
    watchers: Arc<dyn domain::WatcherRepository>,
    issues: Arc<dyn IssueRepository>,
    users: Arc<dyn domain::UserRepository>,
    projects: Arc<dyn ProjectRepository>,
    events: crate::context::EventBus,
}

impl WatcherServiceImpl {
    pub fn new(
        watchers: Arc<dyn domain::WatcherRepository>,
        issues: Arc<dyn IssueRepository>,
        users: Arc<dyn domain::UserRepository>,
        projects: Arc<dyn ProjectRepository>,
        events: crate::context::EventBus,
    ) -> Self {
        Self {
            watchers,
            issues,
            users,
            projects,
            events,
        }
    }
}

#[async_trait]
impl crate::context::WatcherService for WatcherServiceImpl {
    async fn watch(&self, issue_id: IssueId, user_id: UserId) -> Result<(), AppError> {
        // Verify the issue exists
        self.issues.get_by_id(issue_id).await?;
        // Verify the user exists
        self.users.get_by_id(user_id).await?;
        self.watchers.add(issue_id, user_id).await?;
        let issue = self.issues.get_by_id(issue_id).await?;
        let project = self.projects.get_by_id(issue.project_id).await?;
        self.events.publish(shared::TrackerEvent::IssueUpdated {
            issue_id: issue_id.to_string(),
            project_key: project.key.to_string(),
        });
        Ok(())
    }

    async fn unwatch(&self, issue_id: IssueId, user_id: UserId) -> Result<(), AppError> {
        self.watchers.remove(issue_id, user_id).await?;
        Ok(())
    }

    async fn list_watchers(
        &self,
        issue_id: IssueId,
    ) -> Result<Vec<crate::context::WatcherDto>, AppError> {
        let watchers = self.watchers.list_by_issue(issue_id).await?;
        let mut dtos = Vec::with_capacity(watchers.len());
        for w in watchers {
            let user = self.users.get_by_id(w.user_id).await?;
            dtos.push(crate::context::WatcherDto {
                user_id: w.user_id.to_string(),
                username: user.username.as_ref().to_string(),
                display_name: user.display_name.as_ref().to_string(),
            });
        }
        Ok(dtos)
    }

    async fn is_watching(&self, issue_id: IssueId, user_id: UserId) -> Result<bool, AppError> {
        self.watchers.is_watching(issue_id, user_id).await
    }
}

pub struct VoteServiceImpl {
    votes: Arc<dyn domain::VoteRepository>,
    issues: Arc<dyn IssueRepository>,
}

impl VoteServiceImpl {
    pub fn new(votes: Arc<dyn domain::VoteRepository>, issues: Arc<dyn IssueRepository>) -> Self {
        Self { votes, issues }
    }
}

#[async_trait]
impl crate::context::VoteService for VoteServiceImpl {
    async fn vote(
        &self,
        issue_id: IssueId,
        user_id: UserId,
    ) -> Result<crate::context::VoteDto, AppError> {
        // Verify the issue exists
        self.issues.get_by_id(issue_id).await?;
        let vote = self.votes.add(issue_id, user_id).await?;
        Ok(crate::context::VoteDto {
            user_id: vote.user_id.to_string(),
            username: String::new(),
            display_name: String::new(),
            voted_at: vote.voted_at.to_rfc3339(),
        })
    }

    async fn unvote(&self, issue_id: IssueId, user_id: UserId) -> Result<(), AppError> {
        self.votes.remove(issue_id, user_id).await?;
        Ok(())
    }

    async fn list_votes(
        &self,
        issue_id: IssueId,
    ) -> Result<Vec<crate::context::VoteDto>, AppError> {
        let votes = self.votes.list_by_issue(issue_id).await?;
        Ok(votes
            .into_iter()
            .map(|v| crate::context::VoteDto {
                user_id: v.user_id.to_string(),
                username: String::new(),
                display_name: String::new(),
                voted_at: v.voted_at.to_rfc3339(),
            })
            .collect())
    }

    async fn count_votes(&self, issue_id: IssueId) -> Result<u64, AppError> {
        self.votes.count_by_issue(issue_id).await
    }

    async fn has_voted(&self, issue_id: IssueId, user_id: UserId) -> Result<bool, AppError> {
        self.votes.has_voted(issue_id, user_id).await
    }
}

pub struct NotificationServiceImpl {
    notifications: Arc<dyn domain::NotificationRepository>,
    settings: Arc<dyn domain::UserNotificationSettingsRepository>,
}

impl NotificationServiceImpl {
    pub fn new(
        notifications: Arc<dyn domain::NotificationRepository>,
        settings: Arc<dyn domain::UserNotificationSettingsRepository>,
    ) -> Self {
        Self {
            notifications,
            settings,
        }
    }

    fn settings_dto(
        settings: domain::NotificationUserSettings,
    ) -> crate::context::NotificationSettingsDto {
        crate::context::NotificationSettingsDto {
            email_frequency: settings.email_frequency.to_string(),
            disabled_event_types: settings
                .disabled_event_types
                .into_iter()
                .map(|value| value.to_string())
                .collect(),
            notify_own_changes: settings.notify_own_changes,
        }
    }
}

#[async_trait]
impl crate::context::NotificationService for NotificationServiceImpl {
    async fn list_unread(
        &self,
        user_id: UserId,
    ) -> Result<crate::context::NotificationListDto, AppError> {
        let notifications = self.notifications.list_unread(user_id).await?;
        let unread_count = notifications.len();
        let mut notifications: Vec<_> = notifications
            .into_iter()
            .map(|notification| crate::context::NotificationDto {
                id: notification.id.to_string(),
                event_type: notification.event_type.to_string(),
                entity_type: notification.entity_type.to_string(),
                entity_id: notification.entity_id.map(|id| id.to_string()),
                actor_id: notification.actor_id.map(|id| id.to_string()),
                title: notification.title.to_string(),
                body: notification.body.map(|body| body.to_string()),
                is_read: notification.is_read,
                action_url: notification.action_url.map(|url| url.to_string()),
                metadata: notification.metadata,
                created_at: notification.created_at.to_rfc3339(),
            })
            .collect();
        notifications.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        notifications.truncate(10);
        Ok(crate::context::NotificationListDto {
            notifications,
            unread_count,
        })
    }

    async fn mark_read(&self, id: String, user_id: UserId) -> Result<(), AppError> {
        let id = id
            .parse::<shared::NotificationId>()
            .map_err(|_| AppError::invalid_input("invalid notification id"))?;
        self.notifications.mark_read(id, user_id).await
    }

    async fn mark_all_read(&self, user_id: UserId) -> Result<(), AppError> {
        self.notifications.mark_all_read(user_id).await
    }

    async fn get_settings(
        &self,
        user_id: UserId,
    ) -> Result<crate::context::NotificationSettingsDto, AppError> {
        match self.settings.get_settings(user_id).await {
            Ok(settings) => Ok(Self::settings_dto(settings)),
            Err(AppError::NotFound(_)) => Ok(crate::context::NotificationSettingsDto {
                email_frequency: "immediate".to_string(),
                disabled_event_types: Vec::new(),
                notify_own_changes: false,
            }),
            Err(error) => Err(error),
        }
    }

    async fn update_settings(
        &self,
        user_id: UserId,
        cmd: crate::commands::UpdateNotificationSettingsCommand,
    ) -> Result<crate::context::NotificationSettingsDto, AppError> {
        if !matches!(
            cmd.email_frequency.as_ref(),
            "immediate" | "hourly" | "daily" | "never"
        ) {
            return Err(AppError::invalid_input("invalid email_frequency"));
        }
        if cmd
            .disabled_event_types
            .iter()
            .any(|event_type| event_type.is_empty() || event_type.len() > 100)
        {
            return Err(AppError::invalid_input("invalid disabled_event_types"));
        }
        let settings = domain::NotificationUserSettings {
            user_id,
            email_frequency: cmd.email_frequency,
            disabled_event_types: cmd.disabled_event_types,
            notify_own_changes: cmd.notify_own_changes,
        };
        self.settings.save_settings(&settings).await?;
        Ok(Self::settings_dto(settings))
    }
}
pub struct ReportServiceImpl {
    issues: Arc<dyn IssueRepository>,
    sprints: Arc<dyn SprintRepository>,
    statuses: Arc<dyn StatusRepository>,
    history: Arc<dyn domain::IssueStatusHistoryRepository>,
}

impl ReportServiceImpl {
    pub fn new(
        issues: Arc<dyn IssueRepository>,
        sprints: Arc<dyn SprintRepository>,
        statuses: Arc<dyn StatusRepository>,
        history: Arc<dyn domain::IssueStatusHistoryRepository>,
    ) -> Self {
        Self {
            issues,
            sprints,
            statuses,
            history,
        }
    }

    fn category_of(&self, status_id: StatusId, statuses: &[domain::Status]) -> StatusCategory {
        statuses
            .iter()
            .find(|s| s.id == status_id)
            .map(|s| s.category)
            .unwrap_or_default()
    }
}

#[async_trait]
impl crate::context::ReportService for ReportServiceImpl {
    async fn get_velocity(
        &self,
        project_id: ProjectId,
        count: u32,
    ) -> Result<Vec<crate::context::VelocitySprintDto>, AppError> {
        let all_sprints = self.sprints.list_by_project(project_id).await?;
        let mut closed: Vec<_> = all_sprints
            .into_iter()
            .filter(|s| matches!(s.state, domain::SprintState::Closed))
            .collect();
        // Sort by end_date descending (most recent first)
        closed.sort_by_key(|s| std::cmp::Reverse(s.end_date));
        closed.truncate(count as usize);

        let statuses = self.statuses.list_all().await.unwrap_or_default();
        let done_status_ids: Vec<StatusId> = statuses
            .iter()
            .filter(|s| s.category == StatusCategory::Done || s.is_closed)
            .map(|s| s.id)
            .collect();

        let mut result = Vec::new();
        for sprint in &closed {
            let issues = self
                .issues
                .list(IssueQuery {
                    project_id: Some(project_id),
                    sprint_id: Some(sprint.id),
                    ..Default::default()
                })
                .await?;
            let committed = issues.len();
            let completed = issues
                .iter()
                .filter(|i| done_status_ids.contains(&i.status_id))
                .count();
            result.push(crate::context::VelocitySprintDto {
                name: sprint.name.as_ref().to_string(),
                committed,
                completed,
            });
        }
        Ok(result)
    }

    async fn get_burndown(
        &self,
        sprint_id: SprintId,
    ) -> Result<crate::context::BurndownDto, AppError> {
        let sprint = self.sprints.get_by_id(sprint_id).await?;
        let project_id = sprint.project_id;
        let issues = self
            .issues
            .list(IssueQuery {
                project_id: Some(project_id),
                sprint_id: Some(sprint_id),
                ..Default::default()
            })
            .await?;
        let total = issues.len();

        let statuses = self.statuses.list_all().await.unwrap_or_default();
        let done_status_ids: Vec<StatusId> = statuses
            .iter()
            .filter(|s| s.category == StatusCategory::Done || s.is_closed)
            .map(|s| s.id)
            .collect();

        let start = sprint.start_date.unwrap_or_else(shared::now);
        let end = sprint
            .end_date
            .unwrap_or_else(|| shared::now() + chrono::Duration::days(14));
        let today = shared::now();
        let effective_end = if end < today { end } else { today };

        let mut points = Vec::new();
        let mut current = start;
        while current <= effective_end {
            // Count issues that were NOT done as of `current`
            // (i.e., issues whose first done transition is after `current` or never done)
            let remaining = issues
                .iter()
                .filter(|issue| {
                    if !done_status_ids.contains(&issue.status_id) {
                        return true; // still open
                    }
                    // Issue is currently done; check if it was done by `current`
                    // For simplicity without per-issue history, assume all done issues
                    // were completed at their updated_at; if updated_at > current, still remaining.
                    issue.updated_at > current
                })
                .count();
            points.push(crate::context::BurndownPointDto {
                date: current.to_rfc3339(),
                remaining,
            });
            current += chrono::Duration::days(1);
        }

        // Ensure at least one point
        if points.is_empty() {
            points.push(crate::context::BurndownPointDto {
                date: start.to_rfc3339(),
                remaining: total,
            });
        }

        Ok(crate::context::BurndownDto {
            sprint_name: sprint.name.as_ref().to_string(),
            points,
        })
    }

    async fn get_cumulative_flow(
        &self,
        project_id: ProjectId,
    ) -> Result<Vec<crate::context::CumulativeFlowPointDto>, AppError> {
        let issues = self.issues.list(IssueQuery::project(project_id)).await?;
        let history = self.history.list_by_project(project_id).await?;
        let statuses = self.statuses.list_all().await.unwrap_or_default();

        // Build a sorted list of all dates from history entries + issue created_at
        let mut dates: Vec<shared::Timestamp> = Vec::new();
        for h in &history {
            dates.push(h.changed_at);
        }
        for issue in &issues {
            dates.push(issue.created_at);
        }
        dates.sort();
        dates.dedup();

        let mut result = Vec::new();
        for &date in &dates {
            let (mut todo, mut in_progress, mut done) = (0usize, 0usize, 0usize);
            for issue in &issues {
                // Determine the status of the issue at `date` by replaying history
                let issue_history: Vec<_> = history
                    .iter()
                    .filter(|h| h.issue_id == issue.id && h.changed_at <= date)
                    .collect();
                let status_id = if let Some(last) = issue_history.last() {
                    last.to_status_id
                } else if issue.created_at <= date {
                    // Before any history, assume the initial status (todo)
                    issue.status_id
                } else {
                    continue; // issue didn't exist yet
                };
                match self.category_of(status_id, &statuses) {
                    StatusCategory::Todo => todo += 1,
                    StatusCategory::InProgress => in_progress += 1,
                    StatusCategory::Done => done += 1,
                }
            }
            result.push(crate::context::CumulativeFlowPointDto {
                date: date.to_rfc3339(),
                todo,
                in_progress,
                done,
            });
        }

        Ok(result)
    }

    async fn get_control_chart(
        &self,
        project_id: ProjectId,
    ) -> Result<Vec<crate::context::ControlChartPointDto>, AppError> {
        let issues = self.issues.list(IssueQuery::project(project_id)).await?;
        let history = self.history.list_by_project(project_id).await?;
        let statuses = self.statuses.list_all().await.unwrap_or_default();
        let done_status_ids: Vec<StatusId> = statuses
            .iter()
            .filter(|s| s.category == StatusCategory::Done || s.is_closed)
            .map(|s| s.id)
            .collect();

        let mut result = Vec::new();
        for issue in &issues {
            // Find the first transition TO a done status
            let done_transition = history
                .iter()
                .filter(|h| h.issue_id == issue.id && done_status_ids.contains(&h.to_status_id))
                .min_by_key(|h| h.changed_at);

            if let Some(dt) = done_transition {
                let cycle_time = (dt.changed_at - issue.created_at).num_seconds() as f64 / 86400.0;
                result.push(crate::context::ControlChartPointDto {
                    issue_key: issue.key.to_string(),
                    cycle_time_days: cycle_time,
                });
            }
        }
        Ok(result)
    }
}

pub struct CustomFieldServiceImpl {
    fields: Arc<dyn domain::CustomFieldRepository>,
    projects: Arc<dyn ProjectRepository>,
    issues: Arc<dyn IssueRepository>,
}

impl CustomFieldServiceImpl {
    pub fn new(
        fields: Arc<dyn domain::CustomFieldRepository>,
        projects: Arc<dyn ProjectRepository>,
        issues: Arc<dyn IssueRepository>,
    ) -> Self {
        Self {
            fields,
            projects,
            issues,
        }
    }

    fn to_dto(f: &domain::CustomField) -> crate::context::CustomFieldDto {
        crate::context::CustomFieldDto {
            id: f.id.to_string(),
            project_id: f.project_id.to_string(),
            name: f.name.as_ref().to_string(),
            field_type: f.field_type.as_str().to_string(),
            options: f.options.iter().map(|o| o.as_ref().to_string()).collect(),
            is_required: f.is_required,
            created_at: f.created_at.to_rfc3339(),
        }
    }
}

#[async_trait]
impl crate::context::CustomFieldService for CustomFieldServiceImpl {
    async fn create_field(
        &self,
        project_key: &ProjectKey,
        name: &str,
        field_type: &str,
        options: &[String],
        is_required: bool,
        _requester: UserId,
    ) -> Result<crate::context::CustomFieldDto, AppError> {
        let project = self.projects.get_by_key(project_key).await?;
        if name.trim().is_empty() {
            return Err(AppError::invalid_input("field name must not be empty"));
        }
        let ft: domain::CustomFieldType = field_type.parse().map_err(AppError::invalid_input)?;
        let field = domain::CustomField {
            id: shared::CustomFieldId::new(),
            project_id: project.id,
            name: name.trim().to_string().into(),
            field_type: ft,
            options: options
                .iter()
                .map(|s| s.trim().to_string().into())
                .collect(),
            is_required,
            created_at: shared::now(),
        };
        self.fields.save(&field).await?;
        Ok(Self::to_dto(&field))
    }

    async fn list_fields(
        &self,
        project_key: &ProjectKey,
    ) -> Result<Vec<crate::context::CustomFieldDto>, AppError> {
        let project = self.projects.get_by_key(project_key).await?;
        let items = self.fields.list_by_project(project.id).await?;
        Ok(items.iter().map(Self::to_dto).collect())
    }

    async fn update_field(
        &self,
        field_id: shared::CustomFieldId,
        name: &str,
        field_type: &str,
        options: &[String],
        is_required: bool,
        _requester: UserId,
    ) -> Result<crate::context::CustomFieldDto, AppError> {
        let mut field = self.fields.get_by_id(field_id).await?;
        if !name.trim().is_empty() {
            field.name = name.trim().to_string().into();
        }
        field.field_type = field_type.parse().map_err(AppError::invalid_input)?;
        field.options = options
            .iter()
            .map(|s| s.trim().to_string().into())
            .collect();
        field.is_required = is_required;
        self.fields.save(&field).await?;
        Ok(Self::to_dto(&field))
    }

    async fn delete_field(
        &self,
        field_id: shared::CustomFieldId,
        _requester: UserId,
    ) -> Result<(), AppError> {
        self.fields.delete(field_id).await?;
        Ok(())
    }

    async fn set_value(
        &self,
        issue_id: IssueId,
        field_id: shared::CustomFieldId,
        value: serde_json::Value,
        _requester: UserId,
    ) -> Result<(), AppError> {
        // Validate the issue and field exist.
        let _issue = self.issues.get_by_id(issue_id).await?;
        let _field = self.fields.get_by_id(field_id).await?;
        self.fields.set_value(issue_id, field_id, &value).await?;
        Ok(())
    }

    async fn get_values_for_issue(
        &self,
        issue_id: IssueId,
    ) -> Result<Vec<crate::context::CustomFieldValueDto>, AppError> {
        let values = self.fields.get_values_for_issue(issue_id).await?;
        Ok(values
            .into_iter()
            .map(|v| crate::context::CustomFieldValueDto {
                field_id: v.field_id.to_string(),
                value: v.value,
            })
            .collect())
    }
}

pub struct ComponentServiceImpl {
    components: Arc<dyn domain::ProjectComponentRepository>,
    projects: Arc<dyn ProjectRepository>,
}

impl ComponentServiceImpl {
    pub fn new(
        components: Arc<dyn domain::ProjectComponentRepository>,
        projects: Arc<dyn ProjectRepository>,
    ) -> Self {
        Self {
            components,
            projects,
        }
    }

    fn to_dto(c: &domain::ProjectComponent) -> crate::context::ComponentDto {
        crate::context::ComponentDto {
            id: c.id.to_string(),
            project_id: c.project_id.to_string(),
            name: c.name.as_ref().to_string(),
            description: c.description.as_ref().map(|d| d.as_ref().to_string()),
            created_at: c.created_at.to_rfc3339(),
        }
    }
}

#[async_trait]
impl crate::context::ComponentService for ComponentServiceImpl {
    async fn create(
        &self,
        project_key: &ProjectKey,
        name: &str,
        description: Option<&str>,
    ) -> Result<crate::context::ComponentDto, AppError> {
        let project = self.projects.get_by_key(project_key).await?;
        if name.trim().is_empty() {
            return Err(AppError::invalid_input("component name must not be empty"));
        }
        let component = domain::ProjectComponent {
            id: shared::ProjectComponentId::new(),
            project_id: project.id,
            name: name.trim().to_string().into(),
            description: description.map(|d| d.to_string().into()),
            created_at: shared::now(),
        };
        self.components.save(&component).await?;
        Ok(Self::to_dto(&component))
    }

    async fn list_by_project(
        &self,
        project_key: &ProjectKey,
    ) -> Result<Vec<crate::context::ComponentDto>, AppError> {
        let project = self.projects.get_by_key(project_key).await?;
        let items = self.components.list_by_project(project.id).await?;
        Ok(items.iter().map(Self::to_dto).collect())
    }

    async fn update(
        &self,
        id: shared::ProjectComponentId,
        name: &str,
        description: Option<&str>,
    ) -> Result<crate::context::ComponentDto, AppError> {
        let mut component = self.components.get_by_id(id).await?;
        if !name.trim().is_empty() {
            component.name = name.trim().to_string().into();
        }
        component.description = description.map(|d| d.to_string().into());
        self.components.save(&component).await?;
        Ok(Self::to_dto(&component))
    }

    async fn delete(&self, id: shared::ProjectComponentId) -> Result<(), AppError> {
        self.components.delete(id).await?;
        Ok(())
    }
}

pub struct VersionServiceImpl {
    versions: Arc<dyn domain::ProjectVersionRepository>,
    projects: Arc<dyn ProjectRepository>,
}

impl VersionServiceImpl {
    pub fn new(
        versions: Arc<dyn domain::ProjectVersionRepository>,
        projects: Arc<dyn ProjectRepository>,
    ) -> Self {
        Self { versions, projects }
    }

    fn to_dto(v: &domain::ProjectVersion) -> crate::context::VersionDto {
        crate::context::VersionDto {
            id: v.id.to_string(),
            project_id: v.project_id.to_string(),
            name: v.name.as_ref().to_string(),
            description: v.description.as_ref().map(|d| d.as_ref().to_string()),
            released: v.released,
            release_date: v.release_date.map(|d| d.to_rfc3339()),
            created_at: v.created_at.to_rfc3339(),
        }
    }
}

#[async_trait]
impl crate::context::VersionService for VersionServiceImpl {
    async fn create(
        &self,
        project_key: &ProjectKey,
        name: &str,
        description: Option<&str>,
        released: bool,
        release_date: Option<chrono::DateTime<chrono::FixedOffset>>,
    ) -> Result<crate::context::VersionDto, AppError> {
        let project = self.projects.get_by_key(project_key).await?;
        if name.trim().is_empty() {
            return Err(AppError::invalid_input("version name must not be empty"));
        }
        let version = domain::ProjectVersion {
            id: shared::ProjectVersionId::new(),
            project_id: project.id,
            name: name.trim().to_string().into(),
            description: description.map(|d| d.to_string().into()),
            released,
            release_date,
            created_at: shared::now(),
        };
        self.versions.save(&version).await?;
        Ok(Self::to_dto(&version))
    }

    async fn list_by_project(
        &self,
        project_key: &ProjectKey,
    ) -> Result<Vec<crate::context::VersionDto>, AppError> {
        let project = self.projects.get_by_key(project_key).await?;
        let items = self.versions.list_by_project(project.id).await?;
        Ok(items.iter().map(Self::to_dto).collect())
    }

    async fn update(
        &self,
        id: shared::ProjectVersionId,
        name: &str,
        description: Option<&str>,
        released: bool,
        release_date: Option<Option<chrono::DateTime<chrono::FixedOffset>>>,
    ) -> Result<crate::context::VersionDto, AppError> {
        let mut version = self.versions.get_by_id(id).await?;
        if !name.trim().is_empty() {
            version.name = name.trim().to_string().into();
        }
        version.description = description.map(|d| d.to_string().into());
        version.released = released;
        if let Some(rd) = release_date {
            version.release_date = rd;
        }
        self.versions.save(&version).await?;
        Ok(Self::to_dto(&version))
    }

    async fn delete(&self, id: shared::ProjectVersionId) -> Result<(), AppError> {
        self.versions.delete(id).await?;
        Ok(())
    }
}
