use std::ffi::OsStr;
use std::path::Path;
use std::process::Command;

use chrono::TimeZone;
use git2::{BranchType, Oid, Repository, Sort, StatusOptions};

use crate::models::{
    CommitGraph, CommitLane, CommitLaneStatus, FileStatusSummary, LogEntry, RemoteInfo, RepoDetail,
    RepoInfo, SubmoduleCommitLink, SubmoduleInfo,
};

pub fn scan_repos(parent_dir: &Path) -> Vec<RepoInfo> {
    let mut repos = Vec::new();
    let entries = match std::fs::read_dir(parent_dir) {
        Ok(e) => e,
        Err(_) => return repos,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if let Some(info) = build_repo_info(&path) {
            repos.push(info);
        }
    }
    repos.sort_by_cached_key(|r| r.name.to_lowercase());
    repos
}

pub fn build_repo_info(path: &Path) -> Option<RepoInfo> {
    let repo = Repository::open(path).ok()?;
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();

    let is_dirty = check_dirty(&repo);
    let current_branch = get_branch_name(&repo);
    let (ahead, behind) = get_ahead_behind(&repo, &current_branch);
    let submodules = list_submodules(path, &repo);

    Some(RepoInfo {
        name,
        path: path.to_path_buf(),
        is_dirty,
        ahead,
        behind,
        current_branch,
        submodules,
    })
}

pub fn list_local_branches(repo_path: &Path) -> Vec<String> {
    let repo = match Repository::open(repo_path) {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };
    let mut branches: Vec<String> = repo
        .branches(Some(BranchType::Local))
        .into_iter()
        .flatten()
        .filter_map(|b| b.ok())
        .filter_map(|(b, _)| b.name().ok().flatten().map(String::from))
        .collect();
    branches.sort_by_cached_key(|branch| branch.to_lowercase());
    branches
}

pub fn has_upstream(repo_path: &Path, branch_name: &str) -> bool {
    let repo = match Repository::open(repo_path) {
        Ok(r) => r,
        Err(_) => return false,
    };
    let branch = match repo.find_branch(branch_name, BranchType::Local) {
        Ok(b) => b,
        Err(_) => return false,
    };
    branch.upstream().is_ok()
}

fn run_git<I, S>(repo_path: &Path, args: I) -> Result<String, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = Command::new("git")
        .args(args)
        .current_dir(repo_path)
        .output()
        .map_err(|e| format!("Failed to run git: {e}"))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

pub fn checkout_branch(repo_path: &Path, branch: &str) -> Result<String, String> {
    run_git(repo_path, ["checkout", branch])
}

pub fn pull_rebase(repo_path: &Path) -> Result<String, String> {
    run_git(repo_path, ["pull", "--rebase"])
}

pub fn push(repo_path: &Path) -> Result<String, String> {
    run_git(repo_path, ["push"])
}

pub fn push_set_upstream(repo_path: &Path, branch: &str) -> Result<String, String> {
    run_git(repo_path, ["push", "-u", "origin", branch])
}

pub fn fetch_remote(repo_path: &Path, remote: &str) -> Result<String, String> {
    run_git(repo_path, ["fetch", remote])
}

/// Fetch a remote and make the current local branch exactly match its
/// corresponding remote-tracking branch. This intentionally discards local
/// commits and working-tree changes, so callers must confirm with the user.
pub fn reset_to_remote_branch(
    repo_path: &Path,
    remote: &str,
    branch: &str,
) -> Result<String, String> {
    if branch == "HEAD detached" {
        return Err("Cannot reset a detached HEAD to a remote branch".to_string());
    }
    fetch_remote(repo_path, remote)?;
    let target = format!("{remote}/{branch}");
    run_git(repo_path, ["reset", "--hard", &target])
}

pub fn init_submodule(repo_path: &Path, relative_path: &Path) -> Result<String, String> {
    run_git(
        repo_path,
        [
            OsStr::new("submodule"),
            OsStr::new("update"),
            OsStr::new("--init"),
            OsStr::new("--"),
            relative_path.as_os_str(),
        ],
    )
}

