use std::path::Path;

use git2::{BranchType, Repository};

use crate::models::RemoteStatus;

pub fn remote_statuses(repo: &Repository) -> Vec<RemoteStatus> {
    let Ok(remotes) = repo.remotes() else {
        return Vec::new();
    };
    let head = repo.head().ok();
    let branch = head
        .as_ref()
        .filter(|head| head.is_branch())
        .and_then(|head| head.shorthand().ok())
        .unwrap_or("HEAD detached");
    let local = head.as_ref().and_then(|head| head.target());
    let mut statuses = remotes
        .iter()
        .flatten()
        .flatten()
        .map(|name| {
            // Honor an explicitly configured upstream branch name for its remote.
            let config = repo.config().ok();
            let tracked_branch = config.as_ref().and_then(|config| {
                (config
                    .get_string(&format!("branch.{branch}.remote"))
                    .ok()
                    .as_deref()
                    == Some(name))
                .then(|| config.get_string(&format!("branch.{branch}.merge")).ok())
                .flatten()
            });
            let remote_branch = tracked_branch
                .as_deref()
                .and_then(|name| name.strip_prefix("refs/heads/"))
                .unwrap_or(branch);
            let counts = local.and_then(|local| {
                let remote = repo.find_remote(name).ok()?;
                let source = format!("refs/heads/{remote_branch}");
                let remote_oid = remote.refspecs().find_map(|spec| {
                    if spec.direction() != git2::Direction::Fetch || !spec.src_matches(&source) {
                        return None;
                    }
                    let destination = spec.transform(&source).ok()?;
                    repo.refname_to_id(destination.as_str().ok()?).ok()
                })?;
                repo.graph_ahead_behind(local, remote_oid).ok()
            });
            RemoteStatus {
                name: name.to_owned(),
                counts,
            }
        })
        .collect::<Vec<_>>();
    statuses.sort_by_cached_key(|status| status.name.to_lowercase());
    statuses
}

pub(crate) fn main_branch(repo: &Repository) -> Result<(String, Option<String>), String> {
    let remotes = repo.remotes().map_err(|error| error.to_string())?;
    let mut defaults = remotes
        .iter()
        .flatten()
        .flatten()
        .filter_map(|remote| {
            let reference = repo
                .find_reference(&format!("refs/remotes/{remote}/HEAD"))
                .ok()?;
            let prefix = format!("refs/remotes/{remote}/");
            let branch = reference
                .symbolic_target()
                .ok()??
                .strip_prefix(&prefix)?
                .to_owned();
            Some((branch, Some(remote.to_owned())))
        })
        .collect::<Vec<_>>();
    if let Some(index) = defaults
        .iter()
        .position(|(_, remote)| remote.as_deref() == Some("origin"))
    {
        return Ok(defaults.remove(index));
    }
    if let Some(first) = defaults.first() {
        if defaults.iter().all(|(branch, _)| branch == &first.0) {
            return Ok(first.clone());
        }
        return Err("Remotes disagree on the default branch".into());
    }
    for branch in ["main", "master"] {
        if repo.find_branch(branch, BranchType::Local).is_ok() {
            return Ok((branch.to_owned(), None));
        }
    }
    Err("Cannot determine main branch (no remote HEAD or local main/master)".into())
}

pub(crate) fn switch_preview_state(repo: &Repository, branch: &str) -> String {
    if repo.state() != git2::RepositoryState::Clean {
        return "Git 操作进行中".into();
    }
    let head = match repo.head() {
        Ok(head) => Some(head),
        Err(error) if error.code() == git2::ErrorCode::UnbornBranch => None,
        Err(error) => return format!("读取当前分支失败：{error}"),
    };
    if head.as_ref().is_some_and(|head| {
        head.is_branch() && head.name().ok() == Some(format!("refs/heads/{branch}").as_str())
    }) {
        return "已在目标分支".into();
    }
    let mut options = git2::StatusOptions::new();
    options.include_ignored(false).include_untracked(true);
    match repo.statuses(Some(&mut options)) {
        Ok(statuses) if statuses.is_empty() => "可切换".into(),
        Ok(_) => "可能受本地修改阻止".into(),
        Err(error) => format!("读取 Git 状态失败：{error}"),
    }
}

