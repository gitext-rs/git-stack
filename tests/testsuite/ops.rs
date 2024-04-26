use bstr::ByteSlice;

use git_stack::graph::*;

use crate::fixture;

fn protect() -> git_stack::git::ProtectedBranches {
    git_stack::git::ProtectedBranches::new(vec!["master"]).unwrap()
}

mod test_rebase {
    use super::*;

    #[test]
    fn no_op() {
        let mut repo = git_stack::git::InMemoryRepo::new();
        let plan = git_fixture::TodoList::load(std::path::Path::new("tests/fixtures/branches.yml"))
            .unwrap();
        fixture::populate_repo(&mut repo, plan);

        let protect = protect();
        let branches = git_stack::graph::BranchSet::from_repo(&repo, &protect).unwrap();

        let master_branch = repo.find_local_branch("master").unwrap();
        let master_commit = repo.find_commit(master_branch.id).unwrap();

        let mut graph = Graph::from_branches(&repo, branches).unwrap();
        protect_branches(&mut graph);
        rebase_development_branches(&mut graph, master_commit.id);
        let scripts = to_scripts(&graph, vec![]);
        dbg!(&scripts);

        let mut executor = git_stack::rewrite::Executor::new(false);
        for script in scripts {
            let result = executor.run(&mut repo, &script);
            assert_eq!(result, vec![]);
        }
        executor.close(&mut repo, Some("off_master")).unwrap();
        dbg!(&repo);

        let master_branch = repo.find_local_branch("master").unwrap();
        dbg!(&master_branch.id);
        assert_eq!(master_branch.id, master_commit.id);

        let off_master_branch = repo.find_local_branch("off_master").unwrap();
        let ancestors = git_stack::git::commit_range(&repo, off_master_branch.id..).unwrap();
        dbg!(&ancestors);
        assert!(ancestors.contains(&master_branch.id));
    }

    #[test]
    fn rebase() {
        let mut repo = git_stack::git::InMemoryRepo::new();
        let plan = git_fixture::TodoList::load(std::path::Path::new("tests/fixtures/branches.yml"))
            .unwrap();
        fixture::populate_repo(&mut repo, plan);

        let protect = protect();
        let branches = git_stack::graph::BranchSet::from_repo(&repo, &protect).unwrap();

        let master_branch = repo.find_local_branch("master").unwrap();
        let master_commit = repo.find_commit(master_branch.id).unwrap();

        let mut graph = Graph::from_branches(&repo, branches).unwrap();
        protect_branches(&mut graph);
        rebase_development_branches(&mut graph, master_commit.id);
        let scripts = to_scripts(&graph, vec![]);
        dbg!(&scripts);

        let mut executor = git_stack::rewrite::Executor::new(false);
        for script in scripts {
            let result = executor.run(&mut repo, &script);
            assert_eq!(result, vec![]);
        }
        executor.close(&mut repo, Some("off_master")).unwrap();
        dbg!(&repo);

        let master_branch = repo.find_local_branch("master").unwrap();
        dbg!(&master_branch.id);
        assert_eq!(master_branch.id, master_commit.id);

        let feature2_branch = repo.find_local_branch("feature2").unwrap();
        let ancestors = git_stack::git::commit_range(&repo, feature2_branch.id..).unwrap();
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
        let plan = git_fixture::TodoList::load(std::path::Path::new("tests/fixtures/branches.yml"))
            .unwrap();
        fixture::populate_repo(&mut repo, plan);

        let protect = protect();
        let branches = git_stack::graph::BranchSet::from_repo(&repo, &protect).unwrap();

        let master_branch = repo.find_local_branch("master").unwrap();
        let master_commit = repo.find_commit(master_branch.id).unwrap();

        let mut graph = Graph::from_branches(&repo, branches).unwrap();
        protect_branches(&mut graph);
        fixup(&mut graph, &repo, git_stack::config::Fixup::Move);
        let scripts = to_scripts(&graph, vec![]);
        dbg!(&scripts);

        let mut executor = git_stack::rewrite::Executor::new(false);
        for script in scripts {
            let result = executor.run(&mut repo, &script);
            assert_eq!(result, vec![]);
        }
        executor.close(&mut repo, Some("off_master")).unwrap();
        dbg!(&repo);

        let master_branch = repo.find_local_branch("master").unwrap();
        dbg!(&master_branch.id);
        assert_eq!(master_branch.id, master_commit.id);

        let off_master_branch = repo.find_local_branch("off_master").unwrap();
        let ancestors = git_stack::git::commit_range(&repo, off_master_branch.id..).unwrap();
        dbg!(&ancestors);
        assert!(ancestors.contains(&master_branch.id));
    }

