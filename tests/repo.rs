use assert_fs::prelude::*;

use git_stack::repo::*;

#[test]
fn shared_fixture() {
    let temp = assert_fs::TempDir::new().unwrap();
    let plan = git_fixture::Dag::load(std::path::Path::new("tests/fixtures/fork.yml")).unwrap();
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
            assert_eq!(actual.summary, "8");
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
