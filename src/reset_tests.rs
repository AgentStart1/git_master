use super::*;

#[test]
fn reset_blocks_uncommitted_changes_and_allows_clean_repositories() {
    let root = std::env::temp_dir().join(format!(
        "git-master-reset-test-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let remote = Repository::init(root.join("remote")).unwrap();
    let signature = git2::Signature::now("Test", "test@example.invalid").unwrap();
    let blob = remote.blob(b"original\n").unwrap();
    let mut builder = remote.treebuilder(None).unwrap();
    builder.insert("file.txt", blob, 0o100644).unwrap();
    let tree = remote.find_tree(builder.write().unwrap()).unwrap();
    let original = remote
        .commit(
            Some("refs/heads/main"),
            &signature,
            &signature,
            "initial",
            &tree,
            &[],
        )
        .unwrap();
    remote.set_head("refs/heads/main").unwrap();
    for mode in ["unstaged", "staged", "untracked", "deleted"] {
        let path = root.join(mode);
        let repo = Repository::clone(remote.path().to_str().unwrap(), &path).unwrap();
        match mode {
            "unstaged" | "staged" => {
                std::fs::write(path.join("file.txt"), "keep these edits\n").unwrap();
                if mode == "staged" {
                    let mut index = repo.index().unwrap();
                    index.add_path(Path::new("file.txt")).unwrap();
                    index.write().unwrap();
                }
            }
            "untracked" => {
                std::fs::create_dir(path.join("new-directory")).unwrap();
                std::fs::write(path.join("new-directory/new.txt"), "new code").unwrap();
            }
            "deleted" => std::fs::remove_file(path.join("file.txt")).unwrap(),
            _ => unreachable!(),
        }
        // A nonexistent remote demonstrates rejection happens before Fetch.
        let error = reset_to_remote_branch(&path, "does-not-exist", "main").unwrap_err();
        assert!(error.contains("uncommitted"), "{mode}: {error}");
        assert_eq!(repo.head().unwrap().target(), Some(original));
        if mode == "staged" || mode == "unstaged" {
            assert_eq!(
                std::fs::read_to_string(path.join("file.txt")).unwrap(),
                "keep these edits\n"
            );
        }
    }
    let clean_path = root.join("clean");
    let clean = Repository::clone(remote.path().to_str().unwrap(), &clean_path).unwrap();
    let parent = remote.find_commit(original).unwrap();
    let next = remote
        .commit(
            Some("refs/heads/main"),
            &signature,
            &signature,
            "next",
            &tree,
            &[&parent],
        )
        .unwrap();
    reset_to_remote_branch(&clean_path, "origin", "main").unwrap();
    assert_eq!(clean.head().unwrap().target(), Some(next));
    drop((clean, parent, tree, builder));
    drop(remote);
    std::fs::remove_dir_all(root).unwrap();
}
