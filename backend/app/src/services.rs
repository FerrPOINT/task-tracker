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
use shared::{AppError, BoardId, IssueId, ProjectId, ProjectKey, SprintId, StatusId, UserId};

mod helpers;

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
}

impl IssueServiceImpl {
    pub fn new(
        issues: Arc<dyn IssueRepository>,
        projects: Arc<dyn ProjectRepository>,
        boards: Arc<dyn BoardRepository>,
        users: Arc<dyn domain::UserRepository>,
        statuses: Arc<dyn StatusRepository>,
        transitions: Arc<dyn WorkflowTransitionRepository>,
    ) -> Self {
        Self {
            issues,
            projects,
            boards,
            users,
            statuses,
            transitions,
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
                cmd.priority.clone(),
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

        self.issues.save(&issue).await?;
        let statuses = self.statuses.list_all().await.unwrap_or_default();
        let column = statuses
            .iter()
            .find(|s| s.id == issue.status_id)
            .map(|s| s.name.as_ref().to_string())
            .unwrap_or_else(|| helpers::issue_status_column(issue.status_id));
        let (assignee_name, reporter_name) =
            helpers::resolve_names(self.users.clone(), &issue).await;
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
}

impl CommentServiceImpl {
    pub fn new(
        comments: Arc<dyn domain::CommentRepository>,
        users: Arc<dyn domain::UserRepository>,
        issues: Arc<dyn domain::IssueRepository>,
    ) -> Self {
        Self {
            comments,
            users,
            issues,
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
        if let Ok(existing) = self.members.get(cmd.project_id, cmd.user_id).await {
            return Ok(ProjectMemberDto {
                project_id: existing.project_id.to_string(),
                user_id: existing.user_id.to_string(),
                role: existing.role.as_str().to_string(),
                joined_at: existing.joined_at,
            });
        }
        let member = ProjectMember {
            project_id: cmd.project_id,
            user_id: cmd.user_id,
            role,
            joined_at: shared::now(),
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
