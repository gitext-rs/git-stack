#[derive(Clone, Default, Debug, PartialEq, Eq)]
pub struct Branches {
    branches: std::collections::BTreeMap<git2::Oid, Vec<crate::git::Branch>>,
}

impl Branches {
    pub fn new(branches: impl Iterator<Item = crate::git::Branch>) -> Self {
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
        let mut new = Self::new(
            self.branches
                .values()
                .flatten()
                .filter_map(|b| repo.find_local_branch(&b.name)),
        );
        std::mem::swap(&mut new, self);
    }

    pub fn insert(&mut self, branch: crate::git::Branch) {
        self.branches
            .entry(branch.id)
            .or_insert_with(Vec::new)
            .push(branch);
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
                    let branch_name = &branch
                        .first()
                        .expect("we always have at least one branch")
                        .name;
                    log::trace!(
                        "Branch {} is not on the branch of {}",
                        branch_name,
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
                    let branch_name = &branch
                        .first()
                        .expect("we always have at least one branch")
                        .name;
                    log::trace!(
                        "Branch {} is not on the branch of HEAD ({})",
                        branch_name,
                        head_oid
                    );
                    false
                } else if !is_base_descendant {
                    let branch_name = &branch
                        .first()
                        .expect("we always have at least one branch")
                        .name;
                    log::trace!(
                        "Branch {} is not on the branch of {}",
                        branch_name,
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
                    let branch_name = &branch
                        .first()
                        .expect("we always have at least one branch")
                        .name;
                    log::trace!(
                        "Branch {} is not on the branch of HEAD ({})",
                        branch_name,
                        head_oid
                    );
                    false
                } else if !is_base_descendant {
                    let branch_name = &branch
                        .first()
                        .expect("we always have at least one branch")
                        .name;
                    log::trace!(
                        "Branch {} is not on the branch of {}",
                        branch_name,
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

    pub fn protected(&self, protected: &crate::git::ProtectedBranches) -> Self {
        let branches: std::collections::BTreeMap<_, _> = self
            .branches
            .iter()
            .filter_map(|(oid, branches)| {
                let protected_branches: Vec<_> = branches
                    .iter()
                    .filter_map(|b| {
                        if protected.is_protected(&b.name) {
                            log::trace!("Branch {} is protected", b.name);
                            Some(b.clone())
                        } else {
                            None
                        }
                    })
                    .collect();
                if protected_branches.is_empty() {
                    None
                } else {
                    Some((*oid, protected_branches))
                }
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
    let protected_base_oids: std::collections::HashMap<_, _> = protected_branches
        .oids()
        .filter_map(|oid| {
            repo.merge_base(head_oid, oid).map(|merge_oid| {
                (
                    merge_oid,
                    protected_branches.get(oid).expect("oid is known to exist"),
                )
            })
        })
        .collect();
    repo.commits_from(head_oid)
        .filter_map(|commit| {
            protected_base_oids.get(&commit.id).map(|branches| {
                branches
                    .first()
                    .expect("there should always be at least one")
            })
        })
        .next()
}

pub fn find_base<'b>(
    repo: &dyn crate::git::Repo,
    branches: &'b Branches,
    head_oid: git2::Oid,
) -> Option<&'b crate::git::Branch> {
    repo.commits_from(head_oid)
        .filter(|c| c.id != head_oid)
        .filter_map(|commit| {
            branches.get(commit.id).map(|branches| {
                branches
                    .first()
                    .expect("there should always be at least one")
            })
        })
        .next()
}
