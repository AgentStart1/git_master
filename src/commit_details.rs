use std::path::Path;

/// Read a single immutable commit, including full message, identities, dates,
/// parents and changed-file statistics. No working-tree diff is involved.
pub fn load(path: &Path, hash: &str) -> Result<String, String> {
    let oid = git2::Oid::from_str(hash).map_err(|error| error.to_string())?;
    crate::git_ops::run_git(
        path,
        [
            "--no-pager",
            "show",
            "--no-ext-diff",
            "--no-textconv",
            "--no-color",
            "--encoding=UTF-8",
            "--format=Commit: %H%nParents: %P%nAuthor: %an <%ae>%nAuthor date: %aI%nCommitter: %cn <%ce>%nCommit date: %cI%n%n%B",
            "--date=iso-strict",
            "--stat",
            "--numstat",
            "--stat-width=100",
            "--stat-name-width=80",
            "--summary",
            "--root",
            "--diff-merges=first-parent",
            &oid.to_string(),
            "--",
        ],
    )
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    #[test]
    fn reads_full_message_identities_files_and_parents() {
        let path = std::env::temp_dir().join(format!(
            "git-master-commit-details-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let repo = git2::Repository::init(&path).unwrap();
        let signature = git2::Signature::now("Test Author", "test@example.invalid").unwrap();
        let blob = repo.blob(b"first line\n").unwrap();
        let mut builder = repo.treebuilder(None).unwrap();
        builder.insert("example.txt", blob, 0o100644).unwrap();
        let tree = repo.find_tree(builder.write().unwrap()).unwrap();
        let root = repo
            .commit(
                None,
                &signature,
                &signature,
                "Subject\n\nComplete message body",
                &tree,
                &[],
            )
            .unwrap();
        let text = super::load(&path, &root.to_string()).unwrap();
        for expected in [
            root.to_string().as_str(),
            "Complete message body",
            "test@example.invalid",
            "example.txt",
            "Author date:",
            "Commit date:",
        ] {
            assert!(text.contains(expected), "missing {expected}");
        }
        let parent = repo.find_commit(root).unwrap();
        let child = repo
            .commit(None, &signature, &signature, "child", &tree, &[&parent])
            .unwrap();
        let text = super::load(&path, &child.to_string()).unwrap();
        assert!(text.contains(&format!("Parents: {root}")));
        assert!(super::load(&path, "not-a-hash").is_err());
        std::fs::write(path.join("example.txt"), "uncommitted work\n").unwrap();
        let index_before = std::fs::read(repo.path().join("index")).ok();
        let rows =
            super::load_file_diff(&path, &root.to_string(), Path::new("example.txt")).unwrap();
        assert!(
            rows.iter()
                .any(|r| r.kind == crate::diff_model::RowKind::Added && r.text == "first line")
        );
        assert!(!rows.iter().any(|r| r.text.contains("uncommitted work")));
        assert_eq!(
            std::fs::read_to_string(path.join("example.txt")).unwrap(),
            "uncommitted work\n"
        );
        assert_eq!(std::fs::read(repo.path().join("index")).ok(), index_before);

        drop((parent, tree, builder));
        drop(repo);
        std::fs::remove_dir_all(path).unwrap();
    }
}

#[derive(Clone, Debug, Default)]
pub struct Presentation {
    pub title: String,
    pub description: String,
    pub markdown: crate::markdown::Document,
    pub author: String,
    pub date: String,
    pub hash: String,
    pub parents: String,
    pub files: Vec<FileStat>,
}
#[derive(Clone, Debug)]
pub struct FileStat {
    pub path: std::path::PathBuf,
    pub display_name: String,
    pub added: String,
    pub removed: String,
}
impl Presentation {
    pub fn load(path: &Path, hash: &str) -> Result<Self, String> {
        let repo = git2::Repository::open(path).map_err(|e| e.to_string())?;
        let commit = repo
            .find_commit(git2::Oid::from_str(hash).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        let (author, message) = decoded_metadata(path, hash)?;
        let (title, description) = message.split_once('\n').unwrap_or((&message, ""));
        let description = description.trim_matches(['\r', '\n']);
        let signature = commit.author();
        let mut result = Self {
            title: title.into(),
            description: description.into(),
            markdown: crate::markdown::Document::parse(description),
            author: author.into(),
            date: chrono::DateTime::from_timestamp(signature.when().seconds(), 0)
                .map(|d| {
                    match chrono::FixedOffset::east_opt(signature.when().offset_minutes() * 60) {
                        Some(offset) => d.with_timezone(&offset).to_rfc3339(),
                        None => d.to_rfc3339(),
                    }
                })
                .unwrap_or_default(),
            hash: commit.id().to_string(),
            parents: commit
                .parent_ids()
                .map(|id| id.to_string())
                .collect::<Vec<_>>()
                .join(" "),
            files: Vec::new(),
        };
        let diff = commit_diff(&repo, &commit)?;
        for (index, delta) in diff.deltas().enumerate() {
            let Some(path) = delta.new_file().path().or_else(|| delta.old_file().path()) else {
                continue;
            };
            let display_name = match (delta.old_file().path(), delta.new_file().path()) {
                (Some(old), Some(new)) if old != new => {
                    format!("{} → {}", old.display(), new.display())
                }
                _ => path.display().to_string(),
            };
            let patch = git2::Patch::from_diff(&diff, index).map_err(|e| e.to_string())?;
            let (added, removed) = match patch {
                Some(patch) => {
                    let (_, added, removed) = patch.line_stats().map_err(|e| e.to_string())?;
                    (added.to_string(), removed.to_string())
                }
                None if delta.flags().contains(git2::DiffFlags::BINARY) => ("-".into(), "-".into()),
                None => ("0".into(), "0".into()),
            };
            result.files.push(FileStat {
                path: path.to_owned(),
                display_name,
                added,
                removed,
            });
        }
        Ok(result)
    }
}

pub(crate) fn decoded_metadata(path: &Path, hash: &str) -> Result<(String, String), String> {
    // Git honors the commit's encoding header. Explicit output encoding also
    // isolates the UI and clipboard from repository log-output preferences.
    let oid = git2::Oid::from_str(hash).map_err(|e| e.to_string())?;
    let metadata = crate::git_ops::run_git(
        path,
        [
            "show",
            "--no-patch",
            "--encoding=UTF-8",
            "--format=%an <%ae>%x00%B",
            &oid.to_string(),
            "--",
        ],
    )?;
    metadata
        .split_once('\0')
        .map(|(author, message)| (author.to_owned(), message.to_owned()))
        .ok_or_else(|| "Missing commit metadata separator".to_owned())
}

fn commit_diff<'repo>(
    repo: &'repo git2::Repository,
    commit: &git2::Commit<'repo>,
) -> Result<git2::Diff<'repo>, String> {
    let tree = commit.tree().map_err(|e| e.to_string())?;
    let parent = if commit.parent_count() > 0 {
        Some(
            commit
                .parent(0)
                .and_then(|p| p.tree())
                .map_err(|e| e.to_string())?,
        )
    } else {
        None
    };
    let mut diff = repo
        .diff_tree_to_tree(parent.as_ref(), Some(&tree), None)
        .map_err(|e| e.to_string())?;
    diff.find_similar(Some(git2::DiffFindOptions::new().renames(true)))
        .map_err(|e| e.to_string())?;
    Ok(diff)
}

/// Read a selected immutable commit file against its first parent (or the empty tree).
pub fn load_file_diff(
    path: &Path,
    hash: &str,
    file: &Path,
) -> Result<Vec<crate::diff_model::DiffRow>, String> {
    let repo = git2::Repository::open(path).map_err(|e| e.to_string())?;
    let commit = repo
        .find_commit(git2::Oid::from_str(hash).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    let diff = commit_diff(&repo, &commit)?;
    crate::diff_preview::DiffRenderer::detect()
        .render_selected(&repo, diff, false, Some(file))
        .map(|d| d.rows)
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod path_tests {
    use super::*;
    #[test]
    fn structured_paths_roundtrip_unicode_spaces_tabs_and_rename_without_writes() {
        let temp = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(temp.path()).unwrap();
        let signature = git2::Signature::now("Tester", "test@example.invalid").unwrap();
        let blob = repo.blob(b"before\n").unwrap();
        let mut builder = repo.treebuilder(None).unwrap();
        for name in [
            "old.txt",
            "中文 空格.txt",
            "tab\tname.txt",
            "brace {old => new}.txt",
        ] {
            builder.insert(name, blob, 0o100644).unwrap();
        }
        let tree = repo.find_tree(builder.write().unwrap()).unwrap();
        let base = repo
            .commit(None, &signature, &signature, "base", &tree, &[])
            .unwrap();
        let parent = repo.find_commit(base).unwrap();
        let mut next = repo.treebuilder(Some(&tree)).unwrap();
        next.remove("old.txt").unwrap();
        next.insert("new name.txt", blob, 0o100644).unwrap();
        let changed = repo.blob(b"after\n").unwrap();
        for name in ["中文 空格.txt", "tab\tname.txt", "brace {old => new}.txt"] {
            next.insert(name, changed, 0o100644).unwrap();
        }
        let tree2 = repo.find_tree(next.write().unwrap()).unwrap();
        let oid = repo
            .commit(
                None,
                &signature,
                &signature,
                "Subject\n\n12\t3\tmessage is not a stat",
                &tree2,
                &[&parent],
            )
            .unwrap();
        let before_index = std::fs::read(repo.path().join("index")).ok();
        let presentation = Presentation::load(temp.path(), &oid.to_string()).unwrap();
        assert_eq!(presentation.description, "12\t3\tmessage is not a stat");
        assert_eq!(presentation.files.len(), 4);
        let renamed = presentation
            .files
            .iter()
            .find(|f| f.path == Path::new("new name.txt"))
            .unwrap();
        assert_eq!(renamed.display_name, "old.txt → new name.txt");
        for file in presentation.files {
            let rows = load_file_diff(temp.path(), &oid.to_string(), &file.path).unwrap();
            assert!(!rows.is_empty(), "empty preview for {:?}", file.path);
        }
        let root = Presentation::load(temp.path(), &base.to_string()).unwrap();
        assert_eq!(root.files.len(), 4);
        for file in root.files {
            assert!(
                !load_file_diff(temp.path(), &base.to_string(), &file.path)
                    .unwrap()
                    .is_empty()
            );
        }
        assert_eq!(std::fs::read(repo.path().join("index")).ok(), before_index);
        assert!(!temp.path().join("new name.txt").exists());
    }
}

#[cfg(test)]
mod encoding_tests {
    #[test]
    fn declared_legacy_encoding_is_consistent_in_presentation_markdown_and_copy() {
        let temp = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(temp.path()).unwrap();
        repo.config()
            .unwrap()
            .set_str("i18n.logOutputEncoding", "ISO-8859-1")
            .unwrap();
        let tree = repo.treebuilder(None).unwrap().write().unwrap();
        let mut bytes = format!("tree {tree}\nauthor Andr").into_bytes();
        bytes.extend_from_slice(b"\xe9 <test@example.invalid> 1700000000 +0000\ncommitter Test <test@example.invalid> 1700000000 +0000\nencoding ISO-8859-1\n\nR\xe9sum\xe9\n\n**Caf\xe9**\n");
        let oid = repo
            .odb()
            .unwrap()
            .write(git2::ObjectType::Commit, &bytes)
            .unwrap();
        let index_before = std::fs::read(repo.path().join("index")).ok();
        let result = super::Presentation::load(temp.path(), &oid.to_string()).unwrap();
        let copied = super::load(temp.path(), &oid.to_string()).unwrap();
        assert_eq!(result.title, "Résumé");
        assert_eq!(result.description, "**Café**");
        assert_eq!(result.author, "André <test@example.invalid>");
        assert_eq!(result.markdown.0[0].spans[0].text, "Café");
        assert!(result.markdown.0[0].spans[0].style.bold);
        assert!(copied.contains("Résumé\n\n**Café**"));
        assert!(copied.contains("André <test@example.invalid>"));
        assert!(!copied.contains('\u{fffd}'));
        assert_eq!(std::fs::read(repo.path().join("index")).ok(), index_before);
    }
}
