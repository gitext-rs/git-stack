use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Node {
    pub commit: std::rc::Rc<crate::git::Commit>,
    pub branches: Vec<crate::git::Branch>,
    pub action: crate::graph::Action,
    pub pushable: bool,
    pub children: BTreeMap<git2::Oid, Node>,
}

impl Node {
    pub fn new(commit: std::rc::Rc<crate::git::Commit>) -> Self {
        let branches = Vec::new();
        let children = BTreeMap::new();
        Self {
            commit,
            branches,
            action: crate::graph::Action::Pick,
            pushable: false,
            children,
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
        let mut root = Self::new(branch_commit).with_branches(&mut branches);
        for branch_id in branch_ids {
            let branch_commit = repo.find_commit(branch_id).unwrap();
            let node = Node::new(branch_commit).with_branches(&mut branches);
            root = root.extend(repo, node)?;
        }

        Ok(root)
    }

    pub fn extend(mut self, repo: &dyn crate::git::Repo, mut other: Self) -> eyre::Result<Self> {
        if let Some(node) = self.find_commit_mut(other.commit.id) {
            node.merge(other)
        } else {
            let merge_base_id = repo
                .merge_base(self.commit.id, other.commit.id)
                .ok_or_else(|| eyre::eyre!("Could not find merge base"))?;
            if merge_base_id != self.commit.id {
                let prefix = Node::populate(repo, merge_base_id, self.commit.id, self.action)?;
                self = prefix.extend(repo, self)?;
            }
            if merge_base_id != other.commit.id {
                let prefix = Node::populate(repo, merge_base_id, other.commit.id, other.action)?;
                other = prefix.extend(repo, other)?;
            }
            self.merge(other);
        }

        Ok(self)
    }

    fn with_branches(mut self, possible_branches: &mut crate::git::Branches) -> Self {
        self.branches = possible_branches
            .remove(self.commit.id)
            .unwrap_or_else(Vec::new);
        self
    }

    fn populate(
        repo: &dyn crate::git::Repo,
        base_oid: git2::Oid,
        head_oid: git2::Oid,
        default_action: crate::graph::Action,
    ) -> Result<Self, git2::Error> {
        log::trace!("Populating data for {}..{}", base_oid, head_oid);
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

        let head_commit = repo.find_commit(head_oid).unwrap();
        let mut root = Node::new(head_commit);
        root.action = default_action;

        let mut commits = repo.commits_from(head_oid);
        // Already added head_oid
        let first = commits.next().expect("always at lead HEAD");
        assert_eq!(first.id, head_oid);

        if head_oid != base_oid {
            for commit in commits {
                let child = root;
                root = Node::new(commit);
                root.action = default_action;
                root.children.insert(child.commit.id, child);
                if root.commit.id == base_oid {
                    break;
                }
            }
        }

        Ok(root)
    }

    pub(crate) fn find_commit_mut(&mut self, id: git2::Oid) -> Option<&mut Node> {
        if self.commit.id == id {
            return Some(self);
        }

        for child in self.children.values_mut() {
            if let Some(found) = child.find_commit_mut(id) {
                return Some(found);
            }
        }

        None
    }

    fn merge(&mut self, mut other: Self) {
        assert_eq!(self.commit.id, other.commit.id);

        let mut branches = Vec::new();
        std::mem::swap(&mut other.branches, &mut branches);
        self.branches.extend(branches);

        for (child_id, other_child) in other.children.into_iter() {
            if let Some(self_child) = self.children.get_mut(&child_id) {
                self_child.merge(other_child);
            } else {
                self.children.insert(child_id, other_child);
            }
        }
    }
}
