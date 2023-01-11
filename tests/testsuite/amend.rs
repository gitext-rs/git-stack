use bstr::ByteSlice as _;

#[test]
fn amend_noop() {
    let root = snapbox::path::PathFixture::mutable_temp().unwrap();
    let root_path = root.path().unwrap();
    let plan = git_fixture::TodoList {
        init: true,
        sleep: None,
        author: None,
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
        .stdout_eq(
            "\
",
        )
        .stderr_matches(
            "\
error: nothing to amend to [..]: C
",
        );

    let new_head_id = repo.head_commit().id;
    assert_eq!(old_head_id, new_head_id);

    snapbox::assert_eq(std::fs::read(root_path.join("a")).unwrap(), "unstaged a");

    root.close().unwrap();
}

#[test]
fn reword_protected_fails() {
    let root = snapbox::path::PathFixture::mutable_temp().unwrap();
    let root_path = root.path().unwrap();
    let plan = git_fixture::TodoList {
        init: true,
        sleep: None,
        author: None,
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
        .stdout_eq(
            "\
",
        )
        .stderr_eq(
            "\
cannot amend protected commits
",
        );

    let new_head_id = repo.head_commit().id;
    assert_eq!(old_head_id, new_head_id);

    snapbox::assert_eq(std::fs::read(root_path.join("a")).unwrap(), "unstaged a");

    root.close().unwrap();
}

#[test]
fn reword() {
    let root = snapbox::path::PathFixture::mutable_temp().unwrap();
    let root_path = root.path().unwrap();
    let plan = git_fixture::TodoList {
        init: true,
        sleep: None,
        author: None,
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
        .stdout_eq(
            "\
",
        )
        .stderr_matches(
            "\
Saved working directory and index state WIP on target (amend): [..]
Amended to [..]: C
Dropped refs/stash [..]
note: to undo, run `git branch-stash pop git-stack`
",
        );

    let new_head_id = repo.head_commit().id;
    assert_ne!(old_head_id, new_head_id);

    snapbox::assert_eq(std::fs::read(root_path.join("a")).unwrap(), "unstaged a");

    root.close().unwrap();
}

#[test]
fn reword_rebases() {
    let root = snapbox::path::PathFixture::mutable_temp().unwrap();
    let root_path = root.path().unwrap();
    let plan = git_fixture::TodoList {
        init: true,
        sleep: None,
        author: None,
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
    };
    plan.run(root_path).unwrap();

    let repo = git2::Repository::discover(root_path).unwrap();
    let repo = git_stack::git::GitRepo::new(repo);

    let old_head_id = repo.head_commit().id;

    std::fs::write(root_path.join("a"), "unstaged a").unwrap();

    snapbox::cmd::Command::new(snapbox::cmd::cargo_bin!("git-stack"))
        .arg("amend")
        .arg("--message=new B")
        .current_dir(root_path)
        .assert()
        .success()
        .stdout_eq(
            "\
",
        )
        .stderr_matches(
            "\
Saved working directory and index state WIP on local (amend): [..]
Amended to [..]: C
Dropped refs/stash [..]
note: to undo, run `git branch-stash pop git-stack`
",
        );

    let new_head_id = repo.head_commit().id;
    assert_ne!(old_head_id, new_head_id);

    snapbox::assert_eq(std::fs::read(root_path.join("a")).unwrap(), "unstaged a");

    root.close().unwrap();
}

#[test]
fn amend_add() {
    let root = snapbox::path::PathFixture::mutable_temp().unwrap();
    let root_path = root.path().unwrap();
    let plan = git_fixture::TodoList {
        init: true,
        sleep: None,
        author: None,
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
        .stdout_eq(
            "\
",
        )
        .stderr_matches(
            "\
Adding c
Amended to [..]: C
note: to undo, run `git branch-stash pop git-stack`
",
        );

    let new_head_id = repo.head_commit().id;
    assert_ne!(old_head_id, new_head_id);

    root.close().unwrap();
}

#[test]
fn amend_staged() {
    let root = snapbox::path::PathFixture::mutable_temp().unwrap();
    let root_path = root.path().unwrap();
    let plan = git_fixture::TodoList {
        init: true,
        sleep: None,
        author: None,
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
        .stdout_eq(
            "\
",
        )
        .stderr_matches(
            "\
Saved working directory and index state WIP on target (amend): [..]
Amended to [..]: C
Dropped refs/stash [..]
note: to undo, run `git branch-stash pop git-stack`
",
        );

    let new_head_id = repo.head_commit().id;
    assert_ne!(old_head_id, new_head_id);

    snapbox::assert_eq(std::fs::read(root_path.join("a")).unwrap(), "unstaged a");

    root.close().unwrap();
}

#[test]
fn amend_detached() {
    let root = snapbox::path::PathFixture::mutable_temp().unwrap();
    let root_path = root.path().unwrap();
    let plan = git_fixture::TodoList {
        init: true,
        sleep: None,
        author: None,
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
        .stdout_eq(
            "\
",
        )
        .stderr_matches(
            "\
Saved working directory and index state WIP on HEAD (amend): [..]
Amended to [..]: B
Dropped refs/stash [..]
note: to undo, run `git branch-stash pop git-stack`
",
        );

    let new_head_id = repo.head_commit().id;
    assert_ne!(old_head_id, new_head_id);

    snapbox::assert_eq(std::fs::read(root_path.join("a")).unwrap(), "unstaged a");

    root.close().unwrap();
}

#[test]
fn amend_explicit_head() {
    let root = snapbox::path::PathFixture::mutable_temp().unwrap();
    let root_path = root.path().unwrap();
    let plan = git_fixture::TodoList {
        init: true,
        sleep: None,
        author: None,
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
        .stdout_eq(
            "\
",
        )
        .stderr_matches(
            "\
Saved working directory and index state WIP on target (amend): [..]
Amended to [..]: C
Dropped refs/stash [..]
note: to undo, run `git branch-stash pop git-stack`
",
        );

    let new_head_id = repo.head_commit().id;
    assert_ne!(old_head_id, new_head_id);

    snapbox::assert_eq(std::fs::read(root_path.join("a")).unwrap(), "unstaged a");

    root.close().unwrap();
}

#[test]
fn amend_ancestor() {
    let root = snapbox::path::PathFixture::mutable_temp().unwrap();
    let root_path = root.path().unwrap();
    let plan = git_fixture::TodoList {
        init: true,
        sleep: None,
        author: None,
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
        .stdout_eq(
            "\
",
        )
        .stderr_matches(
            "\
Saved working directory and index state WIP on local (amend): [..]
Amended to [..]: B
Dropped refs/stash [..]
note: to undo, run `git branch-stash pop git-stack`
",
        );

    let new_head_id = repo.head_commit().id;
    assert_ne!(old_head_id, new_head_id);

    snapbox::assert_eq(std::fs::read(root_path.join("a")).unwrap(), "unstaged a");

    root.close().unwrap();
}

#[test]
fn amend_conflict() {
    let root = snapbox::path::PathFixture::mutable_temp().unwrap();
    let root_path = root.path().unwrap();
    let plan = git_fixture::TodoList {
        init: true,
        sleep: None,
        author: None,
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
        .stdout_eq(
            "\
",
        )
        .stderr_matches(
            "\
Saved working directory and index state WIP on local (amend): [..]
ERROR: Failed to re-stack branch `local`: squash conflicts:
  c
; class=Index (10); code=Unmerged (-10)
Dropped refs/stash [..]
note: to undo, run `git branch-stash pop git-stack`
",
        );

    let new_head_id = repo.head_commit().id;
    assert_ne!(old_head_id, new_head_id);

    snapbox::assert_eq(std::fs::read(root_path.join("a")).unwrap(), "unstaged a");

    snapbox::assert_eq(repo.head_commit().summary.to_str().unwrap(), "fixup! B");

    root.close().unwrap();
}
