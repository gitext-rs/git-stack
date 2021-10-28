mod fixture;

use bstr::ByteSlice;

use git_stack::graph::*;

mod test_rebase {
    use super::*;

    #[test]
    fn no_op() {
        let mut repo = git_stack::git::InMemoryRepo::new();
        let plan =
            git_fixture::Dag::load(std::path::Path::new("tests/fixtures/branches.yml")).unwrap();
        fixture::populate_repo(&mut repo, plan);

        let master_branch = repo.find_local_branch("master").unwrap();

        let mut protected_branches = git_stack::git::Branches::default();
        protected_branches.insert(master_branch.clone());

        let mut graphed_branches = git_stack::git::Branches::default();
        graphed_branches.insert(master_branch.clone());
        graphed_branches.insert(repo.find_local_branch("off_master").unwrap());

        let master_commit = repo.find_commit(master_branch.id).unwrap();

        let mut graph = Graph::from_branches(&repo, graphed_branches).unwrap();
        git_stack::graph::protect_branches(&mut graph, &repo, &protected_branches);
        git_stack::graph::rebase_branches(&mut graph, master_commit.id);
        let script = git_stack::graph::to_script(&graph);
        dbg!(&script);

        let mut executor = git_stack::git::Executor::new(&repo, false);
        let result = executor.run_script(&mut repo, &script);
        assert_eq!(result, vec![]);
        executor.close(&mut repo, "off_master").unwrap();
        dbg!(&repo);

        let master_branch = repo.find_local_branch("master").unwrap();
        dbg!(&master_branch.id);
        assert_eq!(master_branch.id, master_commit.id);

        let off_master_branch = repo.find_local_branch("off_master").unwrap();
        let ancestors: Vec<_> = repo
            .commits_from(off_master_branch.id)
            .map(|c| c.id)
            .collect();
        dbg!(&ancestors);
        assert!(ancestors.contains(&master_branch.id));
    }

    #[test]
    fn rebase() {
        let mut repo = git_stack::git::InMemoryRepo::new();
        let plan =
            git_fixture::Dag::load(std::path::Path::new("tests/fixtures/branches.yml")).unwrap();
        fixture::populate_repo(&mut repo, plan);

        let master_branch = repo.find_local_branch("master").unwrap();

        let mut protected_branches = git_stack::git::Branches::default();
        protected_branches.insert(master_branch.clone());

        let mut graphed_branches = git_stack::git::Branches::default();
        graphed_branches.insert(master_branch.clone());
        graphed_branches.insert(repo.find_local_branch("feature1").unwrap());
        graphed_branches.insert(repo.find_local_branch("feature2").unwrap());

        let master_commit = repo.find_commit(master_branch.id).unwrap();

        let mut graph = Graph::from_branches(&repo, graphed_branches).unwrap();
        git_stack::graph::protect_branches(&mut graph, &repo, &protected_branches);
        git_stack::graph::rebase_branches(&mut graph, master_commit.id);
        let script = git_stack::graph::to_script(&graph);
        dbg!(&script);

        let mut executor = git_stack::git::Executor::new(&repo, false);
        let result = executor.run_script(&mut repo, &script);
        assert_eq!(result, vec![]);
        executor.close(&mut repo, "off_master").unwrap();
        dbg!(&repo);

        let master_branch = repo.find_local_branch("master").unwrap();
        dbg!(&master_branch.id);
        assert_eq!(master_branch.id, master_commit.id);

        let feature2_branch = repo.find_local_branch("feature2").unwrap();
        let ancestors: Vec<_> = repo
            .commits_from(feature2_branch.id)
            .map(|c| c.id)
            .collect();
        dbg!(&ancestors);
        assert!(ancestors.contains(&master_branch.id));

        let feature1_branch = repo.find_local_branch("feature1").unwrap();
        dbg!(&feature1_branch.id);
        assert!(ancestors.contains(&feature1_branch.id));
    }
}

mod test_fixup {
    use super::*;

    #[test]
    fn no_op() {
        let mut repo = git_stack::git::InMemoryRepo::new();
        let plan =
            git_fixture::Dag::load(std::path::Path::new("tests/fixtures/branches.yml")).unwrap();
        fixture::populate_repo(&mut repo, plan);

        let master_branch = repo.find_local_branch("master").unwrap();

        let mut protected_branches = git_stack::git::Branches::default();
        protected_branches.insert(master_branch.clone());

        let mut graphed_branches = git_stack::git::Branches::default();
        graphed_branches.insert(master_branch.clone());
        graphed_branches.insert(repo.find_local_branch("off_master").unwrap());

        let master_commit = repo.find_commit(master_branch.id).unwrap();

        let mut graph = Graph::from_branches(&repo, graphed_branches).unwrap();
        git_stack::graph::protect_branches(&mut graph, &repo, &protected_branches);
        git_stack::graph::fixup(&mut graph, git_stack::config::Fixup::Move);
        let script = git_stack::graph::to_script(&graph);
        dbg!(&script);

        let mut executor = git_stack::git::Executor::new(&repo, false);
        let result = executor.run_script(&mut repo, &script);
        assert_eq!(result, vec![]);
        executor.close(&mut repo, "off_master").unwrap();
        dbg!(&repo);

        let master_branch = repo.find_local_branch("master").unwrap();
        dbg!(&master_branch.id);
        assert_eq!(master_branch.id, master_commit.id);

        let off_master_branch = repo.find_local_branch("off_master").unwrap();
        let ancestors: Vec<_> = repo
            .commits_from(off_master_branch.id)
            .map(|c| c.id)
            .collect();
        dbg!(&ancestors);
        assert!(ancestors.contains(&master_branch.id));
    }

