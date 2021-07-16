use itertools::Itertools;

pub fn graph<'r>(
    repo: &'r git2::Repository,
    base_oid: git2::Oid,
    head_branch: git2::Branch<'r>,
    dependents: bool,
    all: bool,
) -> Result<Node<'r>, git2::Error> {
    log::trace!("Loading branches");
    let mut possible_branches = std::collections::BTreeMap::new();
    for branch in repo.branches(Some(git2::BranchType::Local))? {
        let (branch, _) = branch?;
        let branch_name = branch.name()?.unwrap_or(crate::git::NO_BRANCH);
        if let Some(branch_oid) = branch.get().target() {
            log::trace!("Resolved branch {} as {}", branch_name, branch_oid);
            possible_branches
                .entry(branch_oid)
                .or_insert_with(|| Vec::new())
                .push(branch);
        } else {
            log::debug!("Could not resolve branch {}", branch_name);
        }
    }

    let head_oid = head_branch.get().target().ok_or_else(|| {
        git2::Error::new(
            git2::ErrorCode::NotFound,
            git2::ErrorClass::Reference,
            format!("could not resolve HEAD"),
        )
    })?;

    let mut root = Node::populate(repo, base_oid, head_branch, &mut possible_branches)?;

    if dependents {
        let possible_branch_oids: Vec<_> = possible_branches.keys().cloned().collect();
        for branch_oid in possible_branch_oids {
            let branch_head_base = match repo.merge_base(branch_oid, head_oid) {
                Ok(branch_head_base) => branch_head_base,
                Err(err) => {
                    log::trace!("Branch {} looks irrelevant: {}", branch_oid, err);
                    continue;
                }
            };
            if branch_head_base == base_oid {
                log::trace!("Branch {} looks irrelevant (shared base)", branch_oid);
                continue;
            }

            let branch_base_base = match repo.merge_base(branch_oid, base_oid) {
                Ok(branch_base_base) => branch_base_base,
                Err(err) => {
                    log::trace!("Branch {} looks irrelevant: {}", branch_oid, err);
                    continue;
                }
            };
            if branch_base_base != base_oid {
                log::trace!("Branch {} looks irrelevant (too early)", branch_oid);
                continue;
            }

            let branches = possible_branches.get(&branch_oid).unwrap();
            let branch_name = branches
                .first()
                .unwrap()
                .name()?
                .unwrap_or(crate::git::NO_BRANCH)
                .to_owned();
            let branch = repo
                .find_branch(&branch_name, git2::BranchType::Local)
                .unwrap();
            match Node::populate(repo, base_oid, branch, &mut possible_branches) {
                Ok(branch_root) => {
                    root.merge(branch_root);
                }
                Err(err) => {
                    log::debug!("Branch {} looks irrelevant: {}", branch_name, err);
                }
            }
        }
    }

    let unused_branches = possible_branches
        .iter()
        .flat_map(|(_, branches)| branches)
        .filter_map(|branch| branch.name().ok().flatten())
        .join(", ");
    log::debug!("Unaffected branches: {}", unused_branches);

    Ok(root)
}

pub struct Node<'r> {
    pub local_commit: git2::Commit<'r>,
    pub branches: Vec<git2::Branch<'r>>,
    pub children: Vec<Vec<Node<'r>>>,
}

impl<'r> Node<'r> {
    fn populate(
        repo: &'r git2::Repository,
        base_oid: git2::Oid,
        head_branch: git2::Branch<'r>,
        branches: &mut std::collections::BTreeMap<git2::Oid, Vec<git2::Branch<'r>>>,
    ) -> Result<Self, git2::Error> {
        let head_name = head_branch.name()?.unwrap_or(crate::git::NO_BRANCH);
        log::trace!("Populating data for {}", head_name);
        let head_oid = head_branch.get().target().ok_or_else(|| {
            git2::Error::new(
                git2::ErrorCode::NotFound,
                git2::ErrorClass::Reference,
                format!("could not resolve HEAD ({})", head_name),
            )
        })?;
        let merge_base_oid = repo.merge_base(base_oid, head_oid)?;
        let merge_base_commit = repo.find_commit(merge_base_oid)?;

        let mut root = Node::from_commit(merge_base_commit);
        root.branches = branches.remove(&base_oid).unwrap_or_else(|| Vec::new());

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
        }
    }

    fn with_branches(
        mut self,
        possible_branches: &mut std::collections::BTreeMap<git2::Oid, Vec<git2::Branch<'r>>>,
    ) -> Self {
        if let Some(branches) = possible_branches.remove(&self.local_commit.id()) {
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
