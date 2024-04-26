use assert_fs::prelude::*;

use git_stack::legacy::git::*;

#[test]
fn shared_branches_fixture() {
    let temp = assert_fs::TempDir::new().unwrap();
    let plan =
        git_fixture::TodoList::load(std::path::Path::new("tests/fixtures/branches.yml")).unwrap();
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
            let two = repo.find_local_branch("feature2").unwrap();

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

    // commit_range
    {
        {
            let head = repo.find_local_branch("base").unwrap();
            let actual: Vec<_> = commit_range(&repo, head.id..)
                .unwrap()
                .into_iter()
                .map(|id| repo.find_commit(id).unwrap())
                .map(|c| c.summary.clone())
                .collect();
            assert_eq!(actual, &["3", "2", "1"]);
        }
    }

    // local_branches
    {
        let mut actual: Vec<_> = repo.local_branches().map(|b| b.to_string()).collect();
        actual.sort_unstable();
        actual.retain(|b| b != "main"); // HACK: default branch name workaround
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
fn contains_commit_not_with_independent_branches() {
    let temp = assert_fs::TempDir::new().unwrap();
    let plan =
        git_fixture::TodoList::load(std::path::Path::new("tests/fixtures/branches.yml")).unwrap();
    plan.run(temp.path()).unwrap();

    let repo = git2::Repository::discover(temp.path()).unwrap();
    let repo = GitRepo::new(repo);

    let feature = repo.find_local_branch("feature2").unwrap();
    let master = repo.find_local_branch("master").unwrap();

    let feature_in_master = repo.contains_commit(master.id, feature.id).unwrap();
    assert!(!feature_in_master);

    temp.close().unwrap();
}

#[test]
fn contains_commit_rebased_branches_with_disjoint_commit() {
    let temp = assert_fs::TempDir::new().unwrap();
    let plan =
        git_fixture::TodoList::load(std::path::Path::new("tests/fixtures/git_rebase_new.yml"))
            .unwrap();
    plan.run(temp.path()).unwrap();

    let repo = git2::Repository::discover(temp.path()).unwrap();
    let repo = GitRepo::new(repo);

    let feature1 = repo.find_local_branch("feature1").unwrap();
    let feature2 = repo.find_local_branch("feature2").unwrap();

    let feature1_in_feature2 = repo.contains_commit(feature2.id, feature1.id).unwrap();
    assert!(feature1_in_feature2);

    let feature2_in_feature1 = repo.contains_commit(feature1.id, feature2.id).unwrap();
    assert!(!feature2_in_feature1);

    temp.close().unwrap();
}

#[test]
#[ignore] // Not correctly detecting the commit already exists earlier in history
fn contains_commit_rebased_branches_with_overlapping_commit() {
    let temp = assert_fs::TempDir::new().unwrap();
    let plan = git_fixture::TodoList::load(std::path::Path::new(
        "tests/fixtures/git_rebase_existing.yml",
    ))
    .unwrap();
    plan.run(temp.path()).unwrap();

    let repo = git2::Repository::discover(temp.path()).unwrap();
    let repo = GitRepo::new(repo);

    let feature1 = repo.find_local_branch("feature1").unwrap();
    let feature2 = repo.find_local_branch("feature2").unwrap();

    let feature1_in_feature2 = repo.contains_commit(feature2.id, feature1.id).unwrap();
    assert!(feature1_in_feature2);

    let feature2_in_feature1 = repo.contains_commit(feature1.id, feature2.id).unwrap();
    assert!(!feature2_in_feature1);

    temp.close().unwrap();
}

#[test]
#[ignore] // Not correctly detecting the commit already exists earlier in history
fn contains_commit_semi_linear_merge() {
    let temp = assert_fs::TempDir::new().unwrap();
    let plan = git_fixture::TodoList::load(std::path::Path::new(
        "tests/fixtures/pr-semi-linear-merge.yml",
    ))
    .unwrap();
    plan.run(temp.path()).unwrap();

    let repo = git2::Repository::discover(temp.path()).unwrap();
    let repo = GitRepo::new(repo);

    let old_master = repo.find_local_branch("old_master").unwrap();
    let master = repo.find_local_branch("master").unwrap();
    let feature1 = repo.find_local_branch("feature1").unwrap();
    let feature2 = repo.find_local_branch("feature2").unwrap();

    let feature1_in_master = repo.contains_commit(master.id, feature1.id).unwrap();
    assert!(feature1_in_master);
    let feature2_in_master = repo.contains_commit(master.id, feature2.id).unwrap();
    assert!(feature2_in_master);

    let feature1_in_old_master = repo.contains_commit(old_master.id, feature1.id).unwrap();
    assert!(feature1_in_old_master);
    let feature2_in_old_master = repo.contains_commit(old_master.id, feature2.id).unwrap();
    assert!(feature2_in_old_master);

    temp.close().unwrap();
}

#[test]
#[ignore] // Not correctly detecting the commit already exists earlier in history
fn contains_commit_pr_squashed() {
    let temp = assert_fs::TempDir::new().unwrap();
    let plan =
        git_fixture::TodoList::load(std::path::Path::new("tests/fixtures/pr-squash.yml")).unwrap();
    plan.run(temp.path()).unwrap();

    let repo = git2::Repository::discover(temp.path()).unwrap();
    let repo = GitRepo::new(repo);

    let old_master = repo.find_local_branch("old_master").unwrap();
    let master = repo.find_local_branch("master").unwrap();
    let feature1 = repo.find_local_branch("feature1").unwrap();
    let feature2 = repo.find_local_branch("feature2").unwrap();

    let feature1_in_master = repo.contains_commit(master.id, feature1.id).unwrap();
    assert!(feature1_in_master);
    let feature2_in_master = repo.contains_commit(master.id, feature2.id).unwrap();
    assert!(feature2_in_master);

    let feature1_in_old_master = repo.contains_commit(old_master.id, feature1.id).unwrap();
    assert!(feature1_in_old_master);
    let feature2_in_old_master = repo.contains_commit(old_master.id, feature2.id).unwrap();
    assert!(feature2_in_old_master);

    temp.close().unwrap();
}

#[test]
fn cherry_pick_clean() {
    let temp = assert_fs::TempDir::new().unwrap();
    let plan =
        git_fixture::TodoList::load(std::path::Path::new("tests/fixtures/branches.yml")).unwrap();
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
    let plan =
        git_fixture::TodoList::load(std::path::Path::new("tests/fixtures/conflict.yml")).unwrap();
    plan.run(temp.path()).unwrap();

    let repo = git2::Repository::discover(temp.path()).unwrap();
    let mut repo = GitRepo::new(repo);

    {
        assert!(!repo.is_dirty());

        let base = repo.find_local_branch("feature1").unwrap();
        let source = repo.find_local_branch("master").unwrap();
        let dest_id = repo.cherry_pick(base.id, source.id);
        println!("{dest_id:#?}");
        assert!(dest_id.is_err());
        assert!(!repo.is_dirty());
    }

    temp.close().unwrap();
}

#[test]
fn squash_clean() {
    let temp = assert_fs::TempDir::new().unwrap();
    let plan =
        git_fixture::TodoList::load(std::path::Path::new("tests/fixtures/branches.yml")).unwrap();
    plan.run(temp.path()).unwrap();

    let repo = git2::Repository::discover(temp.path()).unwrap();
    let mut repo = GitRepo::new(repo);

    {
        assert!(!repo.is_dirty());

        let base = repo.find_local_branch("master").unwrap();
        let source = repo.find_local_branch("feature1").unwrap();
        let dest_id = repo.squash(source.id, base.id).unwrap();

        repo.branch("squashed", dest_id).unwrap();
        assert!(!repo.is_dirty());
    }

    temp.close().unwrap();
}

#[test]
fn branch() {
    let temp = assert_fs::TempDir::new().unwrap();
    let plan =
        git_fixture::TodoList::load(std::path::Path::new("tests/fixtures/branches.yml")).unwrap();
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

        let mut actual: Vec<_> = repo.local_branches().map(|b| b.to_string()).collect();
        actual.sort_unstable();
        actual.retain(|b| b != "main"); // HACK: default branch name workaround
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
    let plan =
        git_fixture::TodoList::load(std::path::Path::new("tests/fixtures/branches.yml")).unwrap();
    plan.run(temp.path()).unwrap();

    let repo = git2::Repository::discover(temp.path()).unwrap();
    let mut repo = GitRepo::new(repo);

    {
        let actual = repo.switch("non-existent");
        assert!(actual.is_err());
    }

    {
        assert!(!repo.is_dirty());

        repo.switch("master").unwrap();
        let actual = repo.head_commit();
        let expected = repo.find_local_branch("master").unwrap();
        assert_eq!(actual.id, expected.id);
        assert!(!repo.is_dirty());
    }

    temp.close().unwrap();
}