    #[test]
    fn fixup_move_after_target() {
        let mut repo = git_stack::git::InMemoryRepo::new();
        let plan =
            git_fixture::TodoList::load(std::path::Path::new("tests/fixtures/fixup.yml")).unwrap();
        fixture::populate_repo(&mut repo, plan);

        let protect = protect();
        let branches = git_stack::graph::BranchSet::from_repo(&repo, &protect).unwrap();

        let master_branch = repo.find_local_branch("master").unwrap();
        let master_commit = repo.find_commit(master_branch.id).unwrap();

        let mut graph = Graph::from_branches(&repo, branches).unwrap();
        protect_branches(&mut graph);
        fixup(&mut graph, &repo, git_stack::config::Fixup::Move);
        let scripts = to_scripts(&graph, vec![]);
        dbg!(&scripts);

        let mut executor = git_stack::rewrite::Executor::new(false);
        for script in scripts {
            let result = executor.run(&mut repo, &script);
            assert_eq!(result, vec![]);
        }
        executor.close(&mut repo, Some("master")).unwrap();
        dbg!(&repo);

        let feature2_branch = repo.find_local_branch("feature2").unwrap();
        let mut commits: Vec<_> = git_stack::git::commit_range(&repo, feature2_branch.id..)
            .unwrap()
            .into_iter()
            .map(|id| repo.find_commit(id).unwrap())
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
        fn protect() -> git_stack::git::ProtectedBranches {
            git_stack::git::ProtectedBranches::new(vec!["feature1"]).unwrap()
        }

        let mut repo = git_stack::git::InMemoryRepo::new();
        let plan =
            git_fixture::TodoList::load(std::path::Path::new("tests/fixtures/fixup.yml")).unwrap();
        fixture::populate_repo(&mut repo, plan);

        let protect = protect();
        let branches = git_stack::graph::BranchSet::from_repo(&repo, &protect).unwrap();

        let master_branch = repo.find_local_branch("feature1").unwrap();
        let master_commit = repo.find_commit(master_branch.id).unwrap();

        let mut graph = Graph::from_branches(&repo, branches).unwrap();
        protect_branches(&mut graph);
        fixup(&mut graph, &repo, git_stack::config::Fixup::Move);
        let scripts = to_scripts(&graph, vec![]);
        dbg!(&scripts);

        let mut executor = git_stack::rewrite::Executor::new(false);
        for script in scripts {
            let result = executor.run(&mut repo, &script);
            assert_eq!(result, vec![]);
        }
        executor.close(&mut repo, Some("master")).unwrap();
        dbg!(&repo);

        let feature2_branch = repo.find_local_branch("feature2").unwrap();
        let mut commits: Vec<_> = git_stack::git::commit_range(&repo, feature2_branch.id..)
            .unwrap()
            .into_iter()
            .map(|id| repo.find_commit(id).unwrap())
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
                "fixup! feature1 commit 1",
                "fixup! feature1 commit 1",
                "fixup! feature1 commit 1",
                "feature1 commit 3",
                "fixup! feature1 commit 2",
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
    let mut plan = git_fixture::TodoList::default();
    plan.commands
        .push(git_fixture::Command::Tree(git_fixture::Tree {
            files: maplit::hashmap! {
                std::path::PathBuf::from("file.txt") => "content base".into(),
            },
            message: Some("Base Commit".to_owned()),
            author: Some("Someone <email>".to_owned()),
        }));
    plan.commands
        .push(git_fixture::Command::Branch("base".into()));
    for i in 0..1000 {
        plan.commands
            .push(git_fixture::Command::Tree(git_fixture::Tree {
                files: maplit::hashmap! {
                    std::path::PathBuf::from("file.txt") => format!("content {i}").into(),
                },
                message: Some(format!("Shared Commit {i}")),
                author: Some("Someone <email>".to_owned()),
            }));
    }
    plan.commands
        .push(git_fixture::Command::Tree(git_fixture::Tree {
            files: maplit::hashmap! {
                std::path::PathBuf::from("file.txt") => "content master".into(),
            },
            message: Some("Master Commit".to_owned()),
            author: Some("Someone <email>".to_owned()),
        }));
    plan.commands
        .push(git_fixture::Command::Branch("master".into()));
    for i in 0..49 {
        plan.commands
            .push(git_fixture::Command::Tree(git_fixture::Tree {
                files: maplit::hashmap! {
                    std::path::PathBuf::from("file.txt") => format!("content {i}").into(),
                },
                message: Some(format!("Private Commit {i}")),
                author: Some("Myself <email>".to_owned()),
            }));
    }
    plan.commands
        .push(git_fixture::Command::Tree(git_fixture::Tree {
            files: maplit::hashmap! {
                std::path::PathBuf::from("file.txt") => "content feature".into(),
            },
            message: Some("Feature Commit".to_owned()),
            author: Some("Myself <email>".to_owned()),
        }));
    plan.commands
        .push(git_fixture::Command::Branch("feature".into()));
    fixture::populate_repo(&mut repo, plan);

    let protect = protect();
    let branches = git_stack::graph::BranchSet::from_repo(&repo, &protect).unwrap();

    let mut graph = git_stack::graph::Graph::from_branches(&repo, branches).unwrap();
    protect_branches(&mut graph);
    protect_large_branches(&mut graph, 50);
    protect_foreign_branches(&mut graph, &repo, "Myself", &[]);

    fixup(&mut graph, &repo, git_stack::config::Fixup::Move);

    let scripts = to_scripts(&graph, vec![]);
    let mut executor = git_stack::rewrite::Executor::new(false);
    for script in scripts {
        let result = executor.run(&mut repo, &script);
        assert_eq!(result, vec![]);
    }
    executor.close(&mut repo, Some("master")).unwrap();
}
