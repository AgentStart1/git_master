use std::path::Path;
use std::process::Command;

use chrono::TimeZone;
use git2::{BranchType, Repository, Sort, StatusOptions};

use crate::models::{FileStatusSummary, LogEntry, RepoDetail, RepoInfo};

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

    Some(RepoInfo {
        name,
        path: path.to_path_buf(),
        is_dirty,
        ahead,
        behind,
        current_branch,
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
    branches.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));
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

fn run_git(repo_path: &Path, args: &[&str]) -> Result<String, String> {
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
    run_git(repo_path, &["checkout", branch])
}

pub fn pull_rebase(repo_path: &Path) -> Result<String, String> {
    run_git(repo_path, &["pull", "--rebase"])
}

pub fn push(repo_path: &Path) -> Result<String, String> {
    run_git(repo_path, &["push"])
}

pub fn push_set_upstream(repo_path: &Path, branch: &str) -> Result<String, String> {
    run_git(repo_path, &["push", "-u", "origin", branch])
}

pub fn get_repo_detail(repo_path: &Path) -> Option<RepoDetail> {
    let repo = Repository::open(repo_path).ok()?;
    let current_branch = get_branch_name(&repo);
    let remote_url = first_remote_url(&repo);
    let file_status = build_file_status(&repo);

    Some(RepoDetail {
        path: repo_path.display().to_string(),
        current_branch,
        remote_url,
        file_status,
    })
}

pub fn get_commit_log(repo_path: &Path, limit: usize) -> Vec<LogEntry> {
    let mut entries = Vec::new();
    let repo = match Repository::open(repo_path) {
        Ok(r) => r,
        Err(_) => return entries,
    };
    let mut revwalk = match repo.revwalk() {
        Ok(r) => r,
        Err(_) => return entries,
    };
    revwalk.push_head().ok();
    revwalk.set_sorting(Sort::TIME).ok();

    for oid in revwalk.flatten().take(limit) {
        let commit = match repo.find_commit(oid) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let hash = commit.id().to_string();
        let hash = hash[..7.min(hash.len())].to_string();
        let author = commit
            .author()
            .name()
            .unwrap_or("unknown")
            .to_string();
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
            hash,
            author,
            date,
            message,
        });
    }
    entries
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

fn first_remote_url(repo: &Repository) -> Option<String> {
    if let Ok(origin) = repo.find_remote("origin")
        && let Ok(url) = origin.url()
    {
        return Some(url.to_string());
    }
    let names = repo.remotes().ok()?;
    for name in names.iter().flatten().flatten() {
        if let Ok(remote) = repo.find_remote(name)
            && let Ok(url) = remote.url()
        {
            return Some(url.to_string());
        }
    }
    None
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
