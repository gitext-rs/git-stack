mod branch;
mod commit;
mod ops;

pub use branch::*;
pub use commit::*;
pub use ops::*;

use std::collections::BTreeMap;
use std::collections::VecDeque;

use crate::any::AnyId;
use crate::any::BoxedEntry;
use crate::any::BoxedResource;
use crate::any::Resource;

#[derive(Clone, Debug)]
pub struct Graph {
    graph: petgraph::graphmap::DiGraphMap<git2::Oid, usize>,
    root_id: git2::Oid,
    commits: BTreeMap<git2::Oid, BTreeMap<AnyId, BoxedResource>>,
    pub branches: BranchSet,
}

impl Graph {
    pub fn from_branches(
        repo: &dyn crate::git::Repo,
        branches: BranchSet,
    ) -> crate::git::Result<Self> {
        let mut root_id = None;
        for branch_id in branches.oids() {
            if let Some(old_root_id) = root_id {
                root_id = repo.merge_base(old_root_id, branch_id);
                if root_id.is_none() {
                    return Err(git2::Error::new(
                        git2::ErrorCode::NotFound,
                        git2::ErrorClass::Reference,
                        format!("no merge base between {old_root_id} and {branch_id}"),
                    ));
                }
            } else {
                root_id = Some(branch_id);
            }
        }
        let root_id = root_id.ok_or_else(|| {
            git2::Error::new(
                git2::ErrorCode::NotFound,
                git2::ErrorClass::Reference,
                "at least one branch is required to make a graph",
            )
        })?;

        let mut graph = Graph::with_base_id(root_id);
        graph.branches = branches;
        for branch_id in graph.branches.oids() {
            for commit_id in crate::git::commit_range(repo, branch_id..root_id)? {
                for (weight, parent_id) in repo.parent_ids(commit_id)?.into_iter().enumerate() {
                    graph.graph.add_edge(commit_id, parent_id, weight);
                }
            }
        }

        Ok(graph)
    }

    pub fn insert(&mut self, node: Node, parent_id: git2::Oid) {
        assert!(
            self.contains_id(parent_id),
            "expected to contain {parent_id}",
        );
        let Node {
            id,
            branches,
            commit,
        } = node;
        self.graph.add_edge(id, parent_id, 0);
        for branch in branches.into_iter().flatten() {
            self.branches.insert(branch);
        }
        if let Some(commit) = commit {
            self.commits.insert(id, commit);
        }
    }

    pub fn rebase(&mut self, id: git2::Oid, from: git2::Oid, to: git2::Oid) {
        assert!(self.contains_id(id), "expected to contain {id}");
        assert!(self.contains_id(from), "expected to contain {from}");
        assert!(self.contains_id(to), "expected to contain {to}");
        assert_eq!(
            self.parents_of(id).find(|parent| *parent == from),
            Some(from)
        );
        assert_ne!(id, self.root_id, "Cannot rebase root ({id})");
        let weight = self.graph.remove_edge(id, from).unwrap();
        self.graph.add_edge(id, to, weight);
    }

    pub fn remove(&mut self, id: git2::Oid) -> Option<Node> {
        assert_ne!(id, self.root_id, "Cannot remove root ({id})");
        let children = self.children_of(id).collect::<Vec<_>>();
        if !children.is_empty() {
            let parents = self.parents_of(id).collect::<Vec<_>>();
            for child_id in children.iter().copied() {
                for (weight, parent_id) in parents.iter().copied().enumerate() {
                    self.graph.add_edge(child_id, parent_id, weight);
                }
            }
        }
        self.graph.remove_node(id).then(|| {
            let branches = self.branches.remove(id);
            let commit = self.commits.remove(&id);
            Node {
                id,
                branches,
                commit,
            }
        })
    }
}

impl Graph {
    pub fn with_base_id(root_id: git2::Oid) -> Self {
        let mut graph = petgraph::graphmap::DiGraphMap::new();
        graph.add_node(root_id);
        let commits = BTreeMap::new();
        let branches = BranchSet::new();
        Self {
            graph,
            root_id,
            commits,
            branches,
        }
    }

    pub fn root_id(&self) -> git2::Oid {
        self.root_id
    }

    pub fn contains_id(&self, id: git2::Oid) -> bool {
        self.graph.contains_node(id)
    }

    pub fn primary_parent_of(&self, root_id: git2::Oid) -> Option<git2::Oid> {
        self.graph
            .edges_directed(root_id, petgraph::Direction::Outgoing)
            .filter_map(|(_child, parent, weight)| (*weight == 0).then_some(parent))
            .next()
    }

