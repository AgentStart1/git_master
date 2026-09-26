use std::path::PathBuf;

use crate::{git_ops, models::RepoInfo};

/// Framework-independent batch worker, owned by the application's background task.
pub struct BatchHost {
    paths: Vec<PathBuf>,
    action: BatchAction,
}

#[derive(Clone, Copy)]
pub enum BatchAction {
    Fetch,
    SwitchMain,
}

#[cfg(test)]
mod tests {
    use super::*;
    use git2::{Repository, Signature};

    #[test]
    fn fetches_every_remote_and_continues_after_failures() {
        let root = std::env::temp_dir().join(format!(
            "git-master-fetch-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let source = Repository::init_bare(root.join("source")).unwrap();
        let tree_id = source.treebuilder(None).unwrap().write().unwrap();
        let tree = source.find_tree(tree_id).unwrap();
        let signature = Signature::now("Test", "test@example.invalid").unwrap();
        let commit = source
            .commit(
                Some("refs/heads/main"),
                &signature,
                &signature,
                "initial",
                &tree,
                &[],
            )
            .unwrap();
        let first = Repository::init(root.join("first")).unwrap();
        first
            .remote("a-broken", root.join("missing").to_str().unwrap())
            .unwrap();
        for name in ["origin", "upstream"] {
            first.remote(name, source.path().to_str().unwrap()).unwrap();
        }
        first
            .config()
            .unwrap()
            .set_bool("remote.upstream.skipFetchAll", true)
            .unwrap();
        let second = Repository::init(root.join("second")).unwrap();
        second
            .remote("origin", source.path().to_str().unwrap())
            .unwrap();
        let empty = Repository::init(root.join("empty")).unwrap();
        std::fs::write(root.join("first/local.txt"), "keep me").unwrap();
        let result = BatchHost::new(
            vec![
                root.join("invalid"),
                root.join("first"),
                root.join("empty"),
                root.join("second"),
            ],
            BatchAction::Fetch,
        )
        .run();
        assert_eq!((result.succeeded, result.failed, result.skipped), (3, 2, 1));
        assert_eq!(result.refreshed.len(), 4);
        for (repo, remote) in [
            (&first, "origin"),
            (&first, "upstream"),
            (&second, "origin"),
        ] {
            assert_eq!(
                repo.refname_to_id(&format!("refs/remotes/{remote}/main"))
                    .unwrap(),
                commit
            );
        }
        assert_eq!(
            std::fs::read_to_string(root.join("first/local.txt")).unwrap(),
            "keep me"
        );
        first
            .reference_symbolic(
                "refs/remotes/origin/HEAD",
                "refs/remotes/origin/main",
                true,
                "test",
            )
            .unwrap();
        let switched = BatchHost::new(
            vec![root.join("empty"), root.join("first")],
            BatchAction::SwitchMain,
        )
        .run();
        assert_eq!((switched.succeeded, switched.failed), (1, 1));
        assert_eq!(first.head().unwrap().shorthand().unwrap(), "main");
        let statuses = crate::repo_actions::remote_statuses(&first);
        assert_eq!(statuses.len(), 3);
        assert_eq!(
            statuses.iter().find(|s| s.name == "origin").unwrap().counts,
            Some((0, 0))
        );
        assert_eq!(
            statuses
                .iter()
                .find(|s| s.name == "a-broken")
                .unwrap()
                .counts,
            None
        );
        let base = first.find_commit(commit).unwrap();
        let local_tree = base.tree().unwrap();
        let local = first
            .commit(None, &signature, &signature, "local", &local_tree, &[&base])
            .unwrap();
        let other = first
            .commit(
                None,
                &signature,
                &signature,
                "remote",
                &local_tree,
                &[&base],
            )
            .unwrap();
        first
            .reference("refs/heads/feature", local, true, "test")
            .unwrap();
        first.set_head("refs/heads/feature").unwrap();
        first
            .reference("refs/remotes/upstream/feature", other, true, "test")
            .unwrap();
        first
            .config()
            .unwrap()
            .set_str("branch.feature.remote", "origin")
            .unwrap();
        first
            .config()
            .unwrap()
            .set_str("branch.feature.merge", "refs/heads/main")
            .unwrap();
        let statuses = crate::repo_actions::remote_statuses(&first);
        let origin = statuses.iter().find(|s| s.name == "origin").unwrap();
        assert_eq!(origin.counts, Some((1, 0)));
        assert_eq!(
            statuses
                .iter()
                .find(|s| s.name == "upstream")
                .unwrap()
                .counts,
            Some((1, 1))
        );
        drop((local_tree, base));
        crate::repo_actions::switch_to_main(&root.join("first")).unwrap();
        assert_eq!(first.head().unwrap().shorthand().unwrap(), "main");
        assert_eq!(
            std::fs::read_to_string(root.join("first/local.txt")).unwrap(),
            "keep me"
        );
        drop(tree);
        drop((source, first, second, empty));
        std::fs::remove_dir_all(root).unwrap();
    }
}

pub struct BatchResult {
    action: BatchAction,
    pub refreshed: Vec<(PathBuf, Option<RepoInfo>)>,
    pub succeeded: usize,
    pub failed: usize,
    pub skipped: usize,
}

impl BatchResult {
    pub fn message(&self) -> String {
        let summary = match self.action {
            BatchAction::SwitchMain => format!(
                "Switch to Main: {} succeeded, {} failed",
                self.succeeded, self.failed
            ),
            BatchAction::Fetch => format!(
                "Fetch All: {} succeeded, {} failed, {} projects without remotes",
                self.succeeded, self.failed, self.skipped
            ),
        };
        if self.failed > 0 {
            format!(
                "{summary}. Log: {}",
                crate::operation_log::log_path().display()
            )
        } else {
            summary
        }
    }
}

impl BatchHost {
    pub fn new(paths: Vec<PathBuf>, action: BatchAction) -> Self {
        Self { paths, action }
    }

    pub fn run(self) -> BatchResult {
        let mut result = BatchResult {
            action: self.action,
            refreshed: Vec::new(),
            succeeded: 0,
            failed: 0,
            skipped: 0,
        };
        for path in self.paths {
            if matches!(self.action, BatchAction::SwitchMain) {
                match crate::repo_actions::switch_to_main(&path) {
                    Ok(_) => result.succeeded += 1,
                    Err(error) => {
                        result.failed += 1;
                        let _ = crate::operation_log::append(&format!(
                            "Switch to Main repo={path:?}: {error}"
                        ));
                    }
                }
                let refreshed = git_ops::build_repo_info(&path);
                result.refreshed.push((path, refreshed));
                continue;
            }
            // Enumerate explicitly: remotes marked skipFetchAll must also be fetched.
            let remotes = git2::Repository::open(&path).and_then(|repo| {
                let names = repo.remotes()?;
                names
                    .iter()
                    .map(|name| {
                        name?
                            .map(str::to_owned)
                            .ok_or_else(|| git2::Error::from_str("Remote name is not UTF-8"))
                    })
                    .collect::<Result<Vec<_>, _>>()
            });
            match remotes {
                Ok(remotes) => {
                    if remotes.is_empty() {
                        result.skipped += 1;
                    }
                    for remote in remotes {
                        match git_ops::fetch_remote(&path, &remote) {
                            Ok(_) => result.succeeded += 1,
                            Err(_) => result.failed += 1,
                        }
                    }
                }
                Err(error) => {
                    result.failed += 1;
                    let _ = crate::operation_log::append(&format!(
                        "Fetch All: cannot enumerate remotes repo={path:?}: {error}"
                    ));
                }
            }
            let refreshed = git_ops::build_repo_info(&path);
            result.refreshed.push((path, refreshed));
        }
        result
    }
}
