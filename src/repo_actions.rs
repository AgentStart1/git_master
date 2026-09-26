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
                let remote = repo
                    .refname_to_id(&format!("refs/remotes/{name}/{remote_branch}"))
                    .ok()?;
                repo.graph_ahead_behind(local, remote).ok()
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
