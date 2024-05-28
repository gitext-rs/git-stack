use bstr::ByteSlice as _;
use snapbox::assert_data_eq;
use snapbox::prelude::*;
use snapbox::str;

#[test]
fn amend_noop() {
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

    std::fs::write(root_path.join("a"), "unstaged a").unwrap();

    let repo = git2::Repository::discover(root_path).unwrap();
    let repo = git_stack::git::GitRepo::new(repo);

    let old_head_id = repo.head_commit().id;

    snapbox::cmd::Command::new(snapbox::cmd::cargo_bin!("git-stack"))
        .arg("amend")
        .current_dir(root_path)
        .assert()
        .failure()
        .stdout_eq_(str![].raw())
        .stderr_eq_(str![[r#"
            error: nothing to amend to [..]: C
        "#]]);

    let new_head_id = repo.head_commit().id;
    assert_eq!(old_head_id, new_head_id);

    assert_data_eq!(
        std::fs::read(root_path.join("a")).unwrap(),
        str!["unstaged a"].raw()
    );

    root.close().unwrap();
}

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

    std::fs::write(root_path.join("a"), "unstaged a").unwrap();

    snapbox::cmd::Command::new(snapbox::cmd::cargo_bin!("git-stack"))
        .arg("amend")
        .arg("--message=hahahaha")
        .current_dir(root_path)
        .assert()
        .failure()
        .stdout_eq_(str![].raw())
        .stderr_eq_(
            str![[r#"
            cannot amend protected commits
        "#]]
            .raw(),
        );

    let new_head_id = repo.head_commit().id;
    assert_eq!(old_head_id, new_head_id);

    assert_data_eq!(
        std::fs::read(root_path.join("a")).unwrap(),
        str!["unstaged a"].raw()
    );

    root.close().unwrap();
}

#[test]
fn reword() {
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

    let old_head_id = repo.head_commit().id;

    std::fs::write(root_path.join("a"), "unstaged a").unwrap();

    snapbox::cmd::Command::new(snapbox::cmd::cargo_bin!("git-stack"))
        .arg("amend")
        .arg("--message=new C")
        .current_dir(root_path)
        .assert()
        .success()
        .stdout_eq_(str![].raw())
        .stderr_eq_(str![[r#"
            Saved working directory and index state WIP on target (amend): [..]
            Amended to [..]: C
            Dropped refs/stash [..]
            note: to undo, run `git branch-stash pop git-stack`
        "#]]);

    let new_head_id = repo.head_commit().id;
    assert_ne!(old_head_id, new_head_id);

    assert_data_eq!(
        std::fs::read(root_path.join("a")).unwrap(),
        str!["unstaged a"].raw()
    );

    root.close().unwrap();
}

#[test]
fn reword_rebases() {
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

    let old_head_id = repo.head_commit().id;

    std::fs::write(root_path.join("a"), "unstaged a").unwrap();

    snapbox::cmd::Command::new(snapbox::cmd::cargo_bin!("git-stack"))
        .arg("amend")
        .arg("--message=new B")
        .arg("target")
        .current_dir(root_path)
        .assert()
        .success()
        .stdout_eq_(str![].raw())
        .stderr_eq_(str![[r#"
            Saved working directory and index state WIP on local (amend): [..]
            Amended to [..]: B
            Dropped refs/stash [..]
            note: to undo, run `git branch-stash pop git-stack`
        "#]]);

    let new_head_id = repo.head_commit().id;
    assert_ne!(old_head_id, new_head_id);

    let local_branch = repo.find_local_branch("local").unwrap();
    let local_commit = repo.find_commit(local_branch.id).unwrap();
    assert_data_eq!(
        local_commit.summary.to_str_lossy().into_owned(),
        str!["C"].raw()
    );

    assert_data_eq!(
        std::fs::read(root_path.join("a")).unwrap(),
        str!["unstaged a"].raw()
    );

    root.close().unwrap();
}

#[test]
fn amend_add() {
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

    let old_head_id = repo.head_commit().id;

    std::fs::write(root_path.join("c"), "new c").unwrap();

    snapbox::cmd::Command::new(snapbox::cmd::cargo_bin!("git-stack"))
        .arg("amend")
        .arg("-a")
        .current_dir(root_path)
        .assert()
        .success()
        .stdout_eq_(str![].raw())
        .stderr_eq_(str![[r#"
            Adding c
            Amended to [..]: C
            note: to undo, run `git branch-stash pop git-stack`
        "#]]);

    let new_head_id = repo.head_commit().id;
    assert_ne!(old_head_id, new_head_id);

    root.close().unwrap();
}

#[test]
fn amend_staged() {
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

    let old_head_id = repo.head_commit().id;

    std::fs::write(root_path.join("a"), "unstaged a").unwrap();

    std::fs::write(root_path.join("c"), "new c").unwrap();
    snapbox::cmd::Command::new("git")
        .arg("add")
        .arg("c")
        .current_dir(root_path)
        .assert()
        .success();
    snapbox::cmd::Command::new(snapbox::cmd::cargo_bin!("git-stack"))
        .arg("amend")
        .current_dir(root_path)
        .assert()
        .success()
        .stdout_eq_(str![].raw())
        .stderr_eq_(str![[r#"
            Saved working directory and index state WIP on target (amend): [..]
            Amended to [..]: C
            Dropped refs/stash [..]
            note: to undo, run `git branch-stash pop git-stack`
        "#]]);

    let new_head_id = repo.head_commit().id;
    assert_ne!(old_head_id, new_head_id);

    assert_data_eq!(
        std::fs::read(root_path.join("a")).unwrap(),
        str!["unstaged a"].raw()
    );

    root.close().unwrap();
}

#[test]
fn amend_detached() {
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
            git_fixture::Command::Head,
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

    let old_head_id = repo.head_commit().id;

    std::fs::write(root_path.join("a"), "unstaged a").unwrap();

    std::fs::write(root_path.join("b"), "new b").unwrap();
    snapbox::cmd::Command::new("git")
        .arg("add")
        .arg("b")
        .current_dir(root_path)
        .assert()
        .success();
    snapbox::cmd::Command::new(snapbox::cmd::cargo_bin!("git-stack"))
        .arg("amend")
        .current_dir(root_path)
        .assert()
        .success()
        .stdout_eq_(str![].raw())
        .stderr_eq_(str![[r#"
            Saved working directory and index state WIP on HEAD (amend): [..]
            Amended to [..]: B
            Dropped refs/stash [..]
            note: to undo, run `git branch-stash pop git-stack`
        "#]]);

    let new_head_id = repo.head_commit().id;
    assert_ne!(old_head_id, new_head_id);

    let local_branch = repo.find_local_branch("local").unwrap();
    let local_commit = repo.find_commit(local_branch.id).unwrap();
    assert_data_eq!(
        local_commit.summary.to_str_lossy().into_owned(),
        str!["C"].raw()
    );

    assert_data_eq!(
        std::fs::read(root_path.join("a")).unwrap(),
        str!["unstaged a"].raw()
    );

    root.close().unwrap();
}

#[test]
fn amend_explicit_head() {
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

    let old_head_id = repo.head_commit().id;

    std::fs::write(root_path.join("a"), "unstaged a").unwrap();

    std::fs::write(root_path.join("c"), "new c").unwrap();
    snapbox::cmd::Command::new("git")
        .arg("add")
        .arg("c")
        .current_dir(root_path)
        .assert()
        .success();
    snapbox::cmd::Command::new(snapbox::cmd::cargo_bin!("git-stack"))
        .arg("amend")
        .arg("HEAD")
        .current_dir(root_path)
        .assert()
        .success()
        .stdout_eq_(str![].raw())
        .stderr_eq_(str![[r#"
            Saved working directory and index state WIP on target (amend): [..]
            Amended to [..]: C
            Dropped refs/stash [..]
            note: to undo, run `git branch-stash pop git-stack`
        "#]]);

    let new_head_id = repo.head_commit().id;
    assert_ne!(old_head_id, new_head_id);

    assert_data_eq!(
        std::fs::read(root_path.join("a")).unwrap(),
        str!["unstaged a"]
    );

    root.close().unwrap();
}

#[test]
fn amend_ancestor() {
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

    let old_head_id = repo.head_commit().id;

    std::fs::write(root_path.join("a"), "unstaged a").unwrap();

    std::fs::write(root_path.join("b"), "new b").unwrap();
    snapbox::cmd::Command::new("git")
        .arg("add")
        .arg("b")
        .current_dir(root_path)
        .assert()
        .success();
    snapbox::cmd::Command::new(snapbox::cmd::cargo_bin!("git-stack"))
        .arg("amend")
        .arg("target")
        .current_dir(root_path)
        .assert()
        .success()
        .stdout_eq_(str![].raw())
        .stderr_eq_(str![[r#"
            Saved working directory and index state WIP on local (amend): [..]
            Amended to [..]: B
            Dropped refs/stash [..]
            note: to undo, run `git branch-stash pop git-stack`
        "#]]);

    let new_head_id = repo.head_commit().id;
    assert_ne!(old_head_id, new_head_id);

    let local_branch = repo.find_local_branch("local").unwrap();
    let local_commit = repo.find_commit(local_branch.id).unwrap();
    assert_data_eq!(
        local_commit.summary.to_str_lossy().into_owned(),
        str!["C"].raw()
    );

    assert_data_eq!(
        std::fs::read(root_path.join("a")).unwrap(),
        str!["unstaged a"].raw()
    );

    root.close().unwrap();
}

#[test]
fn amend_conflict() {
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

    let old_head_id = repo.head_commit().id;

    std::fs::write(root_path.join("a"), "unstaged a").unwrap();

    std::fs::write(root_path.join("c"), "conflicted c").unwrap();
    snapbox::cmd::Command::new("git")
        .arg("add")
        .arg("c")
        .current_dir(root_path)
        .assert()
        .success();
    snapbox::cmd::Command::new(snapbox::cmd::cargo_bin!("git-stack"))
        .arg("amend")
        .arg("target")
        .current_dir(root_path)
        .assert()
        .failure()
        .stdout_eq_(str![].raw())
        .stderr_eq_(str![[r#"
            Saved working directory and index state WIP on local (amend): [..]
            ERROR: Failed to re-stack branch `local`: squash conflicts:
              c
            ; class=Index (10); code=Unmerged (-10)
            Dropped refs/stash [..]
            note: to undo, run `git branch-stash pop git-stack`
        "#]]);

    let new_head_id = repo.head_commit().id;
    assert_ne!(old_head_id, new_head_id);

    assert_data_eq!(
        repo.head_commit().summary.to_str().unwrap(),
        str!["fixup! B"].raw()
    );

    let local_branch = repo.find_local_branch("local").unwrap();
    let local_commit = repo.find_commit(local_branch.id).unwrap();
    assert_data_eq!(
        local_commit.summary.to_str_lossy().into_owned(),
        str!["fixup! B"].raw()
    );

    assert_data_eq!(
        std::fs::read(root_path.join("a")).unwrap(),
        str!["unstaged a"].raw()
    );

    root.close().unwrap();
}