pub fn switch_to_main(path: &Path) -> Result<String, String> {
    let repo = Repository::open(path).map_err(|error| error.to_string())?;
    let (branch, remote) = main_branch(&repo)?;
    if repo.find_branch(&branch, BranchType::Local).is_ok() {
        crate::git_ops::run_git(path, ["switch", "--", &branch])
    } else if let Some(remote) = remote {
        crate::git_ops::run_git(
            path,
            [
                "switch",
                "--track",
                "-c",
                &branch,
                &format!("refs/remotes/{remote}/{branch}"),
            ],
        )
    } else {
        Err(format!("Local main branch {branch} is unavailable"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn preview_repo(bare: bool) -> (tempfile::TempDir, Repository) {
        let dir = tempfile::tempdir().unwrap();
        let repo = if bare {
            Repository::init_bare(dir.path()).unwrap()
        } else {
            Repository::init(dir.path()).unwrap()
        };
        let signature = git2::Signature::now("Test", "test@example.invalid").unwrap();
        let tree_id = repo.treebuilder(None).unwrap().write().unwrap();
        let commit_id = repo
            .commit(
                Some("refs/heads/main"),
                &signature,
                &signature,
                "initial",
                &repo.find_tree(tree_id).unwrap(),
                &[],
            )
            .unwrap();
        repo.reference("refs/heads/feature", commit_id, false, "test")
            .unwrap();
        repo.set_head("refs/heads/feature").unwrap();
        (dir, repo)
    }

    #[test]
    fn switch_preview_ignores_ignored_files() {
        let (dir, repo) = preview_repo(false);
        std::fs::write(dir.path().join("ignored.tmp"), "cache").unwrap();
        std::fs::write(repo.path().join("info/exclude"), "ignored.tmp\n").unwrap();
        assert_eq!(switch_preview_state(&repo, "main"), "可切换");
    }

    #[test]
    fn switch_preview_warns_about_untracked_files() {
        let (dir, repo) = preview_repo(false);
        std::fs::write(dir.path().join("local.txt"), "local").unwrap();
        assert_eq!(switch_preview_state(&repo, "main"), "可能受本地修改阻止");
    }

    #[test]
    fn switch_preview_recognizes_target_branch_even_with_local_changes() {
        let (dir, repo) = preview_repo(false);
        repo.set_head("refs/heads/main").unwrap();
        std::fs::write(dir.path().join("local.txt"), "local").unwrap();
        assert_eq!(switch_preview_state(&repo, "main"), "已在目标分支");
    }

    #[test]
    fn switch_preview_reports_status_read_failure() {
        let (_dir, repo) = preview_repo(true);
        assert!(switch_preview_state(&repo, "main").starts_with("读取 Git 状态失败："));
    }

    #[test]
    fn remote_counts_follow_fetch_refspecs_and_configured_upstream_names() {
        let path = std::env::temp_dir().join(format!(
            "git-master-refspec-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let repo = Repository::init(&path).unwrap();
        let signature = git2::Signature::now("Test", "test@example.invalid").unwrap();
        let tree = repo
            .find_tree(repo.treebuilder(None).unwrap().write().unwrap())
            .unwrap();
        let base = repo
            .commit(None, &signature, &signature, "base", &tree, &[])
            .unwrap();
        let parent = repo.find_commit(base).unwrap();
        let local = repo
            .commit(
                Some("refs/heads/feature"),
                &signature,
                &signature,
                "local",
                &tree,
                &[&parent],
            )
            .unwrap();
        let divergent = repo
            .commit(None, &signature, &signature, "remote", &tree, &[&parent])
            .unwrap();
        repo.set_head("refs/heads/feature").unwrap();
        for remote in ["origin", "mirror", "unmapped"] {
            repo.remote(remote, path.to_str().unwrap()).unwrap();
        }
        let mut config = repo.config().unwrap();
        config
            .set_str(
                "remote.origin.fetch",
                "+refs/heads/*:refs/remotes/company/*",
            )
            .unwrap();
        config.set_str("branch.feature.remote", "origin").unwrap();
        config
            .set_str("branch.feature.merge", "refs/heads/main")
            .unwrap();
        config
            .set_str(
                "remote.unmapped.fetch",
                "+refs/heads/main:refs/remotes/unmapped/main",
            )
            .unwrap();
        repo.reference("refs/remotes/company/main", base, true, "test")
            .unwrap();
        repo.reference("refs/remotes/mirror/feature", divergent, true, "test")
            .unwrap();
        // A stale default-named ref must not override the configured mapping.
        repo.reference("refs/remotes/origin/main", divergent, true, "test")
            .unwrap();
        repo.reference("refs/remotes/unmapped/feature", local, true, "test")
            .unwrap();
        let statuses = remote_statuses(&repo);
        let counts = |name| {
            statuses
                .iter()
                .find(|status| status.name == name)
                .unwrap()
                .counts
        };
        assert_eq!(counts("origin"), Some((1, 0)));
        assert_eq!(counts("mirror"), Some((1, 1)));
        assert_eq!(counts("unmapped"), None);
        drop((config, parent, tree));
        drop(repo);
        std::fs::remove_dir_all(path).unwrap();
    }
}