    #[test]
    fn fixup_move_after_target() {
        let mut repo = git_stack::git::InMemoryRepo::new();
        let plan =
            git_fixture::Dag::load(std::path::Path::new("tests/fixtures/fixup.yml")).unwrap();
        fixture::populate_repo(&mut repo, plan);

        let master_branch = repo.find_local_branch("master").unwrap();

        let mut protected_branches = git_stack::git::Branches::default();
        protected_branches.insert(master_branch.clone());

        let mut graphed_branches = git_stack::git::Branches::default();
        graphed_branches.insert(master_branch.clone());
        graphed_branches.insert(repo.find_local_branch("feature1").unwrap());
        graphed_branches.insert(repo.find_local_branch("feature2").unwrap());

        let master_commit = repo.find_commit(master_branch.id).unwrap();

        let mut graph = Graph::from_branches(&repo, graphed_branches).unwrap();
        git_stack::graph::protect_branches(&mut graph, &repo, &protected_branches);
        git_stack::graph::fixup(&mut graph, git_stack::config::Fixup::Move);
        let script = git_stack::graph::to_script(&graph);
        dbg!(&script);

        let mut executor = git_stack::git::Executor::new(&repo, false);
        let result = executor.run_script(&mut repo, &script);
        assert_eq!(result, vec![]);
        executor.close(&mut repo, "master").unwrap();
        dbg!(&repo);

        let feature2_branch = repo.find_local_branch("feature2").unwrap();
        let mut commits: Vec<_> = repo
            .commits_from(feature2_branch.id)
            .map(|c| c.summary.to_str_lossy().into_owned())
            .collect();
        commits.reverse();
        assert_eq!(
            commits,
            &[
                "commit 1",
                "commit 2",
                "master commit",
                "feature1 commit 1",
                "fixup! feature1 commit 1",
                "fixup! feature1 commit 1",
                "fixup! feature1 commit 1",
                "feature1 commit 2",
                "fixup! feature1 commit 2",
                "feature1 commit 3",
                "feature2 commit",
            ]
        );

        let master_branch = repo.find_local_branch("master").unwrap();
        dbg!(&master_branch.id);
        assert_eq!(master_branch.id, master_commit.id);

        // feature1 was correctly re-targeted to last fixup
        let feature1_branch = repo.find_local_branch("feature1").unwrap();
        let feature1_commit = repo.find_commit(feature1_branch.id).unwrap();
        assert_eq!(
            feature1_commit.summary.to_str(),
            Ok("fixup! feature1 commit 2")
        );

        // feature2 was correctly re-targeted to last non-fixup
        let feature2_commit = repo.find_commit(feature2_branch.id).unwrap();
        assert_eq!(feature2_commit.summary.to_str(), Ok("feature2 commit"));
    }

