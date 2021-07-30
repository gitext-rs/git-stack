use itertools::Itertools;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Node {
    pub local_commit: std::rc::Rc<crate::git::Commit>,
    pub branches: Vec<crate::git::Branch>,
    pub children: Vec<Vec<Node>>,
    pub action: crate::graph::Action,
}

impl Node {
    pub fn new(
        local_commit: std::rc::Rc<crate::git::Commit>,
        possible_branches: &mut crate::git::Branches,
    ) -> Self {
        let branches = possible_branches
            .remove(local_commit.id)
            .unwrap_or_else(Vec::new);
        let children = Vec::new();
        Self {
            local_commit,
            branches,
            children,
            action: crate::graph::Action::Pick,
        }
    }

    pub fn from_branches(
        repo: &dyn crate::git::Repo,
        mut branches: crate::git::Branches,
    ) -> eyre::Result<Self> {
        if branches.is_empty() {
            eyre::bail!("no branches to graph");
        }

        let mut branch_ids: Vec<_> = branches.oids().collect();
        branch_ids.sort_by_key(|id| &branches.get(*id).unwrap()[0].name);
        let branch_id = branch_ids.remove(0);
        let branch_commit = repo.find_commit(branch_id).unwrap();
        let mut root = Self::new(branch_commit, &mut branches);
        for branch_id in branch_ids {
            let branch_commit = repo.find_commit(branch_id).unwrap();
            root = root.insert(repo, branch_commit, &mut branches)?;
        }

        Ok(root)
    }

    pub fn insert(
        mut self,
        repo: &dyn crate::git::Repo,
        local_commit: std::rc::Rc<crate::git::Commit>,
        possible_branches: &mut crate::git::Branches,
    ) -> eyre::Result<Self> {
        let mut self_id = self.local_commit.id;
        let other_id = local_commit.id;
        let merge_base_id = repo
            .merge_base(self_id, other_id)
            .ok_or_else(|| eyre::eyre!("Could not find merge base"))?;

        if merge_base_id != self_id {
            let mut prefix = Node::populate(repo, merge_base_id, self_id, possible_branches)?;
            prefix.push(self);
            self = prefix;
            self_id = merge_base_id;
        }
        let other = Node::populate(repo, self_id, other_id, possible_branches)?;
        self.merge(other);

        Ok(self)
    }

    pub fn extend(
        mut self,
        repo: &dyn crate::git::Repo,
        mut branches: crate::git::Branches,
    ) -> eyre::Result<Self> {
        if !branches.is_empty() {
            let mut branch_ids: Vec<_> = branches.oids().collect();
            branch_ids.sort_by_key(|id| &branches.get(*id).unwrap()[0].name);
            for branch_id in branch_ids {
                let branch_commit = repo.find_commit(branch_id).unwrap();
                self = self.insert(repo, branch_commit, &mut branches)?;
            }
        }

        Ok(self)
    }

    fn populate(
        repo: &dyn crate::git::Repo,
        base_oid: git2::Oid,
        head_oid: git2::Oid,
        branches: &mut crate::git::Branches,
    ) -> Result<Self, git2::Error> {
        if let Some(head_branches) = branches.get(head_oid) {
            let head_name = head_branches.first().unwrap().name.as_str();
            log::trace!("Populating data for {}..{}", base_oid, head_name);
        } else {
            log::trace!("Populating data for {}..{}", base_oid, head_oid);
        }
        let merge_base_oid = repo.merge_base(base_oid, head_oid).ok_or_else(|| {
            git2::Error::new(
                git2::ErrorCode::NotFound,
                git2::ErrorClass::Reference,
                "Could not find merge base",
            )
        })?;
        if merge_base_oid != base_oid {
            return Err(git2::Error::new(
                git2::ErrorCode::NotFound,
                git2::ErrorClass::Reference,
                "HEAD must be a descendant of base",
            ));
        }
        let base_commit = repo.find_commit(base_oid).unwrap();

        let mut root = Node::new(base_commit, branches);

        let mut children: Vec<_> = repo
            .commits_from(head_oid)
            .take_while(|commit| commit.id != base_oid)
            .map(|commit| Node::new(commit, branches))
            .collect();
        children.reverse();
        if !children.is_empty() {
            root.children.push(children);
        }

        Ok(root)
    }

