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
    pub hash: String,
    pub author: String,
    pub date: String,
    pub message: String,
}
