pub fn graph<'r>(
    repo: &'r git2::Repository,
    base_branch: git2::Branch<'r>,
    head_branch: git2::Branch<'r>,
    dependents: bool,
) -> Result<Node<'r>, git2::Error> {
    log::debug!("Loading branches");
    let mut possible_branches = std::collections::BTreeMap::new();
    for branch in repo.branches(Some(git2::BranchType::Local))? {
        let (branch, _) = branch?;
        let branch_name = branch.name()?.unwrap_or("<>");
        if let Some(branch_oid) = branch.get().target() {
            log::debug!("Resolved branch {} as {}", branch_name, branch_oid);
            possible_branches
                .entry(branch_oid)
                .or_insert_with(|| Vec::new())
                .push(branch);
        } else {
            log::debug!("Could not resolve branch {}", branch_name);
        }
    }

    let mut root = Node::populate(
        repo,
        &base_branch,
        vec![head_branch],
        &mut possible_branches,
    )?;

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
        let base_name = base_branch.name()?.unwrap_or("<>");
        log::debug!("Populating data for {}", base_name);
        let base_oid = base_branch.get().target().ok_or_else(|| {
            git2::Error::new(
                git2::ErrorCode::NotFound,
                git2::ErrorClass::Reference,
                format!("could not resolve {}", base_name),
            )
        })?;

        let head_name = head_branch.first().unwrap().name()?.unwrap_or("<>");
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
        f.debug_struct("Node")
            .field("local_commit", &self.local_commit.id())
            .field("children", &self.children)
            .finish()
    }
}
