mod test_branches {
    use git_stack::branches::*;

    fn protect() -> git_stack::protect::ProtectedBranches {
        git_stack::protect::ProtectedBranches::new(vec!["master"]).unwrap()
    }

    #[test]
    fn test_all() {
        let temp = assert_fs::TempDir::new().unwrap();
        let plan = git_fixture::Dag::load(std::path::Path::new("tests/fixtures/fork.yml")).unwrap();
        plan.run(temp.path()).unwrap();

        let repo = git2::Repository::discover(temp.path()).unwrap();

        let branches = Branches::new(&repo).unwrap();
        let result = branches.all(&repo);
        let mut names: Vec<_> = result
            .iter()
            .flat_map(|(_, b)| b.iter().map(|b| b.name().unwrap().unwrap()))
            .collect();
        names.sort_unstable();

        assert_eq!(
            names,
            [
                "base",
                "feature1",
                "feature2",
                "initial",
                "master",
                "off_master"
            ]
        );

        temp.close().unwrap();
    }

    #[test]
    fn test_dependents() {
        let temp = assert_fs::TempDir::new().unwrap();
        let plan = git_fixture::Dag::load(std::path::Path::new("tests/fixtures/fork.yml")).unwrap();
        plan.run(temp.path()).unwrap();

        let repo = git2::Repository::discover(temp.path()).unwrap();

        let base_oid = git_stack::git::resolve_name(&repo, "base").unwrap();
        let head_oid = git_stack::git::resolve_name(&repo, "feature1").unwrap();

        let branches = Branches::new(&repo).unwrap();
        let result = branches.dependents(&repo, base_oid, head_oid);
        let mut names: Vec<_> = result
            .iter()
            .flat_map(|(_, b)| b.iter().map(|b| b.name().unwrap().unwrap()))
            .collect();
        names.sort_unstable();

        // Shouldn't pick up master (branches off base)
        assert_eq!(names, ["base", "feature1", "feature2"]);

        temp.close().unwrap();
    }

    #[test]
    fn test_branch() {
        let temp = assert_fs::TempDir::new().unwrap();
        let plan = git_fixture::Dag::load(std::path::Path::new("tests/fixtures/fork.yml")).unwrap();
        plan.run(temp.path()).unwrap();

        let repo = git2::Repository::discover(temp.path()).unwrap();

        let base_oid = git_stack::git::resolve_name(&repo, "base").unwrap();
        let head_oid = git_stack::git::resolve_name(&repo, "feature1").unwrap();

        let branches = Branches::new(&repo).unwrap();
        let result = branches.branch(&repo, base_oid, head_oid);
        let mut names: Vec<_> = result
            .iter()
            .flat_map(|(_, b)| b.iter().map(|b| b.name().unwrap().unwrap()))
            .collect();
        names.sort_unstable();

        // Shouldn't pick up feature1 (dependent) or master (branches off base)
        assert_eq!(names, ["base", "feature1"]);

        temp.close().unwrap();
    }

    #[test]
    fn test_protected() {
        let temp = assert_fs::TempDir::new().unwrap();
        let plan = git_fixture::Dag::load(std::path::Path::new("tests/fixtures/fork.yml")).unwrap();
        plan.run(temp.path()).unwrap();

        let repo = git2::Repository::discover(temp.path()).unwrap();

        let protect = protect();
        let branches = Branches::new(&repo).unwrap();
        let result = branches.protected(&repo, &protect);
        let mut names: Vec<_> = result
            .iter()
            .flat_map(|(_, b)| b.iter().map(|b| b.name().unwrap().unwrap()))
            .collect();
        names.sort_unstable();

        assert_eq!(names, ["master"]);

        temp.close().unwrap();
    }
}

mod test_find_protected_base {
    use git_stack::branches::*;

    fn no_protect() -> git_stack::protect::ProtectedBranches {
        git_stack::protect::ProtectedBranches::new(vec![]).unwrap()
    }

    fn protect() -> git_stack::protect::ProtectedBranches {
        git_stack::protect::ProtectedBranches::new(vec!["master"]).unwrap()
    }

    #[test]
    fn test_no_protected() {
        let temp = assert_fs::TempDir::new().unwrap();
        let plan = git_fixture::Dag::load(std::path::Path::new("tests/fixtures/fork.yml")).unwrap();
        plan.run(temp.path()).unwrap();

        let repo = git2::Repository::discover(temp.path()).unwrap();

        let protect = no_protect();
        let branches = git_stack::branches::Branches::new(&repo).unwrap();
        let protected = branches.protected(&repo, &protect);

        let head_oid = git_stack::git::resolve_name(&repo, "base").unwrap();

        let branch = find_protected_base(&repo, &protected, head_oid);
        if let Ok(branch) = branch {
            let name = branch.name().unwrap();
            panic!("Should have failed but found {:?}", name);
        }

        temp.close().unwrap();
    }

    #[test]
    fn test_protected_branch() {
        let temp = assert_fs::TempDir::new().unwrap();
        let plan = git_fixture::Dag::load(std::path::Path::new("tests/fixtures/fork.yml")).unwrap();
        plan.run(temp.path()).unwrap();

        let repo = git2::Repository::discover(temp.path()).unwrap();

        let protect = protect();
        let branches = git_stack::branches::Branches::new(&repo).unwrap();
        let protected = branches.protected(&repo, &protect);

        let head_oid = git_stack::git::resolve_name(&repo, "off_master").unwrap();

        let branch = find_protected_base(&repo, &protected, head_oid);
        if let Err(error) = branch {
            panic!("Should have passed but found {:?}", error);
        }

        temp.close().unwrap();
    }

    #[test]
    fn test_protected_base() {
        let temp = assert_fs::TempDir::new().unwrap();
        let plan = git_fixture::Dag::load(std::path::Path::new("tests/fixtures/fork.yml")).unwrap();
        plan.run(temp.path()).unwrap();

        let repo = git2::Repository::discover(temp.path()).unwrap();

        let protect = protect();
        let branches = git_stack::branches::Branches::new(&repo).unwrap();
        let protected = branches.protected(&repo, &protect);

        let head_oid = git_stack::git::resolve_name(&repo, "base").unwrap();

        let branch = find_protected_base(&repo, &protected, head_oid);
        if let Err(error) = branch {
            panic!("Should have passed but found {:?}", error);
        }

        temp.close().unwrap();
    }
}
