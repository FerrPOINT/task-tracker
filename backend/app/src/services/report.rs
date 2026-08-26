use async_trait::async_trait;
use std::sync::Arc;

use crate::authz::Authz;
use domain::{IssueQuery, IssueRepository, SprintRepository, StatusCategory, StatusRepository};
use shared::{AppError, ProjectId, SprintId, StatusId, UserId};

pub struct ReportServiceImpl {
    issues: Arc<dyn IssueRepository>,
    sprints: Arc<dyn SprintRepository>,
    statuses: Arc<dyn StatusRepository>,
    history: Arc<dyn domain::IssueStatusHistoryRepository>,
    authz: Authz,
}

impl ReportServiceImpl {
    pub fn new(
        issues: Arc<dyn IssueRepository>,
        sprints: Arc<dyn SprintRepository>,
        statuses: Arc<dyn StatusRepository>,
        history: Arc<dyn domain::IssueStatusHistoryRepository>,
        authz: Authz,
    ) -> Self {
        Self {
            issues,
            sprints,
            statuses,
            history,
            authz,
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
        requester: UserId,
    ) -> Result<Vec<crate::context::VelocitySprintDto>, AppError> {
        self.authz
            .require_project_access(project_id, requester)
            .await?;
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
        requester: UserId,
    ) -> Result<crate::context::BurndownDto, AppError> {
        let sprint = self.sprints.get_by_id(sprint_id).await?;
        self.authz
            .require_project_access(sprint.project_id, requester)
            .await?;
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
            let remaining = issues
                .iter()
                .filter(|issue| {
                    if !done_status_ids.contains(&issue.status_id) {
                        return true; // still open
                    }
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
        requester: UserId,
    ) -> Result<Vec<crate::context::CumulativeFlowPointDto>, AppError> {
        self.authz
            .require_project_access(project_id, requester)
            .await?;
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
                let issue_history: Vec<_> = history
                    .iter()
                    .filter(|h| h.issue_id == issue.id && h.changed_at <= date)
                    .collect();
                let status_id = if let Some(last) = issue_history.last() {
                    last.to_status_id
                } else if issue.created_at <= date {
                    issue.status_id
                } else {
                    continue;
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
        requester: UserId,
    ) -> Result<Vec<crate::context::ControlChartPointDto>, AppError> {
        self.authz
            .require_project_access(project_id, requester)
            .await?;
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
