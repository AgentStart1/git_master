//! Toolkit-independent request ownership for progressive history expansion.
#[derive(Default)]
pub struct HistoryHost {
    generation: u64,
    loading: Option<u64>,
    auto_failed: bool,
}

impl HistoryHost {
    pub fn can_auto_load(&self) -> bool {
        !self.auto_failed && !self.is_loading()
    }

    pub fn failed(&mut self) {
        self.auto_failed = true;
    }

    pub fn is_loading(&self) -> bool {
        self.loading.is_some()
    }

    pub fn start(&mut self) -> Option<u64> {
        if self.is_loading() {
            return None;
        }
        self.auto_failed = false;
        self.generation += 1;
        self.loading = Some(self.generation);
        self.loading
    }

    pub fn finish(&mut self, request: u64) -> bool {
        if self.loading != Some(request) {
            return false;
        }
        self.loading = None;
        true
    }

    pub fn cancel(&mut self) {
        self.auto_failed = false;
        self.loading = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancellation_and_duplicate_requests_cannot_publish_over_a_new_load() {
        let mut host = HistoryHost::default();
        let old = host.start().unwrap();
        assert!(host.start().is_none());
        host.cancel();
        let new = host.start().unwrap();
        assert!(!host.finish(old));
        assert!(host.is_loading());
        assert!(host.finish(new));
        assert!(!host.finish(new));
        assert!(!host.is_loading());
    }

    #[test]
    fn automatic_failure_waits_for_manual_retry_and_navigation_clears_it() {
        let mut host = HistoryHost::default();
        let request = host.start().unwrap();
        assert!(host.finish(request));
        host.failed();
        assert!(!host.can_auto_load());
        let retry = host.start().unwrap();
        assert!(host.finish(retry));
        assert!(host.can_auto_load());
        host.failed();
        host.cancel();
        assert!(host.can_auto_load());
    }
}