pub fn get_repo_detail(repo_path: &Path) -> Option<RepoDetail> {
    let repo = Repository::open(repo_path).ok()?;
    let current_branch = get_branch_name(&repo);
    let branches = list_local_branches(repo_path);
    let remotes = list_remotes(&repo);
    let file_status = build_file_status(&repo);

    Some(RepoDetail {
        path: repo_path.display().to_string(),
        current_branch,
        branches,
        remotes,
        head_labels: collect_branch_heads(&repo),
        file_status,
    })
}

fn list_submodules(repo_path: &Path, repo: &Repository) -> Vec<SubmoduleInfo> {
    let mut submodules: Vec<SubmoduleInfo> = repo
        .submodules()
        .ok()
        .into_iter()
        .flatten()
        .map(|submodule| {
            let relative_path = submodule.path().to_path_buf();
            let path = repo_path.join(&relative_path);
            let name = submodule
                .name()
                .ok()
                .map(String::from)
                .or_else(|| {
                    relative_path
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                })
                .unwrap_or_else(|| relative_path.display().to_string());
            let url = submodule.url().ok().flatten().map(String::from);
            let repo = Repository::open(&path).ok();
            let is_initialized = repo.is_some();
            let (current_branch, is_dirty, ahead, behind) = repo
                .as_ref()
                .map(|repo| {
                    let current_branch = get_branch_name(repo);
                    let is_dirty = check_dirty(repo);
                    let (ahead, behind) = get_ahead_behind(repo, &current_branch);
                    (current_branch, is_dirty, ahead, behind)
                })
                .unwrap_or_else(|| ("Not initialized".to_string(), false, 0, 0));

            SubmoduleInfo {
                name,
                path,
                relative_path,
                url,
                is_initialized,
                is_dirty,
                ahead,
                behind,
                current_branch,
            }
        })
        .collect();

    submodules.sort_by_cached_key(|s| s.name.to_lowercase());
    submodules
}

pub fn get_commit_log(repo_path: &Path, limit: usize) -> Vec<LogEntry> {
    let repo = match Repository::open(repo_path) {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };
    read_commit_log(&repo, limit)
}

pub fn get_commit_graph(repo_info: &RepoInfo, limit: usize) -> Option<CommitGraph> {
    get_commit_graph_for_branches(repo_info, &[], limit)
}

pub fn get_commit_graph_for_branches(
    repo_info: &RepoInfo,
    branches: &[String],
    limit: usize,
) -> Option<CommitGraph> {
    let repo = Repository::open(&repo_info.path).ok()?;
    let main_entries = read_commit_log_for_branches(&repo, branches, limit);
    let mut lanes = Vec::with_capacity(repo_info.submodules.len() + 1);
    lanes.push(CommitLane {
        id: "main".to_string(),
        name: repo_info.name.clone(),
        relative_path: None,
        status: CommitLaneStatus::Available,
        entries: main_entries.clone(),
    });

    for submodule in &repo_info.submodules {
        let id = submodule_lane_id(&submodule.relative_path);
        let (status, entries) = if !submodule.is_initialized {
            (CommitLaneStatus::Uninitialized, Vec::new())
        } else if let Ok(submodule_repo) = Repository::open(&submodule.path) {
            (
                CommitLaneStatus::Available,
                read_commit_log(&submodule_repo, limit),
            )
        } else {
            (CommitLaneStatus::Unavailable, Vec::new())
        };
        lanes.push(CommitLane {
            id,
            name: submodule.name.clone(),
            relative_path: Some(submodule.relative_path.clone()),
            status,
            entries,
        });
    }

    let mut submodule_links = Vec::new();
    for entry in &main_entries {
        let Ok(oid) = Oid::from_str(&entry.full_hash) else {
            continue;
        };
        let Ok(commit) = repo.find_commit(oid) else {
            continue;
        };
        let Ok(tree) = commit.tree() else {
            continue;
        };
        for submodule in &repo_info.submodules {
            let Ok(tree_entry) = tree.get_path(&submodule.relative_path) else {
                continue;
            };
            if tree_entry.filemode() != 0o160000 {
                continue;
            }
            submodule_links.push(SubmoduleCommitLink {
                main_commit: entry.full_hash.clone(),
                submodule_lane: submodule_lane_id(&submodule.relative_path),
                submodule_commit: tree_entry.id().to_string(),
            });
        }
    }

    Some(CommitGraph {
        repository_path: repo_info.path.clone(),
        lanes,
        submodule_links,
        head_labels: collect_branch_heads(&repo),
    })
}

