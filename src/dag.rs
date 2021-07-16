use itertools::Itertools;

pub fn graph<'r>(
    repo: &'r git2::Repository,
    base_oid: git2::Oid,
    head_oid: git2::Oid,
    mut graph_branches: crate::branches::Branches<'r>,
) -> Result<Node<'r>, git2::Error> {
    let mut root = Node::populate(repo, base_oid, head_oid, &mut graph_branches)?;

    if !graph_branches.is_empty() {
        let branch_oids: Vec<_> = graph_branches.oids().collect();
        for branch_oid in branch_oids {
            let branches = graph_branches.get(branch_oid).unwrap();
            let branch_name = branches
                .first()
                .unwrap()
                .name()?
                .unwrap_or(crate::git::NO_BRANCH)
                .to_owned();
            match Node::populate(repo, base_oid, branch_oid, &mut graph_branches) {
                Ok(branch_root) => {
                    root.merge(branch_root);
                }
                Err(err) => {
                    log::error!("Unhandled branch {}: {}", branch_name, err);
                }
            }
        }
    }

    if !graph_branches.is_empty() {
        let unused_branches = graph_branches
            .iter()
            .flat_map(|(_, branches)| branches)
            .filter_map(|branch| branch.name().ok().flatten())
            .join(", ");
        log::error!("Unhandled branches: {}", unused_branches);
    }

    Ok(root)
}

pub struct Node<'r> {
    pub local_commit: git2::Commit<'r>,
    pub branches: Vec<git2::Branch<'r>>,
    pub children: Vec<Vec<Node<'r>>>,
    pub action: crate::actions::Action,
}

impl<'r> Node<'r> {
    fn populate(
        repo: &'r git2::Repository,
        base_oid: git2::Oid,
        head_oid: git2::Oid,
        branches: &mut crate::branches::Branches<'r>,
    ) -> Result<Self, git2::Error> {
        if let Some(head_branches) = branches.get(head_oid) {
            let head_name = head_branches
                .first()
                .unwrap()
                .name()?
                .unwrap_or(crate::git::NO_BRANCH);
            log::trace!("Populating data for {}", head_name);
        } else {
            log::trace!("Populating data for {}", head_oid);
        }
        let merge_base_oid = repo.merge_base(base_oid, head_oid)?;
        if merge_base_oid != base_oid {
            return Err(git2::Error::new(
                git2::ErrorCode::NotFound,
                git2::ErrorClass::Reference,
                "HEAD must be a descendant of base",
            ));
        }
        let base_commit = repo.find_commit(base_oid)?;

        let mut root = Node::from_commit(base_commit).with_branches(branches);

        let mut children: Vec<_> = crate::git::commits_from(&repo, head_oid)?
            .take_while(|commit| commit.id() != merge_base_oid)
            .map(|commit| Node::from_commit(commit).with_branches(branches))
            .collect();
        children.reverse();
        root.children.push(children);

        Ok(root)
    }

    fn from_commit(local_commit: git2::Commit<'r>) -> Self {
        let branches = Vec::new();
        let children = Vec::new();
        Self {
            local_commit,
            branches,
            children,
            action: crate::actions::Action::Pick,
        }
    }

    fn with_branches(mut self, possible_branches: &mut crate::branches::Branches<'r>) -> Self {
        if let Some(branches) = possible_branches.remove(self.local_commit.id()) {
            self.branches = branches;
        }
        self
    }

    fn merge(&mut self, other: Self) {
        if self.local_commit.id() != other.local_commit.id() {
            return;
        }
        let other_children = other.children;
        for mut other_children_branch in other_children {
            assert!(!other_children_branch.is_empty());
            for mut self_children_branch in self.children.iter_mut() {
                merge_nodes(&mut self_children_branch, &mut other_children_branch);
                if other_children_branch.is_empty() {
                    break;
                }
            }
            if !other_children_branch.is_empty() {
                self.children.push(other_children_branch);
            }
        }
    }
}

/// If a merge occurs, `rhs_nodes` will be empty
fn merge_nodes<'r>(lhs_nodes: &mut Vec<Node<'r>>, rhs_nodes: &mut Vec<Node<'r>>) {
    assert!(
        !lhs_nodes.is_empty(),
        "to exist, there has to be at least one node"
    );
    assert!(
        !rhs_nodes.is_empty(),
        "to exist, there has to be at least one node"
    );

    for (lhs, rhs) in lhs_nodes.iter_mut().zip(rhs_nodes.iter_mut()) {
        if lhs.local_commit.id() != rhs.local_commit.id() {
            break;
        }
        let mut branches = Vec::new();
        std::mem::swap(&mut rhs.branches, &mut branches);
        lhs.branches.extend(branches);
    }

    let index = rhs_nodes
        .iter()
        .zip_longest(lhs_nodes.iter())
        .enumerate()
        .find(|(_, zipped)| match zipped {
            itertools::EitherOrBoth::Both(lhs, rhs) => {
                lhs.local_commit.id() != rhs.local_commit.id()
            }
            _ => true,
        })
        .map(|(index, zipped)| {
            let zipped = zipped.map_any(|_| (), |_| ());
            (index, zipped)
        });

    match index {
        Some((index, itertools::EitherOrBoth::Both(_, _)))
        | Some((index, itertools::EitherOrBoth::Right(_))) => {
            if index == 0 {
                // Not a good merge candidate, find another
            } else {
                let remaining = rhs_nodes.split_off(index);
                lhs_nodes[index - 1].children.push(remaining);
                rhs_nodes.clear();
            }
        }
        Some((_, itertools::EitherOrBoth::Left(_))) | None => {
            // lhs is a superset, so consider us done.
            rhs_nodes.clear();
        }
    }
}

impl<'r> std::fmt::Debug for Node<'r> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let branches: Vec<_> = self
            .branches
            .iter()
            .map(|b| {
                b.name()
                    .ok()
                    .flatten()
                    .unwrap_or(crate::git::NO_BRANCH)
                    .to_owned()
            })
            .collect();
        f.debug_struct("Node")
            .field("local_commit", &self.local_commit.id())
            .field("branches", &branches)
            .field("children", &self.children)
            .finish()
    }
}

pub fn protect_branches<'r>(
    root: &mut Node<'r>,
    repo: &'r git2::Repository,
    protected_branches: &crate::branches::Branches<'r>,
) -> Result<(), git2::Error> {
    // Assuming the root is the base.  The base is not guaranteed to be a protected brancch but
    // might be an ancestor of one.
    for protected_oid in protected_branches.oids() {
        if let Ok(merge_base_oid) = repo.merge_base(root.local_commit.id(), protected_oid) {
            if merge_base_oid == root.local_commit.id() {
                root.action = crate::actions::Action::Protected;
                break;
            }
        }
    }

    for children in root.children.iter_mut() {
        protect_branches_internal(children, repo, protected_branches)?;
    }

    Ok(())
}

fn protect_branches_internal<'r>(
    nodes: &mut Vec<Node<'r>>,
    repo: &'r git2::Repository,
    protected_branches: &crate::branches::Branches<'r>,
) -> Result<bool, git2::Error> {
    let mut descendant_protected = false;
    for node in nodes.iter_mut().rev() {
        let mut children_protected = false;
        for children in node.children.iter_mut() {
            let child_protected = protect_branches_internal(children, repo, protected_branches)?;
            children_protected |= child_protected;
        }
        let self_protected = protected_branches.contains_oid(node.local_commit.id());
        if descendant_protected || children_protected || self_protected {
            node.action = crate::actions::Action::Protected;
            descendant_protected = true;
        }
    }

    Ok(descendant_protected)
}
