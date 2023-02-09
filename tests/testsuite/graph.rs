use git_stack::graph::*;

use crate::fixture;

fn protect() -> git_stack::git::ProtectedBranches {
    git_stack::git::ProtectedBranches::new(vec!["master"]).unwrap()
}

fn to_oid(number: usize) -> git2::Oid {
    let sha = format!("{number:040x}");
    git2::Oid::from_str(&sha).unwrap()
}

#[test]
fn from_branches() {
    let mut repo = git_stack::git::InMemoryRepo::new();
    let plan =
        git_fixture::TodoList::load(std::path::Path::new("tests/fixtures/branches.yml")).unwrap();
    fixture::populate_repo(&mut repo, plan);

    let protect = protect();
    let branches = BranchSet::from_repo(&repo, &protect).unwrap();
    let graph = Graph::from_branches(&repo, branches).unwrap();

    assert_eq!(graph.root_id(), to_oid(1));
}

#[test]
fn descendants() {
    let mut repo = git_stack::git::InMemoryRepo::new();
    let plan =
        git_fixture::TodoList::load(std::path::Path::new("tests/fixtures/branches.yml")).unwrap();
    fixture::populate_repo(&mut repo, plan);

    let protect = protect();
    let branches = BranchSet::from_repo(&repo, &protect).unwrap();
    let graph = Graph::from_branches(&repo, branches).unwrap();
    let actual = graph.descendants().collect::<Vec<_>>();

    let expected = vec![
        to_oid(1),
        to_oid(2),
        to_oid(3),
        to_oid(4),
        to_oid(7),
        to_oid(5),
        to_oid(8),
        to_oid(6),
        to_oid(9),
        to_oid(10),
    ];
    assert_eq!(actual, expected);
    assert!(actual.contains(&graph.root_id()));
}

#[test]
fn descendants_of_root() {
    let mut repo = git_stack::git::InMemoryRepo::new();
    let plan =
        git_fixture::TodoList::load(std::path::Path::new("tests/fixtures/branches.yml")).unwrap();
    fixture::populate_repo(&mut repo, plan);

    let protect = protect();
    let branches = BranchSet::from_repo(&repo, &protect).unwrap();
    let graph = Graph::from_branches(&repo, branches).unwrap();
    let fixture = graph.root_id();
    let actual = graph.descendants_of(fixture).collect::<Vec<_>>();

    let expected = graph.descendants().collect::<Vec<_>>();
    assert_eq!(actual, expected);
    assert!(actual.contains(&fixture));
}

#[test]
fn descendants_of_master() {
    let mut repo = git_stack::git::InMemoryRepo::new();
    let plan =
        git_fixture::TodoList::load(std::path::Path::new("tests/fixtures/branches.yml")).unwrap();
    fixture::populate_repo(&mut repo, plan);

    let protect = protect();
    let branches = BranchSet::from_repo(&repo, &protect).unwrap();
    let graph = Graph::from_branches(&repo, branches).unwrap();
    let fixture = repo.find_local_branch("master").unwrap().id;
    let actual = graph.descendants_of(fixture).collect::<Vec<_>>();

    let expected = vec![to_oid(5), to_oid(6)];
    assert_eq!(actual, expected);
    assert!(actual.contains(&fixture));
}

#[test]
fn ancestors_of_root() {
    let mut repo = git_stack::git::InMemoryRepo::new();
    let plan =
        git_fixture::TodoList::load(std::path::Path::new("tests/fixtures/branches.yml")).unwrap();
    fixture::populate_repo(&mut repo, plan);

    let protect = protect();
    let branches = BranchSet::from_repo(&repo, &protect).unwrap();
    let graph = Graph::from_branches(&repo, branches).unwrap();
    let fixture = graph.root_id();
    let actual = graph.ancestors_of(fixture).collect::<Vec<_>>();

    let expected = vec![to_oid(1)];
    assert_eq!(actual, expected);
    assert!(actual.contains(&fixture));
}

