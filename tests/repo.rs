use assert_fs::prelude::*;

use git_stack::repo::*;

#[test]
fn shared_fixture() {
    let temp = assert_fs::TempDir::new().unwrap();
    let plan = git_fixture::Dag::load(std::path::Path::new("tests/fixtures/branches.yml")).unwrap();
    plan.run(temp.path()).unwrap();

    let repo = git2::Repository::discover(temp.path()).unwrap();
    let repo = GitRepo::new(repo);

    // is_dirty
    {
        {
            assert!(!repo.is_dirty());
        }

        {
            let untracked = temp.child("untracked.txt");
            untracked.touch().unwrap();
            assert!(!repo.is_dirty());
            std::fs::remove_file(untracked.path()).unwrap();
        }

        {
            let tracked = temp.child("file_a.txt");
            tracked.assert("3");
            tracked.write_str("200").unwrap();
            assert!(repo.is_dirty());
            tracked.write_str("3").unwrap();
        }
    }

    // merge_base
    {
        {
            let one = repo.find_local_branch("feature1").unwrap();
            let two = repo.find_local_branch("feature1").unwrap();

            let actual = repo.merge_base(one.id, two.id).unwrap();
            assert_eq!(actual, one.id);
        }

        {
            let one = repo.find_local_branch("feature1").unwrap();
            let two = git2::Oid::zero();

            let actual = repo.merge_base(one.id, two);
            assert!(actual.is_none());
        }

        {
            let one = repo.find_local_branch("feature1").unwrap();
            let two = repo.find_local_branch("feature2").unwrap();

            let actual = repo.merge_base(one.id, two.id).unwrap();
            assert_eq!(actual, one.id);
        }

        {
            let one = repo.find_local_branch("feature1").unwrap();
            let two = repo.find_local_branch("master").unwrap();
            let expected = repo.find_local_branch("base").unwrap();

            let actual = repo.merge_base(one.id, two.id).unwrap();
            assert_eq!(actual, expected.id);
        }
    }

    // find_commit
    {
        {
            let branch = repo.find_local_branch("feature1").unwrap();
            let actual = repo.find_commit(branch.id).unwrap();
            assert_eq!(actual.summary, "7");
        }

        {
            let actual = repo.find_commit(git2::Oid::zero());
            assert!(actual.is_none());
        }
    }

    // head_commit
    {
        {
            let expected = repo.find_local_branch("feature2").unwrap();
            let actual = repo.head_commit();
            assert_eq!(actual.id, expected.id);
        }
    }

    // commits_from
    {
        {
            let head = repo.find_local_branch("base").unwrap();
            let actual: Vec<_> = repo
                .commits_from(head.id)
                .map(|c| c.summary.clone())
                .collect();
            assert_eq!(actual, &["3", "2", "1"]);
        }
    }

    // local_branches
    {
        let mut actual: Vec<_> = repo.local_branches().map(|b| b.name).collect();
        actual.sort_unstable();
        assert_eq!(
            actual,
            &[
                "base",
                "feature1",
                "feature2",
                "initial",
                "master",
                "off_master"
            ]
        );
    }

    temp.close().unwrap();
}

#[test]
fn cherry_pick_clean() {
    let temp = assert_fs::TempDir::new().unwrap();
    let plan = git_fixture::Dag::load(std::path::Path::new("tests/fixtures/branches.yml")).unwrap();
    plan.run(temp.path()).unwrap();

    let repo = git2::Repository::discover(temp.path()).unwrap();
    let mut repo = GitRepo::new(repo);

    {
        let expected_head = repo.head_commit();
        assert!(!repo.is_dirty());

        let base = repo.find_local_branch("off_master").unwrap();
        let source = repo.find_local_branch("feature1").unwrap();
        let dest_id = repo.cherry_pick(base.id, source.id).unwrap();

        let source_commit = repo.find_commit(source.id).unwrap();
        let dest_commit = repo.find_commit(dest_id).unwrap();
        let actual_head = repo.head_commit();

        assert_ne!(dest_id, source.id);
        assert_eq!(dest_commit.summary, source_commit.summary);
        assert_eq!(expected_head.id, actual_head.id);
        assert!(!repo.is_dirty());
    }

    temp.close().unwrap();
}

#[test]
fn cherry_pick_conflict() {
    let temp = assert_fs::TempDir::new().unwrap();
    let plan = git_fixture::Dag::load(std::path::Path::new("tests/fixtures/conflict.yml")).unwrap();
    plan.run(temp.path()).unwrap();

    let repo = git2::Repository::discover(temp.path()).unwrap();
    let mut repo = GitRepo::new(repo);

    {
        assert!(!repo.is_dirty());

        let base = repo.find_local_branch("feature1").unwrap();
        let source = repo.find_local_branch("master").unwrap();
        let dest_id = repo.cherry_pick(base.id, source.id);
        println!("{:#?}", dest_id);
        assert!(dest_id.is_err());
        assert!(!repo.is_dirty());
    }

    temp.close().unwrap();
}

#[test]
fn branch() {
    let temp = assert_fs::TempDir::new().unwrap();
    let plan = git_fixture::Dag::load(std::path::Path::new("tests/fixtures/branches.yml")).unwrap();
    plan.run(temp.path()).unwrap();

    let repo = git2::Repository::discover(temp.path()).unwrap();
    let mut repo = GitRepo::new(repo);

    {
        let actual = repo.branch("new", git2::Oid::zero());
        assert!(actual.is_err());
    }

    // Add new branch
    {
        let base = repo.find_local_branch("base").unwrap();
        repo.branch("new", base.id).unwrap();
        let actual = repo.find_local_branch("new").unwrap();
        assert_eq!(base.id, actual.id);

        let mut actual: Vec<_> = repo.local_branches().map(|b| b.name).collect();
        actual.sort_unstable();
        assert_eq!(
            actual,
            &[
                "base",
                "feature1",
                "feature2",
                "initial",
                "master",
                "new",
                "off_master"
            ]
        );
    }

    // Point the branch to somewhere else
    {
        let old = repo.find_local_branch("feature1").unwrap();
        let target = repo.find_local_branch("off_master").unwrap();
        repo.branch("feature1", target.id).unwrap();
        let new = repo.find_local_branch("feature1").unwrap();
        assert_eq!(new.id, target.id);
        assert_ne!(new.id, old.id);
    }

    temp.close().unwrap();
}

#[test]
fn switch() {
    let temp = assert_fs::TempDir::new().unwrap();
    let plan = git_fixture::Dag::load(std::path::Path::new("tests/fixtures/branches.yml")).unwrap();
    plan.run(temp.path()).unwrap();

    let repo = git2::Repository::discover(temp.path()).unwrap();
    let mut repo = GitRepo::new(repo);

    {
        let actual = repo.switch("non-existent");
        assert!(actual.is_err());
    }

    {
        repo.switch("master").unwrap();
        let actual = repo.head_commit();
        let expected = repo.find_local_branch("master").unwrap();
        assert_eq!(actual.id, expected.id);
    }

    temp.close().unwrap();
}
