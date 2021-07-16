use itertools::Itertools;

pub fn graph<'r>(
    repo: &'r git2::Repository,
    base_branch: Option<git2::Branch<'r>>,
    head_branch: git2::Branch<'r>,
    dependents: bool,
    protected_branches: &ignore::gitignore::Gitignore,
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

    let protected_oids: std::collections::HashMap<_, _> = possible_branches
        .iter()
        .filter_map(|(oid, branches)| {
            let branch_name = branches
                .iter()
                .filter_map(|b| {
                    let branch_name = b
                        .name()
                        .ok()
                        .flatten()
                        .unwrap_or(crate::git::NO_BRANCH)
                        .to_owned();
                    let branch_match = protected_branches.matched(&branch_name, false);
                    if branch_match.is_ignore() {
                        log::trace!("Branch {} is protected", branch_name);
                        Some(branch_name)
                    } else {
                        None
                    }
                })
                .next();
            branch_name.map(|branch_name| (*oid, branch_name))
        })
        .collect();

    let base_branch = base_branch.map(Ok).unwrap_or_else(|| {
        let head_name = head_branch.name()?.unwrap_or(crate::git::NO_BRANCH);
        let head_oid = head_branch.get().target().ok_or_else(|| {
            git2::Error::new(
                git2::ErrorCode::NotFound,
                git2::ErrorClass::Reference,
                format!("could not resolve HEAD ({})", head_name),
            )
        })?;
        let protected_base_oids: std::collections::HashMap<_, _> = protected_oids
            .iter()
            .filter_map(|(oid, name)| {
                repo.merge_base(head_oid, *oid)
                    .ok()
                    .map(|base_oid| (base_oid, name))
            })
            .collect();
        crate::git::commits_from(&repo, head_oid)?
            .filter_map(|commit| {
                protected_base_oids.get(&commit.id()).map(|branch_name| {
                    log::debug!("Base is recent protected branch {}", branch_name);
                    repo.find_branch(branch_name, git2::BranchType::Local)
                })
            })
            .next()
            .unwrap_or_else(|| {
                Err(git2::Error::new(
                    git2::ErrorCode::NotFound,
                    git2::ErrorClass::Reference,
                    "could not find a protected branch to use as a base",
                ))
            })
    })?;

    let mut root = Node::populate(
        repo,
        &base_branch,
        vec![head_branch],
        &mut possible_branches,
    )?;

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
        base_branch: &git2::Branch<'r>,
        head_branch: Vec<git2::Branch<'r>>,
        branches: &mut std::collections::BTreeMap<git2::Oid, Vec<git2::Branch<'r>>>,
    ) -> Result<Self, git2::Error> {
        let base_name = base_branch.name()?.unwrap_or(crate::git::NO_BRANCH);
        let base_oid = base_branch.get().target().ok_or_else(|| {
            git2::Error::new(
                git2::ErrorCode::NotFound,
                git2::ErrorClass::Reference,
                format!("could not resolve {}", base_name),
            )
        })?;

        let head_name = head_branch
            .first()
            .unwrap()
            .name()?
            .unwrap_or(crate::git::NO_BRANCH);
        log::trace!("Populating data for {}..{}", base_name, head_name);
        let head_oid = head_branch.first().unwrap().get().target().ok_or_else(|| {
            git2::Error::new(
                git2::ErrorCode::NotFound,
                git2::ErrorClass::Reference,
                format!("could not resolve HEAD ({})", head_name),
            )
        })?;
        let merge_base_oid = repo.merge_base(base_oid, head_oid)?;
        let merge_base_commit = repo.find_commit(merge_base_oid)?;

        let mut root = Node::from_commit(merge_base_commit);
        root.branches = branches.remove(&base_oid).ok_or_else(|| {
            git2::Error::new(
                git2::ErrorCode::NotFound,
                git2::ErrorClass::Reference,
                format!("could not local branch {}", base_name),
            )
        })?;

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
