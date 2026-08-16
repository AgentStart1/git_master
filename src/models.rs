use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct RepoInfo {
    pub name: String,
    pub path: PathBuf,
    pub is_dirty: bool,
    pub ahead: usize,
    pub behind: usize,
    pub current_branch: String,
    pub submodules: Vec<SubmoduleInfo>,
}

#[derive(Clone, Debug)]
pub struct RepoDetail {
    pub path: String,
    pub current_branch: String,
    pub remote_url: Option<String>,
    pub file_status: FileStatusSummary,
}

#[derive(Clone, Debug)]
pub struct SubmoduleInfo {
    pub name: String,
    pub path: PathBuf,
    pub relative_path: PathBuf,
    pub url: Option<String>,
    pub is_initialized: bool,
    pub is_dirty: bool,
    pub ahead: usize,
    pub behind: usize,
    pub current_branch: String,
}

#[derive(Clone, Debug)]
pub struct SubmoduleDetail {
    pub name: String,
    pub path: String,
    pub url: Option<String>,
    pub is_initialized: bool,
}

#[derive(Clone, Debug, Default)]
pub struct FileStatusSummary {
    pub new_files: usize,
    pub modified: usize,
    pub deleted: usize,
    pub renamed: usize,
    pub conflicted: usize,
}

#[derive(Clone, Debug)]
pub struct LogEntry {
    pub full_hash: String,
    pub hash: String,
    pub parent_hashes: Vec<String>,
    pub author: String,
    pub date: String,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CommitLaneStatus {
    Available,
    Uninitialized,
    Unavailable,
}

#[derive(Clone, Debug)]
pub struct CommitLane {
    pub id: String,
    pub name: String,
    pub relative_path: Option<PathBuf>,
    pub status: CommitLaneStatus,
    pub entries: Vec<LogEntry>,
}

#[derive(Clone, Debug)]
pub struct SubmoduleCommitLink {
    pub main_commit: String,
    pub submodule_lane: String,
    pub submodule_commit: String,
}

#[derive(Clone, Debug)]
pub struct CommitGraph {
    pub repository_path: PathBuf,
    pub lanes: Vec<CommitLane>,
    pub submodule_links: Vec<SubmoduleCommitLink>,
}
