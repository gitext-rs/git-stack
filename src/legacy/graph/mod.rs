mod actions;
mod node;
mod ops;

pub use actions::*;
pub use node::*;
pub use ops::*;

use std::collections::BTreeMap;
use std::collections::VecDeque;
use std::collections::btree_map::Entry;

#[derive(Clone, Debug)]
pub struct Graph {
    root_id: git2::Oid,
    nodes: BTreeMap<git2::Oid, Node>,
}

impl Graph {
    pub fn new(node: Node) -> Self {
        let root_id = node.commit.id;
        let mut nodes = BTreeMap::new();
        nodes.insert(root_id, node);
        Self { root_id, nodes }
    }

    pub fn from_branches(
        repo: &dyn crate::legacy::git::Repo,
        mut branches: crate::legacy::git::Branches,
    ) -> eyre::Result<Self> {
        if branches.is_empty() {
            eyre::bail!("No branches to graph");
        }

        let mut branch_ids: Vec<_> = branches.oids().collect();
        // Be more reproducible to make it easier to debug
        branch_ids.sort_by_key(|id| {
            let first_branch = &branches.get(*id).unwrap()[0];
            (first_branch.remote.as_deref(), first_branch.name.as_str())
        });

        let branch_id = branch_ids.remove(0);
        let branch_commit = repo.find_commit(branch_id).unwrap();
        let root = Node::new(branch_commit).with_branches(&mut branches);
        let mut graph = Self::new(root);

        for branch_id in branch_ids {
            let branch_commit = repo.find_commit(branch_id).unwrap();
            let node = Node::new(branch_commit).with_branches(&mut branches);
            graph.insert(repo, node)?;
        }

        Ok(graph)
    }

    pub fn insert(&mut self, repo: &dyn crate::legacy::git::Repo, node: Node) -> eyre::Result<()> {
        let node_id = node.commit.id;
        if let Some(local) = self.get_mut(node_id) {
            local.update(node);
        } else {
            let merge_base_id = repo
                .merge_base(self.root_id, node_id)
                .ok_or_else(|| eyre::eyre!("Could not find merge base"))?;
            if merge_base_id != self.root_id {
                let root_action = self.root().action;
                self.populate(repo, merge_base_id, self.root_id, root_action)?;
                self.root_id = merge_base_id;
            }
            if merge_base_id != node_id {
                self.populate(repo, merge_base_id, node_id, node.action)?;
            }
            self.get_mut(node_id)
                .expect("populate added node_id")
                .update(node);
        }
        Ok(())
    }

    pub fn extend(&mut self, repo: &dyn crate::legacy::git::Repo, other: Self) -> eyre::Result<()> {
        if self.get(other.root_id).is_none() {
            self.insert(repo, other.root().clone())?;
        }
        for node in other.nodes.into_values() {
            match self.nodes.entry(node.commit.id) {
                Entry::Occupied(mut o) => o.get_mut().update(node),
                Entry::Vacant(v) => {
                    v.insert(node);
                }
            }
        }

        Ok(())
    }

    pub fn remove_child(&mut self, parent_id: git2::Oid, child_id: git2::Oid) -> Option<Self> {
        let parent = self.get_mut(parent_id)?;
        if !parent.children.remove(&child_id) {
            return None;
        }

        let child = self.nodes.remove(&child_id)?;
        let mut node_queue = VecDeque::new();
        node_queue.extend(child.children.iter().copied());
        let mut removed = Self::new(child);
        while let Some(current_id) = node_queue.pop_front() {
            let current = self.nodes.remove(&current_id).expect("all children exist");
            node_queue.extend(current.children.iter().copied());
            removed.nodes.insert(current_id, current);
        }

        Some(removed)
    }

    pub fn root(&self) -> &Node {
        self.nodes.get(&self.root_id).expect("root always exists")
    }

    pub fn root_id(&self) -> git2::Oid {
        self.root_id
    }

    pub fn get(&self, id: git2::Oid) -> Option<&Node> {
        self.nodes.get(&id)
    }

    pub fn get_mut(&mut self, id: git2::Oid) -> Option<&mut Node> {
        self.nodes.get_mut(&id)
    }

    pub fn breadth_first_iter(&self) -> BreadthFirstIter<'_> {
        BreadthFirstIter::new(self, self.root_id())
    }

    fn populate(
        &mut self,
        repo: &dyn crate::legacy::git::Repo,
        base_oid: git2::Oid,
        head_oid: git2::Oid,
        default_action: Action,
    ) -> Result<(), git2::Error> {
        log::trace!("Populating data for {}..{}", base_oid, head_oid);
        debug_assert_eq!(
            repo.merge_base(base_oid, head_oid),
            Some(base_oid),
            "HEAD must be a descendant of base"
        );

        let mut child_id = None;
        for commit_id in crate::legacy::git::commit_range(repo, head_oid..=base_oid)? {
            match self.nodes.entry(commit_id) {
                Entry::Occupied(mut o) => {
                    let current = o.get_mut();
                    if let Some(child_id) = child_id {
                        current.children.insert(child_id);
                        // Tapped into previous entries, don't bother going further
                        break;
                    }
                    // `head_oid` might already exist but none of its parents, so keep going
                    child_id = Some(current.commit.id);
                }
                Entry::Vacant(v) => {
                    let commit = repo
                        .find_commit(commit_id)
                        .expect("commit_range always returns valid ids");
                    let current = v.insert(Node::new(commit));
                    current.action = default_action;
                    if let Some(child_id) = child_id {
                        current.children.insert(child_id);
                    }

                    child_id = Some(current.commit.id);
                }
            }
        }

        Ok(())
    }
}

pub struct BreadthFirstIter<'g> {
    graph: &'g Graph,
    node_queue: VecDeque<git2::Oid>,
}

impl<'g> BreadthFirstIter<'g> {
    pub fn new(graph: &'g Graph, root_id: git2::Oid) -> Self {
        let mut node_queue = VecDeque::new();
        if graph.nodes.contains_key(&root_id) {
            node_queue.push_back(root_id);
        }
        Self { graph, node_queue }
    }
}

impl<'g> Iterator for BreadthFirstIter<'g> {
    type Item = &'g Node;
    fn next(&mut self) -> Option<Self::Item> {
        let next_id = self.node_queue.pop_front()?;
        let next = self.graph.get(next_id)?;
        self.node_queue.extend(next.children.iter().copied());
        Some(next)
    }
}
