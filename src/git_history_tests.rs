use super::*;
use std::collections::HashSet;

fn commit(repo: &Repository, parent: Option<Oid>, time: i64) -> Oid {
    let tree_id = repo.treebuilder(None).unwrap().write().unwrap();
    let tree = repo.find_tree(tree_id).unwrap();
    let signature = git2::Signature::new(
        "History Test",
        "history@example.invalid",
        &git2::Time::new(time, 0),
    )
    .unwrap();
    let parent = parent.map(|id| repo.find_commit(id).unwrap());
    repo.commit(
        None,
        &signature,
        &signature,
        &format!("commit {time}"),
        &tree,
        &parent.iter().collect::<Vec<_>>(),
    )
    .unwrap()
}

#[test]
fn each_tip_survives_and_expansion_reaches_history_past_200() {
    let temp = tempfile::tempdir().unwrap();
    let repo = Repository::init(temp.path()).unwrap();
    let base = commit(&repo, None, 1);
    let mut older = base;
    let mut newer = base;
    for index in 0..450 {
        older = commit(&repo, Some(older), 10 + index);
        newer = commit(&repo, Some(newer), 10000 + index);
    }
    repo.reference("refs/heads/older", older, true, "test")
        .unwrap();
    repo.reference("refs/heads/newer", newer, true, "test")
        .unwrap();
    let remote = commit(&repo, Some(older), 20000);
    repo.reference("refs/remotes/origin/older", remote, true, "test")
        .unwrap();
    let branches = vec!["older".into(), "newer".into()];
    let (first, more) = read_commit_history_page(&repo, &branches, 200);
    let ids = first
        .iter()
        .map(|entry| entry.full_hash.clone())
        .collect::<HashSet<_>>();
    assert!(more);
    for tip in [older, newer, remote] {
        assert!(ids.contains(&tip.to_string()));
    }
    assert_eq!(ids.len(), first.len());
    assert!(first.len() >= 400);
    let (second, more) = read_commit_history_page(&repo, &branches, 400);
    let expanded = second
        .iter()
        .map(|entry| entry.full_hash.clone())
        .collect::<HashSet<_>>();
    assert!(more);
    assert!(expanded.is_superset(&ids));
    assert!(expanded.len() > ids.len());
    let (all, more) = read_commit_history_page(&repo, &branches, 600);
    assert!(!more);
    assert_eq!(all.len(), 902);
    let order = all
        .iter()
        .enumerate()
        .map(|(index, entry)| (entry.full_hash.as_str(), index))
        .collect::<std::collections::HashMap<_, _>>();
    for entry in &all {
        for parent in &entry.parent_hashes {
            assert!(order[entry.full_hash.as_str()] < order[parent.as_str()]);
        }
    }
    assert!(read_commit_history_page(&repo, &branches, 0).0.is_empty());
    assert_eq!(repo.refname_to_id("refs/heads/older").unwrap(), older);
}

#[test]
fn detached_head_exact_page_and_empty_repository() {
    let temp = tempfile::tempdir().unwrap();
    let repo = Repository::init(temp.path()).unwrap();
    repo.set_head("refs/heads/main").unwrap();
    assert_eq!(get_repo_detail(temp.path()).unwrap().current_branch, "main");
    assert!(get_repo_detail(temp.path()).unwrap().head_oid.is_none());
    assert!(read_commit_history_page(&repo, &[], 200).0.is_empty());
    let base = commit(&repo, None, 10);
    let tip = commit(&repo, Some(base), 5); // Clock skew must not reverse ancestry.
    repo.set_head_detached(tip).unwrap();
    assert_eq!(
        get_repo_detail(temp.path()).unwrap().current_branch,
        "HEAD detached"
    );
    let (page, more) = read_commit_history_page(&repo, &[], 1);
    assert!(more);
    assert_eq!(page[0].full_hash, tip.to_string());
    let (page, more) = read_commit_history_page(&repo, &[], 2);
    assert!(!more);
    assert_eq!(page.len(), 2);
    assert_eq!(page[0].full_hash, tip.to_string());
    assert_eq!(page[1].full_hash, base.to_string());
}

#[test]
fn branch_labels_include_symbolic_remote_head_at_its_resolved_commit() {
    let temp = tempfile::tempdir().unwrap();
    let repo = Repository::init(temp.path()).unwrap();
    let oid = commit(&repo, None, 1);
    repo.reference("refs/heads/main", oid, true, "fixture")
        .unwrap();
    repo.set_head("refs/heads/main").unwrap();
    repo.remote("origin", "https://example.invalid/repo.git")
        .unwrap();
    repo.reference("refs/remotes/origin/main", oid, true, "fixture")
        .unwrap();
    repo.reference_symbolic(
        "refs/remotes/origin/HEAD",
        "refs/remotes/origin/main",
        true,
        "fixture",
    )
    .unwrap();
    let labels = collect_branch_heads(&repo);
    let labels = &labels[&oid.to_string()];
    for expected in ["HEAD", "main", "origin/main", "origin/HEAD"] {
        assert!(labels.iter().any(|s| s == expected), "missing {expected}");
    }
}
