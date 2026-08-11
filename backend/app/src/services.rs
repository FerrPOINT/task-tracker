use async_trait::async_trait;
use std::str::FromStr;
use std::sync::Arc;

use crate::commands::{
    CreateCommentCommand, CreateIssueCommand, CreateWorklogCommand, UpdateCommentCommand,
    UpdateIssueCommand, UpdateWorklogCommand,
};
use crate::dto::{
    BacklogDto, BoardColumnDto, BoardDto, CommentDto, DashboardDto, IssueDto, ProjectDto,
    ProjectMemberDto, SprintDto, WorklogDto,
};
use domain::{
    Board, BoardRepository, ColumnCategory, Issue, IssueQuery, IssueRepository, ProjectMember,
    ProjectMemberRepository, ProjectRepository, ProjectRole, SprintRepository, UserRepository,
};
use shared::{AppError, BoardId, IssueId, ProjectId, ProjectKey, StatusId, UserId};

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
}

impl IssueServiceImpl {
    pub fn new(
        issues: Arc<dyn IssueRepository>,
        projects: Arc<dyn ProjectRepository>,
        boards: Arc<dyn BoardRepository>,
        users: Arc<dyn domain::UserRepository>,
    ) -> Self {
        Self {
            issues,
            projects,
            boards,
            users,
        }
    }
}

#[async_trait]
impl crate::context::IssueService for IssueServiceImpl {
    async fn create(&self, cmd: CreateIssueCommand) -> Result<IssueDto, AppError> {
        let project = self.projects.get_by_key(&cmd.project_key).await?;
        let number = self.projects.next_issue_number(project.id).await?;
        let status_id = StatusId::from_uuid(
            cmd.status_id
                .parse()
                .map_err(|_| AppError::invalid_input("status_id"))?,
        );
        let mut issue = Issue::create(
            &project,
            number,
            cmd.issue_type,
            status_id,
            cmd.summary,
            cmd.description.map(domain::RichText::from),
            cmd.reporter_id,
            cmd.priority,
        );
        if let Some(assignee_id) = cmd.assignee_id {
            issue.assign(Some(assignee_id));
        }
        self.issues.save(&issue).await?;
        let column = helpers::issue_status_column(issue.status_id);
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
        let valid = board.columns.iter().any(|c| c.id == cmd.target_status_id);
        if !valid {
            return Err(AppError::invalid_input("invalid target status"));
        }
        let mut updated = issue.clone();
        updated.status_id = cmd.target_status_id;
        updated.updated_at = shared::now();
        self.issues.save(&updated).await?;
        let project = self.projects.get_by_id(updated.project_id).await?;
        let status = board
            .columns
            .iter()
            .find(|c| c.id == updated.status_id)
            .map(|c| c.name.as_ref().to_string())
            .unwrap_or_default();
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
            issue.change_status(StatusId::from_uuid(sid));
        }
        if let Some(assignee_id) = cmd.assignee_id {
            issue.assign(assignee_id);
        }

        self.issues.save(&issue).await?;
        let column = helpers::issue_status_column(issue.status_id);
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

    async fn search(&self, q: &str) -> Result<Vec<IssueDto>, AppError> {
        let issues = self
            .issues
            .list(IssueQuery {
                search_text: Some(q.to_string()),
                ..Default::default()
            })
            .await?;
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
}

impl BoardServiceImpl {
    pub fn new(
        boards: Arc<dyn domain::BoardRepository>,
        issues: Arc<dyn IssueRepository>,
        sprints: Arc<dyn SprintRepository>,
        users: Arc<dyn domain::UserRepository>,
    ) -> Self {
        Self {
            boards,
            issues,
            sprints,
            users,
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

        let columns: Vec<BoardColumnDto> = board
            .columns
            .iter()
            .map(|c| BoardColumnDto {
                id: c.id.to_string(),
                name: c.name.as_ref().to_string(),
                wip_limit: c.wip_limit,
                issue_ids: issues
                    .iter()
                    .filter(|i| i.status_id == c.id)
                    .map(|i| i.id.to_string())
                    .collect(),
            })
            .collect();

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

        let todo_status = board
            .columns
            .iter()
            .find(|c| c.category == ColumnCategory::Todo)
            .map(|c| c.id)
            .unwrap_or(StatusId::from_uuid(uuid::Uuid::nil()));

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
        let mut issue = self.issues.get_by_id(issue_id).await?;
        issue.change_status(status_id);
        self.issues.save(&issue).await?;
        self.build_board_dto(project_key).await
    }
}

pub struct DashboardServiceImpl {
    issues: Arc<dyn IssueRepository>,
    projects: Arc<dyn ProjectRepository>,
    users: Arc<dyn domain::UserRepository>,
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
    async fn search(&self, q: &str) -> Result<Vec<IssueDto>, AppError> {
        let issues = self
            .issues
            .list(IssueQuery {
                search_text: Some(q.to_string()),
                ..Default::default()
            })
            .await?;
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
