use bstr::ByteSlice;

#[test]
fn reword_protected_fails() {
    let root = snapbox::path::PathFixture::mutable_temp().unwrap();
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
        .stdout_eq(
            "\
",
        )
        .stderr_eq(
            "\
cannot reword protected commits
",
        );

    let new_head_id = repo.head_commit().id;
    assert_eq!(old_head_id, new_head_id);
}

#[test]
fn reword_implicit_head() {
    let root = snapbox::path::PathFixture::mutable_temp().unwrap();
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
    snapbox::assert_eq(commit.summary.to_str().unwrap(), "C");

    let old_head_id = repo.head_commit().id;

    snapbox::cmd::Command::new(snapbox::cmd::cargo_bin!("git-stack"))
        .arg("reword")
        .arg("--message=new C")
        .current_dir(root_path)
        .assert()
        .success()
        .stdout_eq(
            "\
",
        )
        .stderr_eq(
            "\
note: to undo, run `git branch-stash pop git-stack`
",
        );

    let branch = repo.find_local_branch("target").unwrap();
    let commit = repo.find_commit(branch.id).unwrap();
    snapbox::assert_eq(commit.summary.to_str().unwrap(), "new C");

    let new_head_id = repo.head_commit().id;
    assert_ne!(old_head_id, new_head_id);

    root.close().unwrap();
}

#[test]
fn reword_explicit_head() {
    let root = snapbox::path::PathFixture::mutable_temp().unwrap();
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
    snapbox::assert_eq(commit.summary.to_str().unwrap(), "C");

    let old_head_id = repo.head_commit().id;

    snapbox::cmd::Command::new(snapbox::cmd::cargo_bin!("git-stack"))
        .arg("reword")
        .arg("--message=new C")
        .arg("HEAD")
        .current_dir(root_path)
        .assert()
        .success()
        .stdout_eq(
            "\
",
        )
        .stderr_eq(
            "\
note: to undo, run `git branch-stash pop git-stack`
",
        );

    let branch = repo.find_local_branch("target").unwrap();
    let commit = repo.find_commit(branch.id).unwrap();
    snapbox::assert_eq(commit.summary.to_str().unwrap(), "new C");

    let new_head_id = repo.head_commit().id;
    assert_ne!(old_head_id, new_head_id);

    root.close().unwrap();
}

#[test]
fn reword_branch() {
    let root = snapbox::path::PathFixture::mutable_temp().unwrap();
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
    snapbox::assert_eq(commit.summary.to_str().unwrap(), "B");

    let old_head_id = repo.head_commit().id;

    snapbox::cmd::Command::new(snapbox::cmd::cargo_bin!("git-stack"))
        .arg("reword")
        .arg("--message=new B")
        .arg("target")
        .current_dir(root_path)
        .assert()
        .success()
        .stdout_eq(
            "\
",
        )
        .stderr_eq(
            "\
note: to undo, run `git branch-stash pop git-stack`
",
        );

    let branch = repo.find_local_branch("target").unwrap();
    let commit = repo.find_commit(branch.id).unwrap();
    snapbox::assert_eq(commit.summary.to_str().unwrap(), "new B");

    let local_branch = repo.find_local_branch("local").unwrap();
    let local_commit = repo.find_commit(local_branch.id).unwrap();
    snapbox::assert_eq(local_commit.summary.to_str_lossy().into_owned(), "C");

    let new_head_id = repo.head_commit().id;
    assert_ne!(old_head_id, new_head_id);

    root.close().unwrap();
}
