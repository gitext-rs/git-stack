use std::collections::BTreeMap;

#[derive(Clone, Debug)]
pub struct BranchSet {
    branches: BTreeMap<git2::Oid, Vec<Branch>>,
}

impl BranchSet {
    pub fn from_repo(
        repo: &dyn crate::git::Repo,
        protected: &crate::git::ProtectedBranches,
    ) -> crate::git::Result<Self> {
        let mut branches = Self::new();
        for mut branch in repo.local_branches().map(Branch::from) {
            if protected.is_protected(branch.base_name()) {
                log::trace!("Branch `{}` is protected", branch.display_name());
                if let Some(remote) =
                    repo.find_remote_branch(repo.pull_remote(), branch.base_name())
                {
                    branch.set_kind(BranchKind::Mixed);
                    branch.set_pull_id(remote.id);
                    let mut remote: Branch = remote.into();
                    remote.set_kind(BranchKind::Protected);
                    branches.insert(remote);
                } else {
                    branch.set_kind(BranchKind::Protected);
                }
            } else {
                if let Some(remote) =
                    repo.find_remote_branch(repo.push_remote(), branch.base_name())
                {
                    branch.set_push_id(remote.id);
                }
                branch.set_kind(BranchKind::Mutable);
            }
            branches.insert(branch);
        }
        Ok(branches)
    }

    pub fn update(&mut self, repo: &dyn crate::git::Repo) -> crate::git::Result<()> {
        let mut branches = Self::new();
        for old_branch in self.branches.values().flatten() {
            let new_branch = if let Some(remote) = old_branch.remote() {
                repo.find_remote_branch(remote, old_branch.base_name())
            } else {
                repo.find_local_branch(old_branch.base_name())
            };
            let new_branch = if let Some(mut new_branch) = new_branch.map(Branch::from) {
                new_branch.kind = old_branch.kind;
                new_branch.pull_id = old_branch.pull_id.and_then(|_| {
                    repo.find_remote_branch(repo.pull_remote(), old_branch.base_name())
                        .map(|b| b.id)
                });
                new_branch.push_id = old_branch.push_id.and_then(|_| {
                    repo.find_remote_branch(repo.push_remote(), old_branch.base_name())
                        .map(|b| b.id)
                });
                if new_branch.id() != old_branch.id() {
                    log::debug!(
                        "{} moved from {} to {}",
                        new_branch.display_name(),
                        old_branch.id(),
                        new_branch.id()
                    );
                }
                new_branch
            } else {
                log::debug!("{} no longer exists", old_branch.display_name());
                let mut old_branch = old_branch.clone();
                old_branch.kind = BranchKind::Deleted;
                old_branch.pull_id = None;
                old_branch.push_id = None;
                old_branch
            };
            branches.insert(new_branch);
        }
        *self = branches;
        Ok(())
    }
}

impl BranchSet {
    pub fn new() -> Self {
        Self {
            branches: Default::default(),
        }
    }

    pub fn insert(&mut self, mut branch: Branch) -> Option<Branch> {
        let id = branch.id();
        let branches = self.branches.entry(id).or_default();

        let mut existing_index = None;
        for (i, current) in branches.iter().enumerate() {
            if current.core == branch.core {
                existing_index = Some(i);
                break;
            }
        }

        if let Some(existing_index) = existing_index {
            std::mem::swap(&mut branch, &mut branches[existing_index]);
            Some(branch)
        } else {
            branches.push(branch);
            None
        }
    }

    pub fn remove(&mut self, oid: git2::Oid) -> Option<Vec<Branch>> {
        self.branches.remove(&oid)
    }

    pub fn contains_oid(&self, oid: git2::Oid) -> bool {
        self.branches.contains_key(&oid)
    }

    pub fn get(&self, oid: git2::Oid) -> Option<&[Branch]> {
        self.branches.get(&oid).map(|v| v.as_slice())
    }

    pub fn get_mut(&mut self, oid: git2::Oid) -> Option<&mut [Branch]> {
        self.branches.get_mut(&oid).map(|v| v.as_mut_slice())
    }

    pub fn is_empty(&self) -> bool {
        self.branches.is_empty()
    }

    pub fn len(&self) -> usize {
        self.branches.len()
    }

    pub fn iter(&self) -> impl Iterator<Item = (git2::Oid, &[Branch])> + '_ {
        self.branches
            .iter()
            .map(|(oid, branch)| (*oid, branch.as_slice()))
    }

    pub fn oids(&self) -> impl Iterator<Item = git2::Oid> + '_ {
        self.branches.keys().copied()
    }
}

impl BranchSet {
    pub fn all(&self) -> Self {
        self.clone()
    }