#[test]
fn ancestors_of_master() {
    let mut repo = git_stack::git::InMemoryRepo::new();
    let plan =
        git_fixture::TodoList::load(std::path::Path::new("tests/fixtures/branches.yml")).unwrap();
    fixture::populate_repo(&mut repo, plan);

    let protect = protect();
    let branches = BranchSet::from_repo(&repo, &protect).unwrap();
    let graph = Graph::from_branches(&repo, branches).unwrap();
    let fixture = repo.find_local_branch("master").unwrap().id;
    let actual = graph.ancestors_of(fixture).collect::<Vec<_>>();

    let expected = vec![to_oid(5), to_oid(4), to_oid(3), to_oid(2), to_oid(1)];
    assert_eq!(actual, expected);
    assert!(actual.contains(&fixture));
}

#[test]
fn parents_of_root() {
    let mut repo = git_stack::git::InMemoryRepo::new();
    let plan =
        git_fixture::TodoList::load(std::path::Path::new("tests/fixtures/branches.yml")).unwrap();
    fixture::populate_repo(&mut repo, plan);

    let protect = protect();
    let branches = BranchSet::from_repo(&repo, &protect).unwrap();
    let graph = Graph::from_branches(&repo, branches).unwrap();
    let fixture = graph.root_id();
    let actual = graph.parents_of(fixture).collect::<Vec<_>>();

    let expected = vec![];
    assert_eq!(actual, expected);
    assert!(!actual.contains(&fixture));
}

#[test]
fn parents_of_master() {
    let mut repo = git_stack::git::InMemoryRepo::new();
    let plan =
        git_fixture::TodoList::load(std::path::Path::new("tests/fixtures/branches.yml")).unwrap();
    fixture::populate_repo(&mut repo, plan);

    let protect = protect();
    let branches = BranchSet::from_repo(&repo, &protect).unwrap();
    let graph = Graph::from_branches(&repo, branches).unwrap();
    let fixture = repo.find_local_branch("master").unwrap().id;
    let actual = graph.parents_of(fixture).collect::<Vec<_>>();

    let expected = vec![to_oid(4)];
    assert_eq!(actual, expected);
    assert!(!actual.contains(&fixture));
}

#[test]
fn children_of_root() {
    let mut repo = git_stack::git::InMemoryRepo::new();
    let plan =
        git_fixture::TodoList::load(std::path::Path::new("tests/fixtures/branches.yml")).unwrap();
    fixture::populate_repo(&mut repo, plan);

    let protect = protect();
    let branches = BranchSet::from_repo(&repo, &protect).unwrap();
    let graph = Graph::from_branches(&repo, branches).unwrap();
    let fixture = graph.root_id();
    let actual = graph.children_of(fixture).collect::<Vec<_>>();

    let expected = vec![to_oid(2)];
    assert_eq!(actual, expected);
    assert!(!actual.contains(&fixture));
}

#[test]
fn children_of_base() {
    let mut repo = git_stack::git::InMemoryRepo::new();
    let plan =
        git_fixture::TodoList::load(std::path::Path::new("tests/fixtures/branches.yml")).unwrap();
    fixture::populate_repo(&mut repo, plan);

    let protect = protect();
    let branches = BranchSet::from_repo(&repo, &protect).unwrap();
    let graph = Graph::from_branches(&repo, branches).unwrap();
    let fixture = repo.find_local_branch("base").unwrap().id;
    let actual = graph.children_of(fixture).collect::<Vec<_>>();

    let expected = vec![to_oid(4), to_oid(7)];
    assert_eq!(actual, expected);
    assert!(!actual.contains(&fixture));
}

#[test]
fn remove_non_existent() {
    let mut repo = git_stack::git::InMemoryRepo::new();
    let plan =
        git_fixture::TodoList::load(std::path::Path::new("tests/fixtures/branches.yml")).unwrap();
    fixture::populate_repo(&mut repo, plan);

    let protect = protect();
    let branches = BranchSet::from_repo(&repo, &protect).unwrap();
    let mut graph = Graph::from_branches(&repo, branches).unwrap();
    let fixture = to_oid(999999);
    let expected_ids = graph.descendants().collect::<Vec<_>>();
    let actual = graph.remove(fixture);

    assert!(actual.is_none());

    let actual_ids = graph.descendants().collect::<Vec<_>>();
    assert_eq!(actual_ids, expected_ids);
}

#[test]
#[should_panic]
fn rebase_root() {
    let mut repo = git_stack::git::InMemoryRepo::new();
    let plan =
        git_fixture::TodoList::load(std::path::Path::new("tests/fixtures/branches.yml")).unwrap();
    fixture::populate_repo(&mut repo, plan);

    let protect = protect();
    let branches = BranchSet::from_repo(&repo, &protect).unwrap();
    let mut graph = Graph::from_branches(&repo, branches).unwrap();
    let fixture = graph.root_id();
    let from = repo.find_local_branch("off_master").unwrap().id;
    let to = repo.find_local_branch("master").unwrap().id;
    graph.rebase(fixture, from, to);
}

