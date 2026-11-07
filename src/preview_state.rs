use crate::worktree::Snapshot;
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PreviewTarget {
    File(PathBuf),
    Stash(git2::Oid),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreviewRequest {
    pub repository: PathBuf,
    pub target: PreviewTarget,
    pub revision: u64,
}
#[derive(Clone, Default)]
pub enum PreviewState {
    #[default]
    Idle,
    Loading {
        request: PreviewRequest,
        previous: Option<Snapshot>,
    },
    Ready {
        request: PreviewRequest,
        snapshot: Snapshot,
    },
    Failed {
        request: PreviewRequest,
        error: String,
    },
}
impl PreviewState {
    pub fn begin(&mut self, request: Option<PreviewRequest>) {
        let Some(request) = request else {
            *self = Self::Idle;
            return;
        };
        let previous = self
            .request()
            .filter(|old| old.repository == request.repository && old.target == request.target)
            .and_then(|_| self.snapshot())
            .cloned();
        *self = Self::Loading { request, previous };
    }
    pub fn request(&self) -> Option<&PreviewRequest> {
        match self {
            Self::Idle => None,
            Self::Loading { request, .. }
            | Self::Ready { request, .. }
            | Self::Failed { request, .. } => Some(request),
        }
    }
    pub fn snapshot(&self) -> Option<&Snapshot> {
        match self {
            Self::Ready { snapshot, .. } => Some(snapshot),
            Self::Loading { previous, .. } => previous.as_ref(),
            _ => None,
        }
    }
    pub fn finish(&mut self, request: PreviewRequest, result: Result<Snapshot, String>) -> bool {
        if self.request() != Some(&request) {
            return false;
        }
        *self = match result {
            Ok(snapshot) => Self::Ready { request, snapshot },
            Err(error) => Self::Failed { request, error },
        };
        true
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn request(file: &str, revision: u64) -> PreviewRequest {
        PreviewRequest {
            repository: "/repo".into(),
            target: PreviewTarget::File(file.into()),
            revision,
        }
    }
    #[test]
    fn changed_target_hides_previous_content_and_rejects_old_completion() {
        let mut state = PreviewState::default();
        let a = request("a", 1);
        let b = request("b", 2);
        state.begin(Some(a.clone()));
        state.finish(a.clone(), Ok(Snapshot::default()));
        state.begin(Some(b.clone()));
        assert!(state.snapshot().is_none());
        assert!(!state.finish(a, Ok(Snapshot::default())));
        assert!(state.finish(b, Err("unavailable".into())));
        assert!(matches!(state, PreviewState::Failed { .. }));
    }
    #[test]
    fn same_target_refresh_preserves_content_but_rejects_previous_generation() {
        let mut state = PreviewState::default();
        let a = request("a", 1);
        state.begin(Some(a.clone()));
        state.finish(a.clone(), Ok(Snapshot::default()));
        let next = request("a", 2);
        state.begin(Some(next.clone()));
        assert!(state.snapshot().is_some());
        assert!(!state.finish(a, Ok(Snapshot::default())));
        assert!(state.finish(next, Ok(Snapshot::default())));
    }
    #[test]
    fn stash_switch_never_retains_another_stash_preview() {
        let mut state = PreviewState::default();
        let mut a = request("", 1);
        a.target = PreviewTarget::Stash(git2::Oid::from_str(&"1".repeat(40)).unwrap());
        state.begin(Some(a.clone()));
        state.finish(a.clone(), Ok(Snapshot::default()));
        a.target = PreviewTarget::Stash(git2::Oid::from_str(&"2".repeat(40)).unwrap());
        a.revision += 1;
        state.begin(Some(a));
        assert!(state.snapshot().is_none());
    }
}