    pub fn descendants(&self, repo: &dyn crate::git::Repo, base_oid: git2::Oid) -> Self {
        let branches = self
            .branches
            .iter()
            .filter(|(branch_oid, branch)| {
                let is_base_descendant = repo
                    .merge_base(**branch_oid, base_oid)
                    .map(|merge_oid| merge_oid == base_oid)
                    .unwrap_or(false);
                if is_base_descendant {
                    true
                } else {
                    let first_branch = &branch.first().expect("we always have at least one branch");
                    log::trace!(
                        "Branch {} is not on the branch of {}",
                        first_branch.display_name(),
                        base_oid
                    );
                    false
                }
            })
            .map(|(oid, branches)| {
                let branches: Vec<_> = branches.clone();
                (*oid, branches)
            })
            .collect();
        Self { branches }
    }

    pub fn dependents(
        &self,
        repo: &dyn crate::git::Repo,
        base_oid: git2::Oid,
        head_oid: git2::Oid,
    ) -> Self {
        let branches = self
            .branches
            .iter()
            .filter(|(branch_oid, branch)| {
                let is_shared_base = repo
                    .merge_base(**branch_oid, head_oid)
                    .map(|merge_oid| merge_oid == base_oid && **branch_oid != base_oid)
                    .unwrap_or(false);
                let is_base_descendant = repo
                    .merge_base(**branch_oid, base_oid)
                    .map(|merge_oid| merge_oid == base_oid)
                    .unwrap_or(false);
                if is_shared_base {
                    let first_branch = &branch.first().expect("we always have at least one branch");
                    log::trace!(
                        "Branch {} is not on the branch of HEAD ({})",
                        first_branch.display_name(),
                        head_oid
                    );
                    false
                } else if !is_base_descendant {
                    let first_branch = &branch.first().expect("we always have at least one branch");
                    log::trace!(
                        "Branch {} is not on the branch of {}",
                        first_branch.display_name(),
                        base_oid
                    );
                    false
                } else {
                    true
                }
            })
            .map(|(oid, branches)| {
                let branches: Vec<_> = branches.clone();
                (*oid, branches)
            })
            .collect();
        Self { branches }
    }

    pub fn branch(
        &self,
        repo: &dyn crate::git::Repo,
        base_oid: git2::Oid,
        head_oid: git2::Oid,
    ) -> Self {
        let branches = self
            .branches
            .iter()
            .filter(|(branch_oid, branch)| {
                let is_head_ancestor = repo
                    .merge_base(**branch_oid, head_oid)
                    .map(|merge_oid| **branch_oid == merge_oid)
                    .unwrap_or(false);
                let is_base_descendant = repo
                    .merge_base(**branch_oid, base_oid)
                    .map(|merge_oid| merge_oid == base_oid)
                    .unwrap_or(false);
                if !is_head_ancestor {
                    let first_branch = &branch.first().expect("we always have at least one branch");
                    log::trace!(
                        "Branch {} is not on the branch of HEAD ({})",
                        first_branch.display_name(),
                        head_oid
                    );
                    false
                } else if !is_base_descendant {
                    let first_branch = &branch.first().expect("we always have at least one branch");
                    log::trace!(
                        "Branch {} is not on the branch of {}",
                        first_branch.display_name(),
                        base_oid
                    );
                    false
                } else {
                    true
                }
            })
            .map(|(oid, branches)| {
                let branches: Vec<_> = branches.clone();
                (*oid, branches)
            })
            .collect();
        Self { branches }
    }
}

impl Default for BranchSet {
    fn default() -> Self {
        Self::new()
    }
}

impl IntoIterator for BranchSet {
    type Item = (git2::Oid, Vec<Branch>);
    type IntoIter = std::collections::btree_map::IntoIter<git2::Oid, Vec<Branch>>;

    fn into_iter(self) -> Self::IntoIter {
        self.branches.into_iter()
    }
}

