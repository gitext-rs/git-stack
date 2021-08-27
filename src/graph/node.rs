use itertools::Itertools;

pub type Stack = vec1::Vec1<Node>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Node {
    pub local_commit: std::rc::Rc<crate::git::Commit>,
    pub branches: Vec<crate::git::Branch>,
    pub stacks: Vec<Stack>,
    pub action: crate::graph::Action,
    pub pushable: bool,
}

impl Node {
    pub fn new(
        local_commit: std::rc::Rc<crate::git::Commit>,
        possible_branches: &mut crate::git::Branches,
    ) -> Self {
        let branches = possible_branches
            .remove(local_commit.id)
            .unwrap_or_else(Vec::new);
        let stacks = Vec::new();
        Self {
            local_commit,
            branches,
            stacks,
            action: crate::graph::Action::Pick,
            pushable: false,
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
            root = root.insert_commit(repo, branch_commit, &mut branches)?;
        }

        Ok(root)
    }

    pub fn insert_commit(
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
            let pushed = prefix.extend(self);
            assert!(pushed);
            self = prefix;
            self_id = merge_base_id;
        }
        let other = Node::populate(repo, self_id, other_id, possible_branches)?;
        self.merge(other);

        Ok(self)
    }

    pub fn extend_branches(
        mut self,
        repo: &dyn crate::git::Repo,
        mut branches: crate::git::Branches,
    ) -> eyre::Result<Self> {
        if !branches.is_empty() {
            let mut branch_ids: Vec<_> = branches.oids().collect();
            branch_ids.sort_by_key(|id| &branches.get(*id).unwrap()[0].name);
            for branch_id in branch_ids {
                let branch_commit = repo.find_commit(branch_id).unwrap();
                self = self.insert_commit(repo, branch_commit, &mut branches)?;
            }
        }

        Ok(self)
    }

    #[must_use]
    pub fn extend(&mut self, other: Self) -> bool {
        let base = self.find_commit_mut(other.local_commit.id);
        if let Some(base) = base {
            base.merge(other);
            true
        } else {
            false
        }
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

        let mut stack: Vec<_> = repo
            .commits_from(head_oid)
            .take_while(|commit| commit.id != base_oid)
            .map(|commit| Node::new(commit, branches))
            .collect();
        stack.reverse();
        if let Ok(mut stack) = Stack::try_from_vec(stack) {
            crate::graph::ops::delinearize_stack(&mut stack);
            root.stacks.push(stack);
        }

        Ok(root)
    }

    pub(crate) fn find_commit_mut(&mut self, id: git2::Oid) -> Option<&mut Node> {
        if self.local_commit.id == id {
            return Some(self);
        }

        for stack in self.stacks.iter_mut() {
            for node in stack.iter_mut() {
                if let Some(found) = node.find_commit_mut(id) {
                    return Some(found);
                }
            }
        }

        None
    }

    fn merge(&mut self, mut other: Self) {
        assert_eq!(self.local_commit.id, other.local_commit.id);
        let mut branches = Vec::new();
        std::mem::swap(&mut other.branches, &mut branches);
        self.branches.extend(branches);

        merge_stacks(self, other);
    }
}

fn merge_stacks(lhs_node: &mut Node, rhs_node: Node) {
    assert_eq!(lhs_node.local_commit.id, rhs_node.local_commit.id);

    let rhs_node_stacks = rhs_node.stacks;
    for rhs_node_stack in rhs_node_stacks {
        // Allow emptu-state to know if merge happened
        let mut rhs_node_stack = rhs_node_stack.into_vec();
        for mut lhs_node_stack in lhs_node.stacks.iter_mut() {
            merge_stack(&mut lhs_node_stack, &mut rhs_node_stack);
            if rhs_node_stack.is_empty() {
                break;
            }
        }
        if let Ok(rhs_node_stack) = Stack::try_from_vec(rhs_node_stack) {
            // No merge, add to stacks
            lhs_node.stacks.push(rhs_node_stack);
        }
    }
}

/// If a merge occurs, `rhs_nodes` will be empty
fn merge_stack(lhs_nodes: &mut Stack, rhs_nodes: &mut Vec<Node>) {
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
                let remaining = Stack::try_from_vec(remaining).unwrap();
                let mut fake_rhs_node = rhs_nodes.pop().expect("if should catch this");
                fake_rhs_node.stacks.push(remaining);
                merge_stacks(&mut lhs_nodes[index - 1], fake_rhs_node);
                rhs_nodes.clear();
            }
        }
        Some((index, itertools::EitherOrBoth::Right(_))) => {
            // rhs is a superset, so we can add it to lhs's stacks
            let remaining = rhs_nodes.split_off(index);
            let remaining = Stack::try_from_vec(remaining).unwrap();
            lhs_nodes.last_mut().stacks.push(remaining);
            rhs_nodes.clear();
        }
        Some((_, itertools::EitherOrBoth::Left(_))) | None => {
            // lhs is a superset, so consider us done.
            rhs_nodes.clear();
        }
    }
}