#[test]
#[should_panic]
fn rebase_from_non_parent() {
    let mut repo = git_stack::git::InMemoryRepo::new();
    let plan =
        git_fixture::TodoList::load(std::path::Path::new("tests/fixtures/branches.yml")).unwrap();
    fixture::populate_repo(&mut repo, plan);

    let protect = protect();
    let branches = BranchSet::from_repo(&repo, &protect).unwrap();
    let mut graph = Graph::from_branches(&repo, branches).unwrap();
    let fixture = repo.find_local_branch("base").unwrap().id;
    let from = repo.find_local_branch("off_master").unwrap().id;
    let to = repo.find_local_branch("master").unwrap().id;
    graph.rebase(fixture, from, to);
}

#[test]
fn rebase() {
    let mut repo = git_stack::git::InMemoryRepo::new();
    let plan =
        git_fixture::TodoList::load(std::path::Path::new("tests/fixtures/branches.yml")).unwrap();
    fixture::populate_repo(&mut repo, plan);

    let protect = protect();
    let branches = BranchSet::from_repo(&repo, &protect).unwrap();
    let mut graph = Graph::from_branches(&repo, branches).unwrap();
    let fixture = repo.find_local_branch("feature1").unwrap().id;
    let from = repo.find_local_branch("base").unwrap().id;
    let to = repo.find_local_branch("master").unwrap().id;
    graph.rebase(fixture, from, to);

    let actual_ids = graph.descendants().collect::<Vec<_>>();
    let expected_ids = vec![
        to_oid(1),
        to_oid(2),
        to_oid(3),
        to_oid(4),
        to_oid(5),
        to_oid(6),
        to_oid(7),
        to_oid(8),
        to_oid(9),
        to_oid(10),
    ];
    assert_eq!(actual_ids, expected_ids);

    let actual_children = graph.children_of(to).collect::<Vec<_>>();
    let expected_children = vec![to_oid(6), fixture];
    assert_eq!(actual_children, expected_children);

    let actual_children = graph.children_of(from).collect::<Vec<_>>();
    let expected_children = vec![to_oid(4)];
    assert_eq!(actual_children, expected_children);

    let actual_parents = graph.parents_of(fixture).collect::<Vec<_>>();
    let expected_parents = vec![to];
    assert_eq!(actual_parents, expected_parents);
}

#[test]
#[should_panic]
fn remove_root() {
    let mut repo = git_stack::git::InMemoryRepo::new();
    let plan =
        git_fixture::TodoList::load(std::path::Path::new("tests/fixtures/branches.yml")).unwrap();
    fixture::populate_repo(&mut repo, plan);

    let protect = protect();
    let branches = BranchSet::from_repo(&repo, &protect).unwrap();
    let mut graph = Graph::from_branches(&repo, branches).unwrap();
    let fixture = graph.root_id();
    graph.remove(fixture);
}

#[test]
fn remove_base() {
    let mut repo = git_stack::git::InMemoryRepo::new();
    let plan =
        git_fixture::TodoList::load(std::path::Path::new("tests/fixtures/branches.yml")).unwrap();
    fixture::populate_repo(&mut repo, plan);

    let protect = protect();
    let branches = BranchSet::from_repo(&repo, &protect).unwrap();
    let mut graph = Graph::from_branches(&repo, branches).unwrap();
    let fixture = repo.find_local_branch("base").unwrap().id;
    let expected_children = graph
        .children_of(fixture)
        .collect::<std::collections::HashSet<_>>();
    let actual = graph.remove(fixture);

    assert!(actual.is_some());

    let actual_ids = graph.descendants().collect::<Vec<_>>();
    let expected_ids = vec![
        to_oid(1),
        to_oid(2),
        to_oid(7),
        to_oid(4),
        to_oid(8),
        to_oid(5),
        to_oid(9),
        to_oid(6),
        to_oid(10),
    ];
    assert_eq!(actual_ids, expected_ids);

    let actual_children = graph
        .children_of(to_oid(2))
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(actual_children, expected_children);
}
