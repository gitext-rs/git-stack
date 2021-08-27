mod fixture;

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

        let mut graph_branches = git_stack::git::Branches::default();
        graph_branches.insert(master_branch.clone());
        graph_branches.insert(repo.find_local_branch("off_master").unwrap());

        let master_commit = repo.find_commit(master_branch.id).unwrap();

        let mut root = Node::from_branches(&repo, graph_branches).unwrap();
        git_stack::graph::protect_branches(&mut root, &repo, &protected_branches).unwrap();
        git_stack::graph::rebase_branches(&mut root, master_commit.id).unwrap();
        let script = git_stack::graph::to_script(&root);

        let mut executor = git_stack::git::Executor::new(&repo, false);
        let result = executor.run_script(&mut repo, &script);
        assert_eq!(result, vec![]);
        executor.close(&mut repo, "off_master").unwrap();

        let master_branch = repo.find_local_branch("master").unwrap();
        assert_eq!(master_branch.id, master_commit.id);

        let off_master_branch = repo.find_local_branch("off_master").unwrap();
        let ancestors: Vec<_> = repo
            .commits_from(off_master_branch.id)
            .map(|c| c.id)
            .collect();
        println!("{:#?}", ancestors);
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

        let mut graph_branches = git_stack::git::Branches::default();
        graph_branches.insert(master_branch.clone());
        graph_branches.insert(repo.find_local_branch("feature1").unwrap());
        graph_branches.insert(repo.find_local_branch("feature2").unwrap());

        let master_commit = repo.find_commit(master_branch.id).unwrap();

        let mut root = Node::from_branches(&repo, graph_branches).unwrap();
        git_stack::graph::protect_branches(&mut root, &repo, &protected_branches).unwrap();
        git_stack::graph::rebase_branches(&mut root, master_commit.id).unwrap();
        let script = git_stack::graph::to_script(&root);

        let mut executor = git_stack::git::Executor::new(&repo, false);
        let result = executor.run_script(&mut repo, &script);
        assert_eq!(result, vec![]);
        executor.close(&mut repo, "off_master").unwrap();

        let master_branch = repo.find_local_branch("master").unwrap();
        assert_eq!(master_branch.id, master_commit.id);

        let feature1_branch = repo.find_local_branch("feature1").unwrap();
        let feature2_branch = repo.find_local_branch("feature2").unwrap();
        let ancestors: Vec<_> = repo
            .commits_from(feature2_branch.id)
            .map(|c| c.id)
            .collect();
        println!("{:#?}", ancestors);
        assert!(ancestors.contains(&master_branch.id));
        assert!(ancestors.contains(&feature1_branch.id));
    }
}
