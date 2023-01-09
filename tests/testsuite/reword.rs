use bstr::ByteSlice;

#[test]
fn reword_head() {
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
            git_fixture::Command::Branch("local".into()),
        ],
    };
    plan.run(root_path).unwrap();

    let repo = git2::Repository::discover(root_path).unwrap();
    let repo = git_stack::git::GitRepo::new(repo);

    let branch = repo.find_local_branch("local").unwrap();
    let commit = repo.find_commit(branch.id).unwrap();
    snapbox::assert_eq(commit.summary.to_str().unwrap(), "C");

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

    let branch = repo.find_local_branch("local").unwrap();
    let commit = repo.find_commit(branch.id).unwrap();
    snapbox::assert_eq(commit.summary.to_str().unwrap(), "new C");

    root.close().unwrap();
}
