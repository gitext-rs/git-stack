use bstr::ByteSlice;
use snapbox::assert_data_eq;
use snapbox::prelude::*;
use snapbox::str;

#[test]
fn reword_protected_fails() {
    let root = snapbox::dir::DirRoot::mutable_temp().unwrap();
    let root_path = root.path().unwrap();
    let plan = git_fixture::TodoList {
        commands: vec![
            git_fixture::Command::Tree(git_fixture::Tree {
                files: [("a", "a")]
                    .into_iter()
                    .map(|(p, c)| (p.into(), c.into()))
                    .collect::<std::collections::HashMap<_, _>>(),
                message: Some("A".to_owned()),
                author: None,
            }),
            git_fixture::Command::Branch("main".into()),
        ],
        ..Default::default()
    };
    plan.run(root_path).unwrap();

    let repo = git2::Repository::discover(root_path).unwrap();
    let repo = git_stack::git::GitRepo::new(repo);

    let old_head_id = repo.head_commit().id;

    snapbox::cmd::Command::new(snapbox::cmd::cargo_bin!("git-stack"))
        .arg("reword")
        .arg("--message=hahahaha")
        .current_dir(root_path)
        .assert()
        .failure()
        .stdout_eq_(str![].raw())
        .stderr_eq_(
            str![[r#"
cannot reword protected commits

"#]]
            .raw(),
        );

    let new_head_id = repo.head_commit().id;
    assert_eq!(old_head_id, new_head_id);
}

#[test]
fn reword_implicit_head() {
    let root = snapbox::dir::DirRoot::mutable_temp().unwrap();
    let root_path = root.path().unwrap();
    let plan = git_fixture::TodoList {
        commands: vec![
            git_fixture::Command::Tree(git_fixture::Tree {
                files: [("a", "a")]
                    .into_iter()
                    .map(|(p, c)| (p.into(), c.into()))
                    .collect::<std::collections::HashMap<_, _>>(),
                message: Some("A".to_owned()),
                author: None,
            }),
            git_fixture::Command::Branch("main".into()),
            git_fixture::Command::Tree(git_fixture::Tree {
                files: [("a", "a"), ("b", "b")]
                    .into_iter()
                    .map(|(p, c)| (p.into(), c.into()))
                    .collect::<std::collections::HashMap<_, _>>(),
                message: Some("B".to_owned()),
                author: None,
            }),
            git_fixture::Command::Tree(git_fixture::Tree {
                files: [("a", "a"), ("b", "b"), ("c", "c")]
                    .into_iter()
                    .map(|(p, c)| (p.into(), c.into()))
                    .collect::<std::collections::HashMap<_, _>>(),
                message: Some("C".to_owned()),
                author: None,
            }),
            git_fixture::Command::Branch("target".into()),
        ],
        ..Default::default()
    };
    plan.run(root_path).unwrap();

    let repo = git2::Repository::discover(root_path).unwrap();
    let repo = git_stack::git::GitRepo::new(repo);

    let branch = repo.find_local_branch("target").unwrap();
    let commit = repo.find_commit(branch.id).unwrap();
    assert_data_eq!(commit.summary.to_str().unwrap(), str!["C"].raw());

    let old_head_id = repo.head_commit().id;

    snapbox::cmd::Command::new(snapbox::cmd::cargo_bin!("git-stack"))
        .arg("reword")
        .arg("--message=new C")
        .current_dir(root_path)
        .assert()
        .success()
        .stdout_eq_(str![].raw())
        .stderr_eq_(
            str![[r#"
note: to undo, run `git branch-stash pop git-stack`

"#]]
            .raw(),
        );

    let branch = repo.find_local_branch("target").unwrap();
    let commit = repo.find_commit(branch.id).unwrap();
    assert_data_eq!(commit.summary.to_str().unwrap(), str!["new C"].raw());

    let new_head_id = repo.head_commit().id;
    assert_ne!(old_head_id, new_head_id);

    root.close().unwrap();
}

#[test]
fn reword_explicit_head() {
    let root = snapbox::dir::DirRoot::mutable_temp().unwrap();
    let root_path = root.path().unwrap();
    let plan = git_fixture::TodoList {
        commands: vec![
            git_fixture::Command::Tree(git_fixture::Tree {
                files: [("a", "a")]
                    .into_iter()
                    .map(|(p, c)| (p.into(), c.into()))
                    .collect::<std::collections::HashMap<_, _>>(),
                message: Some("A".to_owned()),
                author: None,
            }),
            git_fixture::Command::Branch("main".into()),
            git_fixture::Command::Tree(git_fixture::Tree {
                files: [("a", "a"), ("b", "b")]
                    .into_iter()
                    .map(|(p, c)| (p.into(), c.into()))
                    .collect::<std::collections::HashMap<_, _>>(),
                message: Some("B".to_owned()),
                author: None,
            }),
            git_fixture::Command::Tree(git_fixture::Tree {
                files: [("a", "a"), ("b", "b"), ("c", "c")]
                    .into_iter()
                    .map(|(p, c)| (p.into(), c.into()))
                    .collect::<std::collections::HashMap<_, _>>(),
                message: Some("C".to_owned()),
                author: None,
            }),
            git_fixture::Command::Branch("target".into()),
        ],
        ..Default::default()
    };
    plan.run(root_path).unwrap();

    let repo = git2::Repository::discover(root_path).unwrap();
    let repo = git_stack::git::GitRepo::new(repo);

    let branch = repo.find_local_branch("target").unwrap();
    let commit = repo.find_commit(branch.id).unwrap();
    assert_data_eq!(commit.summary.to_str().unwrap(), str!["C"].raw());

    let old_head_id = repo.head_commit().id;

    snapbox::cmd::Command::new(snapbox::cmd::cargo_bin!("git-stack"))
        .arg("reword")
        .arg("--message=new C")
        .arg("HEAD")
        .current_dir(root_path)
        .assert()
        .success()
        .stdout_eq_(str![].raw())
        .stderr_eq_(
            str![[r#"
note: to undo, run `git branch-stash pop git-stack`

"#]]
            .raw(),
        );

    let branch = repo.find_local_branch("target").unwrap();
    let commit = repo.find_commit(branch.id).unwrap();
    assert_data_eq!(commit.summary.to_str().unwrap(), str!["new C"].raw());

    let new_head_id = repo.head_commit().id;
    assert_ne!(old_head_id, new_head_id);

    root.close().unwrap();
}

#[test]
fn reword_branch() {
    let root = snapbox::dir::DirRoot::mutable_temp().unwrap();
    let root_path = root.path().unwrap();
    let plan = git_fixture::TodoList {
        commands: vec![
            git_fixture::Command::Tree(git_fixture::Tree {
                files: [("a", "a")]
                    .into_iter()
                    .map(|(p, c)| (p.into(), c.into()))
                    .collect::<std::collections::HashMap<_, _>>(),
                message: Some("A".to_owned()),
                author: None,
            }),
            git_fixture::Command::Branch("main".into()),
            git_fixture::Command::Tree(git_fixture::Tree {
                files: [("a", "a"), ("b", "b")]
                    .into_iter()
                    .map(|(p, c)| (p.into(), c.into()))
                    .collect::<std::collections::HashMap<_, _>>(),
                message: Some("B".to_owned()),
                author: None,
            }),
            git_fixture::Command::Branch("target".into()),
            git_fixture::Command::Tree(git_fixture::Tree {
                files: [("a", "a"), ("b", "b"), ("c", "c")]
                    .into_iter()
                    .map(|(p, c)| (p.into(), c.into()))
                    .collect::<std::collections::HashMap<_, _>>(),
                message: Some("C".to_owned()),
                author: None,
            }),
            git_fixture::Command::Branch("local".into()),
        ],
        ..Default::default()
    };
    plan.run(root_path).unwrap();

    let repo = git2::Repository::discover(root_path).unwrap();
    let repo = git_stack::git::GitRepo::new(repo);

    let branch = repo.find_local_branch("target").unwrap();
    let commit = repo.find_commit(branch.id).unwrap();
    assert_data_eq!(commit.summary.to_str().unwrap(), str!["B"].raw());

    let old_head_id = repo.head_commit().id;

    std::fs::write(root_path.join("a"), "unstaged a").unwrap();

    snapbox::cmd::Command::new(snapbox::cmd::cargo_bin!("git-stack"))
        .arg("reword")
        .arg("--message=new B")
        .arg("target")
        .current_dir(root_path)
        .assert()
        .success()
        .stdout_eq_(str![].raw())
        .stderr_eq_(str![[r#"
Saved working directory and index state WIP on local (reword): [..]
Dropped refs/stash [..]
note: to undo, run `git branch-stash pop git-stack`

"#]]);

    let branch = repo.find_local_branch("target").unwrap();
    let commit = repo.find_commit(branch.id).unwrap();
    assert_data_eq!(commit.summary.to_str().unwrap(), str!["new B"].raw());

    let local_branch = repo.find_local_branch("local").unwrap();
    let local_commit = repo.find_commit(local_branch.id).unwrap();
    assert_data_eq!(
        local_commit.summary.to_str_lossy().into_owned(),
        str!["C"].raw()
    );

    let new_head_id = repo.head_commit().id;
    assert_ne!(old_head_id, new_head_id);

    assert_data_eq!(
        std::fs::read(root_path.join("a")).unwrap(),
        str!["unstaged a"].raw()
    );

    root.close().unwrap();
}