impl Extend<Branch> for BranchSet {
    fn extend<T: IntoIterator<Item = Branch>>(&mut self, iter: T) {
        for branch in iter {
            self.insert(branch);
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Branch {
    core: crate::git::Branch,
    kind: BranchKind,
    pull_id: Option<git2::Oid>,
    push_id: Option<git2::Oid>,
}

impl Branch {
    pub fn set_kind(&mut self, kind: BranchKind) -> &mut Self {
        self.kind = kind;
        self
    }

    pub fn set_id(&mut self, id: git2::Oid) -> &mut Self {
        self.core.id = id;
        self
    }

    pub fn set_pull_id(&mut self, pull_id: git2::Oid) -> &mut Self {
        self.pull_id = Some(pull_id);
        self
    }

    pub fn set_push_id(&mut self, push_id: git2::Oid) -> &mut Self {
        self.push_id = Some(push_id);
        self
    }
}

impl Branch {
    pub fn git(&self) -> &crate::git::Branch {
        &self.core
    }

    pub fn name(&self) -> String {
        self.core.to_string()
    }

    pub fn display_name(&self) -> impl std::fmt::Display + '_ {
        &self.core
    }

    pub fn remote(&self) -> Option<&str> {
        self.core.remote.as_deref()
    }

    pub fn base_name(&self) -> &str {
        &self.core.name
    }

    pub fn local_name(&self) -> Option<&str> {
        self.core.local_name()
    }

    pub fn kind(&self) -> BranchKind {
        self.kind
    }

    pub fn id(&self) -> git2::Oid {
        self.core.id
    }

    pub fn pull_id(&self) -> Option<git2::Oid> {
        self.pull_id
    }

    pub fn push_id(&self) -> Option<git2::Oid> {
        self.push_id
    }
}

impl From<crate::git::Branch> for Branch {
    fn from(core: crate::git::Branch) -> Self {
        Self {
            core,
            kind: BranchKind::Deleted,
            pull_id: None,
            push_id: None,
        }
    }
}

impl PartialEq<crate::git::Branch> for Branch {
    fn eq(&self, other: &crate::git::Branch) -> bool {
        self.core == *other
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum BranchKind {
    // Of no interest
    Deleted,
    // Completely mutable
    Mutable,
    // Local is mutable, remove is protected
    Mixed,
    // Must not touch
    Protected,
}

impl BranchKind {
    pub fn has_user_commits(self) -> bool {
        match self {
            Self::Deleted => false,
            Self::Mutable => true,
            Self::Mixed => true,
            Self::Protected => false,
        }
    }
}

pub fn find_protected_base<'b>(
    repo: &dyn crate::git::Repo,
    branches: &'b BranchSet,
    head_oid: git2::Oid,
) -> Option<&'b Branch> {
    // We're being asked about a protected branch
    if let Some(head_branches) = branches.get(head_oid) {
        if let Some(head_branch) = head_branches
            .iter()
            .find(|b| b.kind() == BranchKind::Protected)
        {
            return Some(head_branch);
        }
    }

    let protected_base_oids = branches
        .iter()
        .filter_map(|(id, b)| {
            b.iter()
                .find(|b| b.kind() == BranchKind::Protected)
                .map(|_| id)
        })
        .filter_map(|oid| {
            let merge_oid = repo.merge_base(head_oid, oid)?;
            Some((merge_oid, oid))
        })
        .collect::<Vec<_>>();

    // Not much choice for applicable base
    match protected_base_oids.len() {
        0 => {
            return None;
        }
        1 => {
            let (_, protected_oid) = protected_base_oids[0];
            let protected_branch = branches
                .get(protected_oid)
                .expect("protected_oid came from protected_branches")
                .iter()
                .find(|b| b.kind() == BranchKind::Protected)
                .expect("protected_branches has at least one protected branch");
            return Some(protected_branch);
        }
        _ => {}
    }

    // Prefer protected branch from first parent
    let mut next_oid = Some(head_oid);
    while let Some(parent_oid) = next_oid {
        if let Some((_, closest_common_oid)) = protected_base_oids
            .iter()
            .filter(|(base, _)| *base == parent_oid)
            .min_by_key(|(base, branch)| {
                (
                    repo.commit_count(*base, head_oid),
                    repo.commit_count(*base, *branch),
                )
            })
        {
            let protected_branch = branches
                .get(*closest_common_oid)
                .expect("protected_oid came from protected_branches")
                .iter()
                .find(|b| b.kind() == BranchKind::Protected)
                .expect("protected_branches has at least one protected branch");
            return Some(protected_branch);
        }
        next_oid = repo
            .parent_ids(parent_oid)
            .expect("child_oid came from verified source")
            .first()
            .copied();
    }

    // Prefer most direct ancestors
    if let Some((_, closest_common_oid)) =
        protected_base_oids.iter().min_by_key(|(base, protected)| {
            let to_protected = repo.commit_count(*base, *protected);
            let to_head = repo.commit_count(*base, head_oid);
            (to_protected, to_head)
        })
    {
        let protected_branch = branches
            .get(*closest_common_oid)
            .expect("protected_oid came from protected_branches")
            .iter()
            .find(|b| b.kind() == BranchKind::Protected)
            .expect("protected_branches has at least one protected branch");
        return Some(protected_branch);
    }

    None
}

pub fn infer_base(repo: &dyn crate::git::Repo, head_oid: git2::Oid) -> Option<git2::Oid> {
    let head_commit = repo.find_commit(head_oid)?;
    let head_committer = head_commit.committer.clone();

    let mut next_oid = head_oid;
    loop {
        let next_commit = repo.find_commit(next_oid)?;
        if next_commit.committer != head_committer {
            return Some(next_oid);
        }
        let parent_ids = repo.parent_ids(next_oid).ok()?;
        match parent_ids.len() {
            1 => {
                next_oid = parent_ids[0];
            }
            _ => {
                // Assume merge-commits are topic branches being merged into the upstream
                return Some(next_oid);
            }
        }
    }
}
