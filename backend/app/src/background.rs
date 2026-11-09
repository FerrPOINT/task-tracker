//! Project roles for Fleet research. Caller must independently pass project admission.
use domain::ProjectRole;

pub fn permissions(
    owner: bool,
    member: Option<&ProjectRole>,
    administrator: bool,
    writable: bool,
) -> (bool, bool) {
    let manage =
        owner || administrator || matches!(member, Some(ProjectRole::Admin | ProjectRole::Owner));
    let triage = owner || administrator || member.is_some();
    (manage && writable, triage && writable)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn roles_do_not_turn_read_access_into_management() {
        assert_eq!(permissions(false, None, false, true), (false, false));
        assert_eq!(
            permissions(false, Some(&ProjectRole::Member), false, true),
            (false, true)
        );
        assert_eq!(
            permissions(false, Some(&ProjectRole::Admin), false, true),
            (true, true)
        );
        assert_eq!(permissions(true, None, false, true), (true, true));
        assert_eq!(permissions(false, None, true, true), (true, true));
        for role in [ProjectRole::Member, ProjectRole::Admin, ProjectRole::Owner] {
            assert_eq!(permissions(true, Some(&role), true, false), (false, false));
        }
    }
}
