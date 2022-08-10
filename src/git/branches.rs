#[derive(Clone, Default, Debug, PartialEq, Eq)]
pub struct Branches {
    branches: std::collections::BTreeMap<git2::Oid, Vec<crate::git::Branch>>,
}

impl Branches {
    pub fn new(branches: impl IntoIterator<Item = crate::git::Branch>) -> Self {
        let mut grouped_branches = std::collections::BTreeMap::new();
        for branch in branches {
            grouped_branches
                .entry(branch.id)
                .or_insert_with(Vec::new)
                .push(branch);
        }
        Self {
            branches: grouped_branches,
        }
    }

    pub fn update(&mut self, repo: &dyn crate::git::Repo) {
        let mut new = Self::new(self.branches.values().flatten().filter_map(|b| {
            if let Some(remote) = b.remote.as_deref() {
                repo.find_remote_branch(remote, &b.name)
            } else {
                repo.find_local_branch(&b.name)
            }
        }));
        std::mem::swap(&mut new, self);
    }

    pub fn insert(&mut self, branch: crate::git::Branch) {
        let branches = self.branches.entry(branch.id).or_insert_with(Vec::new);
        if !branches
            .iter()
            .any(|b| b.remote == branch.remote && b.name == branch.name)
        {
            branches.push(branch);
        }
    }

    pub fn extend(&mut self, branches: impl Iterator<Item = crate::git::Branch>) {
        for branch in branches {
            self.insert(branch);
        }
    }

    pub fn contains_oid(&self, oid: git2::Oid) -> bool {
        self.branches.contains_key(&oid)
    }

    pub fn get(&self, oid: git2::Oid) -> Option<&[crate::git::Branch]> {
        self.branches.get(&oid).map(|v| v.as_slice())
    }

    pub fn remove(&mut self, oid: git2::Oid) -> Option<Vec<crate::git::Branch>> {
        self.branches.remove(&oid)
    }

    pub fn oids(&self) -> impl Iterator<Item = git2::Oid> + '_ {
        self.branches.keys().copied()
    }

    pub fn iter(&self) -> impl Iterator<Item = (git2::Oid, &[crate::git::Branch])> + '_ {
        self.branches
            .iter()
            .map(|(oid, branch)| (*oid, branch.as_slice()))
    }

    pub fn is_empty(&self) -> bool {
        self.branches.is_empty()
    }

    pub fn len(&self) -> usize {
        self.branches.len()
    }

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
                        first_branch,
                        base_oid
                    );
                    false
                }
            })
            .map(|(oid, branches)| {
                let branches: Vec<_> = branches.to_vec();
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
                        first_branch,
                        head_oid
                    );
                    false
                } else if !is_base_descendant {
                    let first_branch = &branch.first().expect("we always have at least one branch");
                    log::trace!(
                        "Branch {} is not on the branch of {}",
                        first_branch,
                        base_oid
                    );
                    false
                } else {
                    true
                }
            })
            .map(|(oid, branches)| {
                let branches: Vec<_> = branches.to_vec();
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
                        first_branch,
                        head_oid
                    );
                    false
                } else if !is_base_descendant {
                    let first_branch = &branch.first().expect("we always have at least one branch");
                    log::trace!(
                        "Branch {} is not on the branch of {}",
                        first_branch,
                        base_oid
                    );
                    false
                } else {
                    true
                }
            })
            .map(|(oid, branches)| {
                let branches: Vec<_> = branches.to_vec();
                (*oid, branches)
            })
            .collect();
        Self { branches }
    }
}

impl IntoIterator for Branches {
    type Item = (git2::Oid, Vec<crate::git::Branch>);
    type IntoIter = std::collections::btree_map::IntoIter<git2::Oid, Vec<crate::git::Branch>>;

    fn into_iter(self) -> Self::IntoIter {
        self.branches.into_iter()
    }
}

pub fn find_protected_base<'b>(
    repo: &dyn crate::git::Repo,
    protected_branches: &'b Branches,
    head_oid: git2::Oid,
) -> Option<&'b crate::git::Branch> {
    // We're being asked about a protected branch
    if let Some(head_branches) = protected_branches.get(head_oid) {
        return head_branches.first();
    }

    let protected_base_oids = protected_branches
        .oids()
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
            return protected_branches
                .get(protected_oid)
                .expect("protected_oid came from protected_branches")
                .first();
        }
        _ => {}
    }

    // Prefer protected branch from first parent
    let mut child_oid = head_oid;
    while let Some(parent_oid) = repo
        .parent_ids(child_oid)
        .expect("child_oid came from verified source")
        .first()
        .copied()
    {
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
            return protected_branches
                .get(*closest_common_oid)
                .expect("protected_oid came from protected_branches")
                .first();
        }
        child_oid = parent_oid;
    }

    // Prefer most direct ancestors
    if let Some((_, closest_common_oid)) =
        protected_base_oids.iter().min_by_key(|(base, protected)| {
            let to_protected = repo.commit_count(*base, *protected);
            let to_head = repo.commit_count(*base, head_oid);
            (to_protected, to_head)
        })
    {
        return protected_branches
            .get(*closest_common_oid)
            .expect("protected_oid came from protected_branches")
            .first();
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
