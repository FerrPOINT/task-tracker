//! Transport authorization belongs to the private API machine factory. This
//! in-process deadline only attenuates that verified authority across storage I/O.
pub use pdlc_shared::identity_lifecycle::{
    DeletionRequest, OwnerFreezeReceipt, OwnerReferenceSummary, OwnerReleaseReceipt,
};
use std::time::Instant;
#[derive(Clone)]
pub struct IdentityDeletionWindow {
    deadline: Instant,
    valid_until: i64,
}
impl IdentityDeletionWindow {
    pub fn restricted(deadline: Instant, valid_until: i64) -> Self {
        Self {
            deadline,
            valid_until,
        }
    }
    pub fn is_current(&self) -> bool {
        Instant::now() < self.deadline && chrono::Utc::now().timestamp() < self.valid_until
    }
    /// Translate the original monotonic budget using a database clock sampled
    /// before lock waits. The returned cap can only shorten that authority.
    pub fn commit_valid_until(
        &self,
        database_anchor: chrono::DateTime<chrono::Utc>,
    ) -> Result<chrono::DateTime<chrono::Utc>, crate::AppError> {
        self.require_current()?;
        let remaining = self
            .deadline
            .checked_duration_since(Instant::now())
            .ok_or(crate::AppError::Unauthorized)?;
        let duration =
            chrono::Duration::from_std(remaining).map_err(|_| crate::AppError::Unauthorized)?;
        let monotonic_cap = database_anchor
            .checked_add_signed(duration)
            .ok_or(crate::AppError::Unauthorized)?;
        let wall_cap = chrono::DateTime::from_timestamp(self.valid_until, 0)
            .ok_or(crate::AppError::Unauthorized)?;
        Ok(monotonic_cap.min(wall_cap))
    }
    pub fn require_current(&self) -> Result<(), crate::AppError> {
        if self.is_current() {
            Ok(())
        } else {
            Err(crate::AppError::Unauthorized)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn database_clock_translation_never_renews_original_authority() {
        let now = chrono::Utc::now();
        let wall = now.timestamp() + 60;
        let window = IdentityDeletionWindow::restricted(
            Instant::now() + std::time::Duration::from_secs(5),
            wall,
        );
        let behind = now - chrono::Duration::hours(1);
        let bound = window.commit_valid_until(behind).unwrap();
        assert!(bound <= behind + chrono::Duration::seconds(5));
        assert!(
            bound < now,
            "a lagging DB clock must not add an hour of authority"
        );
        let ahead = now + chrono::Duration::hours(1);
        assert_eq!(
            window.commit_valid_until(ahead).unwrap().timestamp(),
            wall,
            "the original wall expiry also caps an advanced database clock"
        );
        let expired = IdentityDeletionWindow::restricted(Instant::now(), wall);
        assert!(matches!(
            expired.commit_valid_until(now),
            Err(crate::AppError::Unauthorized)
        ));
    }
}