    pub fn parents_of(
        &self,
        root_id: git2::Oid,
    ) -> petgraph::graphmap::NeighborsDirected<'_, git2::Oid, petgraph::Directed> {
        self.graph
            .neighbors_directed(root_id, petgraph::Direction::Outgoing)
    }

    pub fn children_of(
        &self,
        root_id: git2::Oid,
    ) -> petgraph::graphmap::NeighborsDirected<'_, git2::Oid, petgraph::Directed> {
        self.graph
            .neighbors_directed(root_id, petgraph::Direction::Incoming)
    }

    pub fn primary_children_of(&self, root_id: git2::Oid) -> impl Iterator<Item = git2::Oid> + '_ {
        self.graph
            .edges_directed(root_id, petgraph::Direction::Incoming)
            .filter_map(|(child, _parent, weight)| (*weight == 0).then_some(child))
    }

    pub fn ancestors_of(&self, root_id: git2::Oid) -> AncestorsIter {
        let cursor = AncestorsCursor::new(self, root_id);
        AncestorsIter {
            cursor,
            graph: self,
        }
    }

    pub fn descendants(&self) -> DescendantsIter {
        self.descendants_of(self.root_id)
    }

    pub fn descendants_of(&self, root_id: git2::Oid) -> DescendantsIter {
        let cursor = DescendantsCursor::new(self, root_id);
        DescendantsIter {
            cursor,
            graph: self,
        }
    }

    pub fn commit_get<R: Resource>(&self, id: git2::Oid) -> Option<&R> {
        let commit = self.commits.get(&id)?;
        let boxed_resource = commit.get(&AnyId::of::<R>())?;
        let resource = boxed_resource.as_ref::<R>();
        Some(resource)
    }

    pub fn commit_get_mut<R: Resource>(&mut self, id: git2::Oid) -> Option<&mut R> {
        let commit = self.commits.get_mut(&id)?;
        let boxed_resource = commit.get_mut(&AnyId::of::<R>())?;
        let resource = boxed_resource.as_mut::<R>();
        Some(resource)
    }

    pub fn commit_set<R: Into<BoxedEntry>>(&mut self, id: git2::Oid, r: R) -> bool {
        let BoxedEntry { id: key, value } = r.into();
        self.commits
            .entry(id)
            .or_default()
            .insert(key, value)
            .is_some()
    }
}

#[derive(Debug)]
pub struct Node {
    id: git2::Oid,
    commit: Option<BTreeMap<AnyId, BoxedResource>>,
    branches: Option<Vec<Branch>>,
}

impl Node {
    pub fn new(id: git2::Oid) -> Self {
        Self {
            id,
            commit: None,
            branches: None,
        }
    }
}

#[derive(Debug)]
pub struct AncestorsIter<'g> {
    cursor: AncestorsCursor,
    graph: &'g Graph,
}

impl<'g> AncestorsIter<'g> {
    pub fn into_cursor(self) -> AncestorsCursor {
        self.cursor
    }
}

impl<'g> Iterator for AncestorsIter<'g> {
    type Item = git2::Oid;

    fn next(&mut self) -> Option<Self::Item> {
        self.cursor.next(self.graph)
    }
}

#[derive(Debug)]
pub struct AncestorsCursor {
    node_queue: VecDeque<git2::Oid>,
    primary_parents: bool,
    prior: Option<git2::Oid>,
    seen: std::collections::HashSet<git2::Oid>,
}

impl AncestorsCursor {
    fn new(graph: &Graph, root_id: git2::Oid) -> Self {
        let mut node_queue = VecDeque::new();
        if graph.graph.contains_node(root_id) {
            node_queue.push_back(root_id);
        }
        Self {
            node_queue,
            primary_parents: false,
            prior: None,
            seen: Default::default(),
        }
    }

    pub fn primary_parents(mut self, yes: bool) -> Self {
        self.primary_parents = yes;
        self
    }
}

impl AncestorsCursor {
    pub fn next(&mut self, graph: &Graph) -> Option<git2::Oid> {
        if let Some(prior) = self.prior {
            if self.primary_parents {
                // Single path, no chance for duplicating paths
                self.node_queue.extend(graph.primary_parent_of(prior));
            } else {
                for parent_id in graph.parents_of(prior) {
                    if self.seen.insert(parent_id) {
                        self.node_queue.push_back(parent_id);
                    }
                }
            }
        }
        let next = self.node_queue.pop_front()?;
        self.prior = Some(next);
        Some(next)
    }

    pub fn stop(&mut self) {
        self.prior = None;
    }
}

#[derive(Debug)]
pub struct DescendantsIter<'g> {
    cursor: DescendantsCursor,
    graph: &'g Graph,
}

impl<'g> DescendantsIter<'g> {
    pub fn into_cursor(self) -> DescendantsCursor {
        self.cursor
    }
}

impl<'g> Iterator for DescendantsIter<'g> {
    type Item = git2::Oid;

    fn next(&mut self) -> Option<Self::Item> {
        self.cursor.next(self.graph)
    }
}

#[derive(Debug)]
pub struct DescendantsCursor {
    node_queue: VecDeque<git2::Oid>,
    prior: Option<git2::Oid>,
}

impl DescendantsCursor {
    fn new(graph: &Graph, root_id: git2::Oid) -> Self {
        let mut node_queue = VecDeque::new();
        if graph.graph.contains_node(root_id) {
            node_queue.push_back(root_id);
        }
        Self {
            node_queue,
            prior: None,
        }
    }
}

impl DescendantsCursor {
    pub fn next(&mut self, graph: &Graph) -> Option<git2::Oid> {
        if let Some(prior) = self.prior {
            self.node_queue.extend(graph.primary_children_of(prior));
        }
        let next = self.node_queue.pop_front()?;
        self.prior = Some(next);
        Some(next)
    }

    pub fn stop(&mut self) {
        self.prior = None;
    }
}