fn collect_branch_heads(repo: &Repository) -> std::collections::HashMap<String, Vec<String>> {
    let mut labels = std::collections::HashMap::<String, Vec<String>>::new();
    for branch in repo
        .branches(None)
        .into_iter()
        .flatten()
        .filter_map(|branch| branch.ok())
    {
        let (branch, _) = branch;
        let Ok(Some(name)) = branch.name() else {
            continue;
        };
        let Some(oid) = branch.get().target() else {
            continue;
        };
        labels
            .entry(oid.to_string())
            .or_default()
            .push(name.to_string());
    }
    for names in labels.values_mut() {
        names.sort_by_key(|name| (name.contains('/'), name.to_lowercase()));
    }
    if let Ok(head) = repo.head()
        && let Some(oid) = head.target()
    {
        labels
            .entry(oid.to_string())
            .or_default()
            .insert(0, "HEAD".into());
    }
    labels
}

fn submodule_lane_id(relative_path: &Path) -> String {
    format!("submodule:{}", relative_path.to_string_lossy())
}

fn read_commit_log(repo: &Repository, limit: usize) -> Vec<LogEntry> {
    read_commit_log_for_branches(repo, &[], limit)
}

fn read_commit_log_for_branches(
    repo: &Repository,
    branches: &[String],
    limit: usize,
) -> Vec<LogEntry> {
    let mut entries = Vec::new();
    let mut revwalk = match repo.revwalk() {
        Ok(r) => r,
        Err(_) => return entries,
    };
    let branch_names = if branches.is_empty() {
        default_history_branches(repo)
    } else {
        branches.to_vec()
    };
    for branch_name in &branch_names {
        if let Ok(branch) = repo.find_branch(branch_name, BranchType::Local)
            && let Some(oid) = branch.get().target()
        {
            revwalk.push(oid).ok();
        }
    }
    // Include the matching remote-tracking branches for every selected local
    // branch. This makes the canvas compare local and remote histories by
    // default without requiring a separate remote branch picker.
    for remote_branch in repo
        .branches(Some(BranchType::Remote))
        .into_iter()
        .flatten()
        .filter_map(|branch| branch.ok())
    {
        let Ok(Some(name)) = remote_branch.0.name() else {
            continue;
        };
        let remote_leaf = name
            .split_once('/')
            .map(|(_, branch)| branch)
            .unwrap_or(name);
        if branch_names.iter().any(|branch| branch == remote_leaf)
            && let Some(oid) = remote_branch.0.get().target()
        {
            revwalk.push(oid).ok();
        }
    }
    if branches.is_empty() {
        revwalk.push_head().ok();
    }
    revwalk.set_sorting(Sort::TOPOLOGICAL | Sort::TIME).ok();

    for oid in revwalk.flatten().take(limit) {
        let commit = match repo.find_commit(oid) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let full_hash = commit.id().to_string();
        let hash = full_hash[..7.min(full_hash.len())].to_string();
        let parent_hashes = commit.parent_ids().map(|id| id.to_string()).collect();
        let author = commit.author().name().unwrap_or("unknown").to_string();
        let time = commit.time();
        let date = chrono::Utc
            .timestamp_opt(time.seconds(), 0)
            .single()
            .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
            .unwrap_or_default();
        let message = commit
            .message()
            .unwrap_or("")
            .lines()
            .next()
            .unwrap_or("")
            .to_string();

        entries.push(LogEntry {
            full_hash,
            hash,
            parent_hashes,
            author,
            date,
            message,
        });
    }
    entries
}