    #[test]
    fn stray_fixups() {
        let mut repo = git_stack::git::InMemoryRepo::new();
        let plan =
            git_fixture::Dag::load(std::path::Path::new("tests/fixtures/fixup.yml")).unwrap();
        fixture::populate_repo(&mut repo, plan);

        let master_branch = repo.find_local_branch("feature1").unwrap();

        let mut protected_branches = git_stack::git::Branches::default();
        protected_branches.insert(master_branch.clone());

        let mut graphed_branches = git_stack::git::Branches::default();
        graphed_branches.insert(master_branch.clone());
        graphed_branches.insert(repo.find_local_branch("feature1").unwrap());
        graphed_branches.insert(repo.find_local_branch("feature2").unwrap());

        let master_commit = repo.find_commit(master_branch.id).unwrap();

        let mut graph = Graph::from_branches(&repo, graphed_branches).unwrap();
        git_stack::graph::protect_branches(&mut graph, &repo, &protected_branches);
        git_stack::graph::fixup(&mut graph, git_stack::config::Fixup::Move);
        let script = git_stack::graph::to_script(&graph);
        dbg!(&script);

        let mut executor = git_stack::git::Executor::new(&repo, false);
        let result = executor.run_script(&mut repo, &script);
        assert_eq!(result, vec![]);
        executor.close(&mut repo, "master").unwrap();
        dbg!(&repo);

        let feature2_branch = repo.find_local_branch("feature2").unwrap();
        let mut commits: Vec<_> = repo
            .commits_from(feature2_branch.id)
            .map(|c| c.summary.to_str_lossy().into_owned())
            .collect();
        commits.reverse();
        assert_eq!(
            commits,
            &[
                "commit 1",
                "commit 2",
                "master commit",
                "feature1 commit 1",
                "feature1 commit 2",
                "fixup! feature1 commit 2",
                "fixup! feature1 commit 1",
                "fixup! feature1 commit 1",
                "fixup! feature1 commit 1",
                "feature1 commit 3",
                "feature2 commit",
            ]
        );

        let master_branch = repo.find_local_branch("feature1").unwrap();
        dbg!(&master_branch.id);
        assert_eq!(master_branch.id, master_commit.id);

        // feature1 was correctly re-targeted to last fixup
        let feature1_branch = repo.find_local_branch("feature1").unwrap();
        let feature1_commit = repo.find_commit(feature1_branch.id).unwrap();
        assert_eq!(feature1_commit.summary.to_str(), Ok("feature1 commit 2"));

        // feature2 was correctly re-targeted to last non-fixup
        let feature2_commit = repo.find_commit(feature2_branch.id).unwrap();
        assert_eq!(feature2_commit.summary.to_str(), Ok("feature2 commit"));
    }
}

#[test]
fn overflow() {
    let mut repo = git_stack::git::InMemoryRepo::new();
    let mut plan = git_fixture::Dag::default();
    plan.events
        .push(git_fixture::Event::Tree(git_fixture::Tree {
            tracked: maplit::hashmap! {
                std::path::PathBuf::from("file.txt") => "content base".into(),
            },
            message: Some("Base Commit".to_owned()),
            author: Some("Someone <email>".to_owned()),
            branch: Some(git_fixture::Branch::new("base")),
            ..Default::default()
        }));
    for i in 0..1000 {
        plan.events
            .push(git_fixture::Event::Tree(git_fixture::Tree {
                tracked: maplit::hashmap! {
                    std::path::PathBuf::from("file.txt") => format!("content {}", i).into(),
                },
                message: Some(format!("Shared Commit {}", i)),
                author: Some("Someone <email>".to_owned()),
                ..Default::default()
            }));
    }
    plan.events
        .push(git_fixture::Event::Tree(git_fixture::Tree {
            tracked: maplit::hashmap! {
                std::path::PathBuf::from("file.txt") => "content master".into(),
            },
            message: Some("Master Commit".to_owned()),
            author: Some("Someone <email>".to_owned()),
            branch: Some(git_fixture::Branch::new("master")),
            ..Default::default()
        }));
    for i in 0..49 {
        plan.events
            .push(git_fixture::Event::Tree(git_fixture::Tree {
                tracked: maplit::hashmap! {
                    std::path::PathBuf::from("file.txt") => format!("content {}", i).into(),
                },
                message: Some(format!("Private Commit {}", i)),
                author: Some("Myself <email>".to_owned()),
                ..Default::default()
            }));
    }
    plan.events
        .push(git_fixture::Event::Tree(git_fixture::Tree {
            tracked: maplit::hashmap! {
                std::path::PathBuf::from("file.txt") => "content feature".into(),
            },
            message: Some("Feature Commit".to_owned()),
            author: Some("Myself <email>".to_owned()),
            branch: Some(git_fixture::Branch::new("feature")),
            ..Default::default()
        }));
    fixture::populate_repo(&mut repo, plan);

    let mut graphed_branches = git_stack::git::Branches::default();
    graphed_branches.insert(repo.find_local_branch("base").unwrap());
    graphed_branches.insert(repo.find_local_branch("master").unwrap());
    graphed_branches.insert(repo.find_local_branch("feature").unwrap());

    let mut protected_branches = git_stack::git::Branches::default();
    protected_branches.insert(repo.find_local_branch("master").unwrap());

    let mut graph = git_stack::graph::Graph::from_branches(&repo, graphed_branches).unwrap();
    git_stack::graph::protect_branches(&mut graph, &repo, &protected_branches);
    git_stack::graph::protect_large_branches(&mut graph, 50);
    git_stack::graph::protect_foreign_branches(&mut graph, "Myself");

    git_stack::graph::drop_squashed_by_tree_id(&mut graph);
    git_stack::graph::fixup(&mut graph, git_stack::config::Fixup::Move);

    let script = git_stack::graph::to_script(&graph);
    let mut executor = git_stack::git::Executor::new(&repo, false);
    let result = executor.run_script(&mut repo, &script);
    assert_eq!(result, vec![]);
    executor.close(&mut repo, "master").unwrap();
}
