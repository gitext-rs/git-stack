mod actions;
mod node;
mod ops;

pub use actions::*;
pub use node::*;
pub use ops::*;

use std::collections::btree_map::Entry;
use std::collections::BTreeMap;

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
        repo: &dyn crate::git::Repo,
        mut branches: crate::git::Branches,
    ) -> eyre::Result<Self> {
        if branches.is_empty() {
            eyre::bail!("no branches to graph");
        }

        let mut branch_ids: Vec<_> = branches.oids().collect();
        // Be more reproducible to make it easier to debug
        branch_ids.sort_by_key(|id| &branches.get(*id).unwrap()[0].name);

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

    pub fn insert(&mut self, repo: &dyn crate::git::Repo, node: Node) -> eyre::Result<()> {
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

    pub fn extend(&mut self, repo: &dyn crate::git::Repo, other: Self) -> eyre::Result<()> {
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

    fn populate(
        &mut self,
        repo: &dyn crate::git::Repo,
        base_oid: git2::Oid,
        head_oid: git2::Oid,
        default_action: crate::graph::Action,
    ) -> Result<(), git2::Error> {
        log::trace!("Populating data for {}..{}", base_oid, head_oid);
        debug_assert_eq!(
            repo.merge_base(base_oid, head_oid),
            Some(base_oid),
            "HEAD must be a descendant of base"
        );

        let mut child_id = None;
        for commit in repo.commits_from(head_oid) {
            match self.nodes.entry(commit.id) {
                Entry::Occupied(mut o) => {
                    let current = o.get_mut();
                    if let Some(child_id) = child_id {
                        current.children.insert(child_id);
                        // Tapped into previous entries, don't bother going further
                        break;
                    }

                    child_id = Some(current.commit.id);
                }
                Entry::Vacant(v) => {
                    let current = v.insert(Node::new(commit));
                    current.action = default_action;
                    if let Some(child_id) = child_id {
                        current.children.insert(child_id);
                    }

                    if current.commit.id == base_oid {
                        break;
                    }

                    child_id = Some(current.commit.id);
                }
            }
        }

        Ok(())
    }
}
