use std::path::{Path, PathBuf};
use std::process::Command;

use super::{delete_branch, pull_remote_rebase, push_remote};

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "git-master-actions-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn git(path: &Path, args: &[&str]) {
    let output = Command::new("git")
        .args(args)
        .current_dir(path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn configure_identity(path: &Path) {
    git(path, &["config", "user.name", "Git Master Tests"]);
    git(
        path,
        &["config", "user.email", "git-master@example.invalid"],
    );
}

#[test]
fn remote_pull_autostashes_and_push_targets_the_selected_remote() {
    let temp = TestDirectory::new();
    let remote = temp.0.join("remote.git");
    let seed = temp.0.join("seed");
    let local = temp.0.join("local");

    git(
        &temp.0,
        &["init", "--bare", "--initial-branch=main", "remote.git"],
    );
    git(&temp.0, &["init", "--initial-branch=main", "seed"]);
    configure_identity(&seed);
    std::fs::write(seed.join("tracked.txt"), "base\n").unwrap();
    git(&seed, &["add", "tracked.txt"]);
    git(&seed, &["commit", "-m", "base"]);
    git(
        &seed,
        &["remote", "add", "origin", remote.to_str().unwrap()],
    );
    git(&seed, &["push", "origin", "main"]);
    git(
        &temp.0,
        &["clone", remote.to_str().unwrap(), local.to_str().unwrap()],
    );
    configure_identity(&local);

    std::fs::write(seed.join("remote.txt"), "remote change\n").unwrap();
    git(&seed, &["add", "remote.txt"]);
    git(&seed, &["commit", "-m", "remote change"]);
    git(&seed, &["push", "origin", "main"]);
    std::fs::write(local.join("tracked.txt"), "local edit\n").unwrap();

    pull_remote_rebase(&local, "origin", "main").unwrap();

    assert_eq!(
        std::fs::read_to_string(local.join("tracked.txt"))
            .unwrap()
            .replace("\r\n", "\n"),
        "local edit\n"
    );
    assert!(local.join("remote.txt").exists());

    git(&local, &["add", "tracked.txt"]);
    git(&local, &["commit", "-m", "local change"]);
    push_remote(&local, "origin", "main").unwrap();
    let local_head = std::fs::read_to_string(remote.join("refs/heads/main")).unwrap();
    let expected = String::from_utf8(
        Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(&local)
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap();
    assert_eq!(local_head.trim(), expected.trim());
}

#[test]
fn deleting_a_branch_is_safe_and_cannot_delete_the_current_branch() {
    let temp = TestDirectory::new();
    git(&temp.0, &["init", "--initial-branch=main"]);
    configure_identity(&temp.0);
    std::fs::write(temp.0.join("tracked.txt"), "base\n").unwrap();
    git(&temp.0, &["add", "tracked.txt"]);
    git(&temp.0, &["commit", "-m", "base"]);
    git(&temp.0, &["branch", "finished"]);

    delete_branch(&temp.0, "finished").unwrap();
    assert!(delete_branch(&temp.0, "main").is_err());
}