    fn push(&mut self, other: Self) {
        let other_oid = other.local_commit.id;
        if self.local_commit.id == other_oid {
            self.merge(other);
        } else if self.children.len() == 1 {
            let child = &mut self.children[0];
            for node in child.iter_mut() {
                if node.local_commit.id == other_oid {
                    node.merge(other);
                    return;
                }
            }
            unimplemented!("This case isn't needed yet");
        } else {
            unimplemented!("This case isn't needed yet");
        }
    }

    fn merge(&mut self, mut other: Self) {
        let mut branches = Vec::new();
        std::mem::swap(&mut other.branches, &mut branches);
        self.branches.extend(branches);

        merge_children(self, other);
    }
}

fn merge_children(lhs_node: &mut Node, rhs_node: Node) {
    assert_eq!(lhs_node.local_commit.id, rhs_node.local_commit.id);

    let rhs_node_children = rhs_node.children;
    for mut rhs_node_children_branch in rhs_node_children {
        assert!(!rhs_node_children_branch.is_empty());
        for mut lhs_node_children_branch in lhs_node.children.iter_mut() {
            merge_branch(&mut lhs_node_children_branch, &mut rhs_node_children_branch);
            if rhs_node_children_branch.is_empty() {
                break;
            }
        }
        if !rhs_node_children_branch.is_empty() {
            lhs_node.children.push(rhs_node_children_branch);
        }
    }
}

/// If a merge occurs, `rhs_nodes` will be empty
fn merge_branch(lhs_nodes: &mut Vec<Node>, rhs_nodes: &mut Vec<Node>) {
    assert!(
        !lhs_nodes.is_empty(),
        "to exist, there has to be at least one node"
    );
    assert!(
        !rhs_nodes.is_empty(),
        "to exist, there has to be at least one node"
    );

    for (lhs, rhs) in lhs_nodes.iter_mut().zip(rhs_nodes.iter_mut()) {
        if lhs.local_commit.id != rhs.local_commit.id {
            break;
        }
        let mut branches = Vec::new();
        std::mem::swap(&mut rhs.branches, &mut branches);
        lhs.branches.extend(branches);
    }

    let index = lhs_nodes
        .iter()
        .zip_longest(rhs_nodes.iter())
        .enumerate()
        .find(|(_, zipped)| match zipped {
            itertools::EitherOrBoth::Both(lhs, rhs) => lhs.local_commit.id != rhs.local_commit.id,
            _ => true,
        })
        .map(|(index, zipped)| {
            let zipped = zipped.map_any(|_| (), |_| ());
            (index, zipped)
        });

    match index {
        Some((index, itertools::EitherOrBoth::Both(_, _))) => {
            if index == 0 {
                // Not a good merge candidate, find another
            } else {
                let remaining = rhs_nodes.split_off(index);
                let mut fake_rhs_node = rhs_nodes.pop().expect("if should catch this");
                assert!(fake_rhs_node.children.is_empty(), "assuming rhs is linear");
                fake_rhs_node.children.push(remaining);
                merge_children(&mut lhs_nodes[index - 1], fake_rhs_node);
                rhs_nodes.clear();
            }
        }
        Some((index, itertools::EitherOrBoth::Right(_))) => {
            // rhs is a superset, so we can append it to lhs
            let remaining = rhs_nodes.split_off(index);
            lhs_nodes.extend(remaining);
            rhs_nodes.clear();
        }
        Some((_, itertools::EitherOrBoth::Left(_))) | None => {
            // lhs is a superset, so consider us done.
            rhs_nodes.clear();
        }
    }
}
