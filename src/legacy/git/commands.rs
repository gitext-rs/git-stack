#[derive(Clone, Default, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Script {
    pub commands: Vec<Command>,
    pub dependents: Vec<Script>,
}

impl Script {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn is_empty(&self) -> bool {
        self.commands.is_empty() && self.dependents.is_empty()
    }

    pub fn branch(&self) -> Option<&str> {
        for command in self.commands.iter().rev() {
            if let Command::CreateBranch(name) = command {
                return Some(name);
            }
        }

        None
    }

    pub fn dependent_branches(&self) -> Vec<&str> {
        let mut branches = Vec::new();
        for dependent in self.dependents.iter() {
            branches.push(dependent.branch().unwrap_or("detached"));
            branches.extend(dependent.dependent_branches());
        }
        branches
    }

    pub fn is_branch_deleted(&self, branch: &str) -> bool {
        for command in &self.commands {
            if let Command::DeleteBranch(ref current) = command {
                if branch == current {
                    return true;
                }
            }
        }

        for dependent in &self.dependents {
            if dependent.is_branch_deleted(branch) {
                return true;
            }
        }

        false
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Command {
    /// Switch to an existing commit
    SwitchCommit(git2::Oid),
    /// Mark the current commit with an `Oid` for future reference
    RegisterMark(git2::Oid),
    /// Switch to a previously registered marked commit
    SwitchMark(git2::Oid),
    /// Cherry-pick an existing commit
    CherryPick(git2::Oid),
    /// Squash a commit into prior commit, keeping the parent commits identity
    Fixup(git2::Oid),
    /// Mark a branch for creation at the current commit
    CreateBranch(String),
    /// Mark a branch for deletion
    DeleteBranch(String),
}

pub struct Executor {
    head_oid: git2::Oid,
    marks: std::collections::HashMap<git2::Oid, git2::Oid>,
    branches: Vec<(git2::Oid, String)>,
    delete_branches: Vec<String>,
    post_rewrite: Vec<(git2::Oid, git2::Oid)>,
    dry_run: bool,
    detached: bool,
}

impl Executor {
    pub fn new(repo: &dyn crate::legacy::git::Repo, dry_run: bool) -> Executor {
        let head_oid = repo.head_commit().id;
        Self {
            head_oid,
            marks: Default::default(),
            branches: Default::default(),
            delete_branches: Default::default(),
            post_rewrite: Default::default(),
            dry_run,
            detached: false,
        }
    }

    pub fn run_script<'s>(
        &mut self,
        repo: &mut dyn crate::legacy::git::Repo,
        script: &'s Script,
    ) -> Vec<(git2::Error, &'s str, Vec<&'s str>)> {
        let mut failures = Vec::new();
        let branch_name = script.branch().unwrap_or("detached");

        log::trace!("Applying `{}`", branch_name);
        log::trace!("Script: {:#?}", script.commands);
        #[allow(clippy::disallowed_methods)]
        let res = script
            .commands
            .iter()
            .try_for_each(|command| self.stage_single(repo, command));
        match res.and_then(|_| self.commit(repo)) {
            Ok(()) => {
                log::trace!("         `{}` succeeded", branch_name);
                for dependent in script.dependents.iter() {
                    failures.extend(self.run_script(repo, dependent));
                }
                if !failures.is_empty() {
                    log::trace!("         `{}`'s dependent failed", branch_name);
                }
            }
            Err(err) => {
                log::trace!("         `{}` failed: {}", branch_name, err);
                self.abandon(repo);
                failures.push((err, branch_name, script.dependent_branches()));
            }
        }

        failures
    }

    pub fn stage_single(
        &mut self,
        repo: &mut dyn crate::legacy::git::Repo,
        command: &Command,
    ) -> Result<(), git2::Error> {
        match command {
            Command::SwitchCommit(oid) => {
                let commit = repo.find_commit(*oid).ok_or_else(|| {
                    git2::Error::new(
                        git2::ErrorCode::NotFound,
                        git2::ErrorClass::Reference,
                        format!("could not find commit {oid:?}"),
                    )
                })?;
                log::trace!("git checkout {}  # {}", oid, commit.summary);
                self.head_oid = *oid;
            }
            Command::RegisterMark(mark_oid) => {
                let target_oid = self.head_oid;
                self.marks.insert(*mark_oid, target_oid);
            }
            Command::SwitchMark(mark_oid) => {
                let oid = *self
                    .marks
                    .get(mark_oid)
                    .expect("We only switch to marks that are created");

                let commit = repo.find_commit(oid).unwrap();
                log::trace!("git checkout {}  # {}", oid, commit.summary);
                self.head_oid = oid;
            }
            Command::CherryPick(cherry_oid) => {
                let cherry_commit = repo.find_commit(*cherry_oid).ok_or_else(|| {
                    git2::Error::new(
                        git2::ErrorCode::NotFound,
                        git2::ErrorClass::Reference,
                        format!("could not find commit {cherry_oid:?}"),
                    )
                })?;
                log::trace!(
                    "git cherry-pick {}  # {}",
                    cherry_oid,
                    cherry_commit.summary
                );
                let updated_oid = if self.dry_run {
                    *cherry_oid
                } else {
                    repo.cherry_pick(self.head_oid, *cherry_oid)?
                };
                self.post_rewrite.push((*cherry_oid, updated_oid));
                self.head_oid = updated_oid;
            }
            Command::Fixup(squash_oid) => {
                let cherry_commit = repo.find_commit(*squash_oid).ok_or_else(|| {
                    git2::Error::new(
                        git2::ErrorCode::NotFound,
                        git2::ErrorClass::Reference,
                        format!("could not find commit {squash_oid:?}"),
                    )
                })?;
                log::trace!(
                    "git merge --squash {}  # {}",
                    squash_oid,
                    cherry_commit.summary
                );
                let updated_oid = if self.dry_run {
                    *squash_oid
                } else {
                    repo.squash(*squash_oid, self.head_oid)?
                };
                for (_old_oid, new_oid) in &mut self.post_rewrite {
                    if *new_oid == self.head_oid {
                        *new_oid = updated_oid;
                    }
                }
                self.post_rewrite.push((*squash_oid, updated_oid));
                self.head_oid = updated_oid;
            }
            Command::CreateBranch(name) => {
                let branch_oid = self.head_oid;
                self.branches.push((branch_oid, name.to_owned()));
            }
            Command::DeleteBranch(name) => {
                self.delete_branches.push(name.to_owned());
            }
        }

        Ok(())
    }

    pub fn commit(&mut self, repo: &mut dyn crate::legacy::git::Repo) -> Result<(), git2::Error> {
        let hook_repo = repo.path().map(git2::Repository::open).transpose()?;
        let hooks = if self.dry_run {
            None
        } else {
            hook_repo
                .as_ref()
                .map(git2_ext::hooks::Hooks::with_repo)
                .transpose()?
        };

        log::trace!("Running reference-transaction hook");
        let reference_transaction = self.branches.clone();
        let reference_transaction: Vec<(git2::Oid, git2::Oid, &str)> = reference_transaction
            .iter()
            .map(|(new_oid, name)| {
                // HACK: relying on "force updating the reference regardless of its current value" part
                // of rules rather than tracking the old value
                let old_oid = git2::Oid::zero();
                (old_oid, *new_oid, name.as_str())
            })
            .collect();
        let reference_transaction =
            if let (Some(hook_repo), Some(hooks)) = (hook_repo.as_ref(), hooks.as_ref()) {
                Some(
                    hooks
                        .run_reference_transaction(hook_repo, &reference_transaction)
                        .map_err(|err| {
                            git2::Error::new(
                                git2::ErrorCode::GenericError,
                                git2::ErrorClass::Os,
                                err.to_string(),
                            )
                        })?,
                )
            } else {
                None
            };

        if !self.branches.is_empty() || !self.delete_branches.is_empty() {
            // In case we are changing the branch HEAD is attached to
            if !self.dry_run {
                repo.detach()?;
                self.detached = true;
            }

            for (oid, name) in self.branches.iter() {
                let commit = repo.find_commit(*oid).unwrap();
                log::trace!("git checkout {}  # {}", oid, commit.summary);
                log::trace!("git switch -c {}", name);
                if !self.dry_run {
                    repo.branch(name, *oid)?;
                }
            }
        }
        self.branches.clear();

        for name in self.delete_branches.iter() {
            log::trace!("git branch -D {}", name);
            if !self.dry_run {
                repo.delete_branch(name)?;
            }
        }
        self.delete_branches.clear();

        if let Some(tx) = reference_transaction {
            tx.committed();
        }
        self.post_rewrite.retain(|(old, new)| old != new);
        if !self.post_rewrite.is_empty() {
            log::trace!("Running post-rewrite hook");
            if let (Some(hook_repo), Some(hooks)) = (hook_repo.as_ref(), hooks.as_ref()) {
                hooks.run_post_rewrite_rebase(hook_repo, &self.post_rewrite);
            }
            self.post_rewrite.clear();
        }

        Ok(())
    }

    pub fn abandon(&mut self, repo: &dyn crate::legacy::git::Repo) {
        self.head_oid = repo.head_commit().id;
        self.branches.clear();
        self.delete_branches.clear();
        self.post_rewrite.clear();
    }

    pub fn close(
        &mut self,
        repo: &mut dyn crate::legacy::git::Repo,
        restore_branch: &str,
    ) -> Result<(), git2::Error> {
        assert_eq!(&self.branches, &[]);
        assert_eq!(self.delete_branches, Vec::<String>::new());
        log::trace!("git switch {}", restore_branch);
        if !self.dry_run {
            if self.detached {
                repo.switch(restore_branch)?;
            }
            self.head_oid = repo.head_commit().id;
        }

        Ok(())
    }
}
