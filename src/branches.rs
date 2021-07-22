use bstr::ByteSlice;

pub struct Branches<'r> {
    branches: std::collections::BTreeMap<git2::Oid, Vec<git2::Branch<'r>>>,
}

impl<'r> Branches<'r> {
    pub fn new(repo: &'r git2::Repository) -> eyre::Result<Self> {
        log::trace!("Loading branches");
        let mut branches = std::collections::BTreeMap::new();
        for branch in repo.branches(Some(git2::BranchType::Local))? {
            let (branch, _) = branch?;
            let branch_name = if let Some(branch_name) = branch.name()? {
                branch_name
            } else {
                log::debug!(
                    "Ignoring non-UTF8 branch {:?}",
                    branch.name_bytes()?.as_bstr()
                );
                continue;
            };
            if let Some(branch_oid) = branch.get().target() {
                log::trace!("Resolved branch {} as {}", branch_name, branch_oid);
                branches
                    .entry(branch_oid)
                    .or_insert_with(|| Vec::new())
                    .push(branch);
            } else {
                log::debug!("Could not resolve branch {}", branch_name);
            }
        }
        Ok(Self { branches })
    }

    pub fn contains_oid(&self, oid: git2::Oid) -> bool {
        self.branches.contains_key(&oid)
    }

    pub fn get(&self, oid: git2::Oid) -> Option<&[git2::Branch<'r>]> {
        self.branches.get(&oid).map(|v| v.as_slice())
    }

    pub fn remove(&mut self, oid: git2::Oid) -> Option<Vec<git2::Branch<'r>>> {
        self.branches.remove(&oid)
    }

    pub fn oids(&self) -> impl Iterator<Item = git2::Oid> + '_ {
        self.branches.keys().copied()
    }

    pub fn iter(&self) -> impl Iterator<Item = (git2::Oid, &[git2::Branch<'r>])> + '_ {
        self.branches
            .iter()
            .map(|(oid, branch)| (*oid, branch.as_slice()))
    }

    pub fn is_empty(&self) -> bool {
        self.branches.is_empty()
    }

    pub fn all(&self, repo: &'r git2::Repository) -> Self {
        let branches = self
            .branches
            .iter()
            .map(|(oid, branches)| {
                let branches: Vec<_> = branches
                    .iter()
                    .map(|b| clone_local_branch(repo, b).expect("previously confirmed valid"))
                    .collect();
                (*oid, branches)
            })
            .collect();
        Self { branches }
    }

    pub fn dependents(
        &self,
        repo: &'r git2::Repository,
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
                    let branch_name = branch
                        .first()
                        .expect("we always have at least one branch")
                        .name()
                        .ok()
                        .flatten()
                        .expect("we've pre-filtered out non-UTF8");
                    log::trace!(
                        "Branch {} is not on the branch of HEAD ({})",
                        branch_name,
                        head_oid
                    );
                    false
                } else if !is_base_descendant {
                    let branch_name = branch
                        .first()
                        .expect("we always have at least one branch")
                        .name()
                        .ok()
                        .flatten()
                        .expect("we've pre-filtered out non-UTF8");
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
                let branches: Vec<_> = branches
                    .iter()
                    .map(|b| clone_local_branch(repo, b).expect("previously confirmed valid"))
                    .collect();
                (*oid, branches)
            })
            .collect();
        Self { branches }
    }

    pub fn branch(
        &self,
        repo: &'r git2::Repository,
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
                    let branch_name = branch
                        .first()
                        .expect("we always have at least one branch")
                        .name()
                        .ok()
                        .flatten()
                        .expect("we've pre-filtered out non-UTF8");
                    log::trace!(
                        "Branch {} is not on the branch of HEAD ({})",
                        branch_name,
                        head_oid
                    );
                    false
                } else if !is_base_descendant {
                    let branch_name = branch
                        .first()
                        .expect("we always have at least one branch")
                        .name()
                        .ok()
                        .flatten()
                        .expect("we've pre-filtered out non-UTF8");
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
                let branches: Vec<_> = branches
                    .iter()
                    .map(|b| clone_local_branch(repo, b).expect("previously confirmed valid"))
                    .collect();
                (*oid, branches)
            })
            .collect();
        Self { branches }
    }

    pub fn protected(
        &self,
        repo: &'r git2::Repository,
        protected: &crate::protect::ProtectedBranches,
    ) -> Self {
        let branches: std::collections::BTreeMap<_, _> = self
            .branches
            .iter()
            .filter_map(|(oid, branches)| {
                let protected_branches: Vec<_> = branches
                    .iter()
                    .filter_map(|b| {
                        let branch_name = b
                            .name()
                            .ok()
                            .flatten()
                            .expect("we've pre-filtered out non-UTF8");
                        if protected.is_protected(&branch_name) {
                            log::trace!("Branch {} is protected", branch_name);
                            Some(
                                repo.find_branch(branch_name, git2::BranchType::Local)
                                    .expect("previously confirmed to exist"),
                            )
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

impl<'r> std::fmt::Debug for Branches<'r> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let branches: std::collections::BTreeMap<_, _> = self
            .branches
            .iter()
            .map(|(oid, branches)| {
                let branches: Vec<_> = branches
                    .iter()
                    .map(|b| {
                        b.name()
                            .ok()
                            .flatten()
                            .expect("we've pre-filtered out non-UTF8")
                    })
                    .collect();
                (*oid, branches)
            })
            .collect();
        branches.fmt(f)
    }
}

pub fn find_protected_base<'r, 'b>(
    repo: &'r git2::Repository,
    protected_branches: &'b Branches<'r>,
    head_oid: git2::Oid,
) -> Result<&'b git2::Branch<'r>, git2::Error> {
    let protected_base_oids: std::collections::HashMap<_, _> = protected_branches
        .oids()
        .filter_map(|oid| {
            repo.merge_base(head_oid, oid).ok().map(|merge_oid| {
                (
                    merge_oid,
                    protected_branches.get(oid).expect("oid is known to exist"),
                )
            })
        })
        .collect();
    crate::git::commits_from(repo, head_oid)?
        .filter_map(|commit| {
            if let Some(branches) = protected_base_oids.get(&commit.id()) {
                Some(
                    branches
                        .first()
                        .expect("there should always be at least one"),
                )
            } else {
                None
            }
        })
        .next()
        .ok_or_else(|| {
            git2::Error::new(
                git2::ErrorCode::NotFound,
                git2::ErrorClass::Reference,
                "could not find a protected branch to use as a base",
            )
        })
}

pub fn clone_local_branch<'r>(
    repo: &'r git2::Repository,
    branch: &git2::Branch<'r>,
) -> Result<git2::Branch<'r>, git2::Error> {
    let branch_name = branch.name()?.ok_or_else(|| {
        git2::Error::new(
            git2::ErrorCode::NotFound,
            git2::ErrorClass::Reference,
            "branch has non-UTF8 name",
        )
    })?;
    repo.find_branch(branch_name, git2::BranchType::Local)
}
