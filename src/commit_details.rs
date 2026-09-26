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
        drop((parent, tree, builder));
        drop(repo);
        std::fs::remove_dir_all(path).unwrap();
    }
}