fn default_history_branches(repo: &Repository) -> Vec<String> {
    let current = get_branch_name(repo);
    let primary = ["main", "master"]
        .into_iter()
        .find(|branch| repo.find_branch(branch, BranchType::Local).is_ok())
        .unwrap_or(&current);
    let mut branches = vec![primary.to_string(), current];
    branches.sort();
    branches.dedup();
    branches
}

/// Shared status configuration so the dirty flag and the per-category counts
/// observe the same set of files: untracked files (recursing into untracked
/// dirs) are included, ignored files are excluded, and rename detection is on
/// in both the index and the work tree.
fn status_options() -> StatusOptions {
    let mut opts = StatusOptions::new();
    opts.include_untracked(true);
    opts.recurse_untracked_dirs(true);
    opts.renames_head_to_index(true);
    opts.renames_index_to_workdir(true);
    opts
}

fn list_remotes(repo: &Repository) -> Vec<RemoteInfo> {
    let Ok(names) = repo.remotes() else {
        return Vec::new();
    };
    let mut remotes: Vec<RemoteInfo> = names
        .iter()
        .flatten()
        .flatten()
        .map(String::from)
        .map(|name| RemoteInfo {
            url: repo
                .find_remote(&name)
                .ok()
                .and_then(|remote| remote.url().ok().map(String::from)),
            name,
        })
        .collect();
    remotes.sort_by_cached_key(|remote| remote.name.to_lowercase());
    remotes
}

fn check_dirty(repo: &Repository) -> bool {
    match repo.statuses(Some(&mut status_options())) {
        Ok(statuses) => statuses.iter().any(|s| {
            !s.status()
                .intersects(git2::Status::IGNORED | git2::Status::CURRENT)
        }),
        Err(_) => false,
    }
}

fn get_branch_name(repo: &Repository) -> String {
    repo.head()
        .ok()
        .and_then(|r| r.shorthand().ok().map(String::from))
        .unwrap_or_else(|| "HEAD detached".into())
}

fn get_ahead_behind(repo: &Repository, branch_name: &str) -> (usize, usize) {
    let local = match repo.find_branch(branch_name, BranchType::Local) {
        Ok(b) => b,
        Err(_) => return (0, 0),
    };
    let upstream = match local.upstream() {
        Ok(u) => u,
        Err(_) => return (0, 0),
    };
    let local_oid = match local.get().target() {
        Some(o) => o,
        None => return (0, 0),
    };
    let upstream_oid = match upstream.get().target() {
        Some(o) => o,
        None => return (0, 0),
    };
    repo.graph_ahead_behind(local_oid, upstream_oid)
        .unwrap_or((0, 0))
}

fn build_file_status(repo: &Repository) -> FileStatusSummary {
    let mut summary = FileStatusSummary::default();
    let statuses = match repo.statuses(Some(&mut status_options())) {
        Ok(s) => s,
        Err(_) => return summary,
    };
    for entry in statuses.iter() {
        let s = entry.status();
        if s.intersects(git2::Status::WT_NEW | git2::Status::INDEX_NEW) {
            summary.new_files += 1;
        }
        if s.intersects(git2::Status::WT_MODIFIED | git2::Status::INDEX_MODIFIED) {
            summary.modified += 1;
        }
        if s.intersects(git2::Status::WT_DELETED | git2::Status::INDEX_DELETED) {
            summary.deleted += 1;
        }
        if s.intersects(git2::Status::WT_RENAMED | git2::Status::INDEX_RENAMED) {
            summary.renamed += 1;
        }
        if s.intersects(git2::Status::CONFLICTED) {
            summary.conflicted += 1;
        }
    }
    summary
}
