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

fn main_branch(repo: &Repository) -> Result<(String, Option<String>), String> {
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
