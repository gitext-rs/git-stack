use bstr::ByteSlice;
use itertools::Itertools;

pub trait Repo {
    fn user(&self) -> Option<std::rc::Rc<str>>;

    fn is_dirty(&self) -> bool;
    fn merge_base(&self, one: git2::Oid, two: git2::Oid) -> Option<git2::Oid>;

    fn find_commit(&self, id: git2::Oid) -> Option<std::rc::Rc<Commit>>;
    fn head_commit(&self) -> std::rc::Rc<Commit>;
    fn head_branch(&self) -> Option<Branch>;
    fn resolve(&self, revspec: &str) -> Option<std::rc::Rc<Commit>>;
    fn commits_from(
        &self,
        head_id: git2::Oid,
    ) -> Box<dyn Iterator<Item = std::rc::Rc<Commit>> + '_>;
    fn contains_commit(
        &self,
        haystack_id: git2::Oid,
        needle_id: git2::Oid,
    ) -> Result<bool, git2::Error>;
    fn cherry_pick(
        &mut self,
        head_id: git2::Oid,
        cherry_id: git2::Oid,
    ) -> Result<git2::Oid, git2::Error>;
    fn squash(&mut self, head_id: git2::Oid, into_id: git2::Oid) -> Result<git2::Oid, git2::Error>;

    fn stash_push(&mut self, message: Option<&str>) -> Result<git2::Oid, git2::Error>;
    fn stash_pop(&mut self, stash_id: git2::Oid) -> Result<(), git2::Error>;

    fn branch(&mut self, name: &str, id: git2::Oid) -> Result<(), git2::Error>;
    fn delete_branch(&mut self, name: &str) -> Result<(), git2::Error>;
    fn find_local_branch(&self, name: &str) -> Option<Branch>;
    fn local_branches(&self) -> Box<dyn Iterator<Item = Branch> + '_>;
    fn detach(&mut self) -> Result<(), git2::Error>;
    fn switch(&mut self, name: &str) -> Result<(), git2::Error>;
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Branch {
    pub name: String,
    pub id: git2::Oid,
    pub push_id: Option<git2::Oid>,
    pub pull_id: Option<git2::Oid>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Commit {
    pub id: git2::Oid,
    pub tree_id: git2::Oid,
    pub summary: bstr::BString,
    pub time: std::time::SystemTime,
    pub author: Option<std::rc::Rc<str>>,
    pub committer: Option<std::rc::Rc<str>>,
}

impl Commit {
    pub fn fixup_summary(&self) -> Option<&bstr::BStr> {
        self.summary
            .strip_prefix(b"fixup! ")
            .map(ByteSlice::as_bstr)
    }

    pub fn wip_summary(&self) -> Option<&bstr::BStr> {
        // Gitlab MRs only: b"[Draft]", b"(Draft)",
        static WIP_PREFIXES: &[&[u8]] = &[
            b"WIP:", b"draft:", b"Draft:", // Gitlab commits
            b"wip ", b"WIP ", // Less formal
        ];

        if self.summary == b"WIP".as_bstr() || self.summary == b"wip".as_bstr() {
            // Very informal
            Some(b"".as_bstr())
        } else {
            WIP_PREFIXES
                .iter()
                .filter_map(|prefix| {
                    self.summary
                        .strip_prefix(*prefix)
                        .map(ByteSlice::trim)
                        .map(ByteSlice::as_bstr)
                })
                .next()
        }
    }

    pub fn revert_summary(&self) -> Option<&bstr::BStr> {
        self.summary
            .strip_prefix(b"Revert ")
            .and_then(|s| s.strip_suffix(b"\""))
            .map(ByteSlice::as_bstr)
    }
}

pub struct GitRepo {
    repo: git2::Repository,
    push_remote: Option<String>,
    pull_remote: Option<String>,
    commits: std::cell::RefCell<std::collections::HashMap<git2::Oid, std::rc::Rc<Commit>>>,
    interned_strings: std::cell::RefCell<std::collections::HashSet<std::rc::Rc<str>>>,
}

impl GitRepo {
    pub fn new(repo: git2::Repository) -> Self {
        Self {
            repo,
            push_remote: None,
            pull_remote: None,
            commits: Default::default(),
            interned_strings: Default::default(),
        }
    }

    pub fn set_push_remote(&mut self, remote: &str) {
        self.push_remote = Some(remote.to_owned());
    }

    pub fn set_pull_remote(&mut self, remote: &str) {
        self.pull_remote = Some(remote.to_owned());
    }

    pub fn push_remote(&self) -> &str {
        self.push_remote.as_deref().unwrap_or("origin")
    }

    pub fn pull_remote(&self) -> &str {
        self.pull_remote.as_deref().unwrap_or("origin")
    }

    pub fn raw(&self) -> &git2::Repository {
        &self.repo
    }

    pub fn user(&self) -> Option<std::rc::Rc<str>> {
        self.repo
            .signature()
            .ok()
            .and_then(|s| s.name().map(|n| self.intern_string(n)))
    }

    pub fn is_dirty(&self) -> bool {
        if self.repo.state() != git2::RepositoryState::Clean {
            log::trace!("Repository status is unclean: {:?}", self.repo.state());
            return true;
        }

        let status = self
            .repo
            .statuses(Some(git2::StatusOptions::new().include_ignored(false)))
            .unwrap();
        if status.is_empty() {
            false
        } else {
            log::trace!(
                "Repository is dirty: {}",
                status
                    .iter()
                    .flat_map(|s| s.path().map(|s| s.to_owned()))
                    .join(", ")
            );
            true
        }
    }

    pub fn merge_base(&self, one: git2::Oid, two: git2::Oid) -> Option<git2::Oid> {
        self.repo.merge_base(one, two).ok()
    }

    pub fn find_commit(&self, id: git2::Oid) -> Option<std::rc::Rc<Commit>> {
        let mut commits = self.commits.borrow_mut();
        if let Some(commit) = commits.get(&id) {
            Some(std::rc::Rc::clone(commit))
        } else {
            let commit = self.repo.find_commit(id).ok()?;
            let summary: bstr::BString = commit.summary_bytes().unwrap().into();
            let time = std::time::SystemTime::UNIX_EPOCH
                + std::time::Duration::from_secs(commit.time().seconds().max(0) as u64);

            let author = commit.author().name().map(|n| self.intern_string(n));
            let committer = commit.author().name().map(|n| self.intern_string(n));
            let commit = std::rc::Rc::new(Commit {
                id: commit.id(),
                tree_id: commit.tree_id(),
                summary,
                time,
                author,
                committer,
            });
            commits.insert(id, std::rc::Rc::clone(&commit));
            Some(commit)
        }
    }

    pub fn head_commit(&self) -> std::rc::Rc<Commit> {
        let head_id = self
            .repo
            .head()
            .unwrap()
            .resolve()
            .unwrap()
            .target()
            .unwrap();
        self.find_commit(head_id).unwrap()
    }

    pub fn head_branch(&self) -> Option<Branch> {
        let resolved = self.repo.head().unwrap().resolve().unwrap();
        let name = resolved.shorthand()?;
        let id = resolved.target()?;

        let push_id = self
            .repo
            .find_branch(
                &format!("{}/{}", self.push_remote(), name),
                git2::BranchType::Remote,
            )
            .ok()
            .and_then(|b| b.get().target());
        let pull_id = self
            .repo
            .find_branch(
                &format!("{}/{}", self.pull_remote(), name),
                git2::BranchType::Remote,
            )
            .ok()
            .and_then(|b| b.get().target());

        Some(Branch {
            name: name.to_owned(),
            id,
            push_id,
            pull_id,
        })
    }

    pub fn resolve(&self, revspec: &str) -> Option<std::rc::Rc<Commit>> {
        let id = self.repo.revparse_single(revspec).ok()?.id();
        self.find_commit(id)
    }

    pub fn commits_from(
        &self,
        head_id: git2::Oid,
    ) -> impl Iterator<Item = std::rc::Rc<Commit>> + '_ {
        let mut revwalk = self.repo.revwalk().unwrap();
        revwalk.push(head_id).unwrap();
        revwalk.set_sorting(git2::Sort::TOPOLOGICAL).unwrap();

        revwalk
            .filter_map(Result::ok)
            .filter_map(move |oid| self.find_commit(oid))
    }

    pub fn contains_commit(
        &self,
        haystack_id: git2::Oid,
        needle_id: git2::Oid,
    ) -> Result<bool, git2::Error> {
        let needle_commit = self.repo.find_commit(needle_id)?;
        let needle_ann_commit = self.repo.find_annotated_commit(needle_id)?;
        let haystack_ann_commit = self.repo.find_annotated_commit(haystack_id)?;

        let parent_ann_commit = if 0 < needle_commit.parent_count() {
            let parent_commit = needle_commit.parent(0)?;
            Some(self.repo.find_annotated_commit(parent_commit.id())?)
        } else {
            None
        };

        let mut rebase = self.repo.rebase(
            Some(&needle_ann_commit),
            parent_ann_commit.as_ref(),
            Some(&haystack_ann_commit),
            Some(git2::RebaseOptions::new().inmemory(true)),
        )?;

        if let Some(op) = rebase.next() {
            op.map_err(|e| {
                let _ = rebase.abort();
                e
            })?;
            let inmemory_index = rebase.inmemory_index().unwrap();
            if inmemory_index.has_conflicts() {
                return Ok(false);
            }

            let sig = self.repo.signature().unwrap();
            match rebase.commit(None, &sig, None).map_err(|e| {
                let _ = rebase.abort();
                e
            }) {
                // Created commit, must be unique
                Ok(_) => Ok(false),
                Err(err) => {
                    if err.class() == git2::ErrorClass::Rebase
                        && err.code() == git2::ErrorCode::Applied
                    {
                        return Ok(true);
                    }
                    Err(err)
                }
            }
        } else {
            // No commit created, must exist somehow
            rebase.finish(None)?;
            Ok(true)
        }
    }

    fn cherry_pick(
        &mut self,
        head_id: git2::Oid,
        cherry_id: git2::Oid,
    ) -> Result<git2::Oid, git2::Error> {
        let base_id = self
            .commits_from(cherry_id)
            .filter(|c| c.id != cherry_id)
            .map(|c| c.id)
            .next()
            .unwrap_or(cherry_id);
        if base_id == head_id {
            return Ok(cherry_id);
        }
        let base_ann_commit = self.repo.find_annotated_commit(base_id)?;
        let head_ann_commit = self.repo.find_annotated_commit(head_id)?;
        let cherry_ann_commit = self.repo.find_annotated_commit(cherry_id)?;
        let cherry_commit = self.repo.find_commit(cherry_id)?;
        let mut rebase = self.repo.rebase(
            Some(&cherry_ann_commit),
            Some(&base_ann_commit),
            Some(&head_ann_commit),
            Some(git2::RebaseOptions::new().inmemory(true)),
        )?;

        let mut tip_id = head_id;
        while let Some(op) = rebase.next() {
            op.map_err(|e| {
                let _ = rebase.abort();
                e
            })?;
            let inmemory_index = rebase.inmemory_index().unwrap();
            if inmemory_index.has_conflicts() {
                let conflicts = inmemory_index
                    .conflicts()?
                    .map(|conflict| {
                        let conflict = conflict.unwrap();
                        let our_path = conflict
                            .our
                            .as_ref()
                            .map(|c| bytes2path(&c.path))
                            .or_else(|| conflict.their.as_ref().map(|c| bytes2path(&c.path)))
                            .or_else(|| conflict.ancestor.as_ref().map(|c| bytes2path(&c.path)))
                            .unwrap_or_else(|| std::path::Path::new("<unknown>"));
                        format!("{}", our_path.display())
                    })
                    .join("\n  ");
                return Err(git2::Error::new(
                    git2::ErrorCode::Unmerged,
                    git2::ErrorClass::Index,
                    format!("cherry-pick conflicts:\n  {}\n", conflicts),
                ));
            }

            let mut sig = self.repo.signature()?;
            if let (Some(name), Some(email)) = (sig.name(), sig.email()) {
                // For simple rebases, preserve the original commit time
                sig = git2::Signature::new(name, email, &cherry_commit.time())?.to_owned();
            }
            let commit_id = match rebase.commit(None, &sig, None).map_err(|e| {
                let _ = rebase.abort();
                e
            }) {
                Ok(commit_id) => Ok(commit_id),
                Err(err) => {
                    if err.class() == git2::ErrorClass::Rebase
                        && err.code() == git2::ErrorCode::Applied
                    {
                        log::trace!("Skipping {}, already applied to {}", cherry_id, head_id);
                        return Ok(tip_id);
                    }
                    Err(err)
                }
            }?;
            tip_id = commit_id;
        }
        rebase.finish(None)?;
        Ok(tip_id)
    }

    pub fn squash(
        &mut self,
        head_id: git2::Oid,
        into_id: git2::Oid,
    ) -> Result<git2::Oid, git2::Error> {
        // Based on https://www.pygit2.org/recipes/git-cherry-pick.html
        let head_commit = self.repo.find_commit(head_id)?;
        let head_tree = self.repo.find_tree(head_commit.tree_id())?;

        let base_commit = if 0 < head_commit.parent_count() {
            head_commit.parent(0)?
        } else {
            head_commit.clone()
        };
        let base_tree = self.repo.find_tree(base_commit.tree_id())?;

        let into_commit = self.repo.find_commit(into_id)?;
        let into_tree = self.repo.find_tree(into_commit.tree_id())?;

        let onto_commit;
        let onto_commits;
        let onto_commits: &[&git2::Commit] = if 0 < into_commit.parent_count() {
            onto_commit = into_commit.parent(0)?;
            onto_commits = [&onto_commit];
            &onto_commits
        } else {
            &[]
        };

        let mut result_index = self
            .repo
            .merge_trees(&base_tree, &into_tree, &head_tree, None)?;
        if result_index.has_conflicts() {
            let conflicts = result_index
                .conflicts()?
                .map(|conflict| {
                    let conflict = conflict.unwrap();
                    let our_path = conflict
                        .our
                        .as_ref()
                        .map(|c| bytes2path(&c.path))
                        .or_else(|| conflict.their.as_ref().map(|c| bytes2path(&c.path)))
                        .or_else(|| conflict.ancestor.as_ref().map(|c| bytes2path(&c.path)))
                        .unwrap_or_else(|| std::path::Path::new("<unknown>"));
                    format!("{}", our_path.display())
                })
                .join("\n  ");
            return Err(git2::Error::new(
                git2::ErrorCode::Unmerged,
                git2::ErrorClass::Index,
                format!("squash conflicts:\n  {}\n", conflicts),
            ));
        }
        let result_id = result_index.write_tree_to(&self.repo)?;
        let result_tree = self.repo.find_tree(result_id)?;
        let new_id = self.repo.commit(
            None,
            &into_commit.author(),
            &into_commit.committer(),
            into_commit.message().unwrap(),
            &result_tree,
            onto_commits,
        )?;
        Ok(new_id)
    }

    pub fn stash_push(&mut self, message: Option<&str>) -> Result<git2::Oid, git2::Error> {
        let signature = self.repo.signature()?;
        self.repo.stash_save2(&signature, message, None)
    }

    pub fn stash_pop(&mut self, stash_id: git2::Oid) -> Result<(), git2::Error> {
        let mut index = None;
        self.repo.stash_foreach(|i, _, id| {
            if *id == stash_id {
                index = Some(i);
                false
            } else {
                true
            }
        })?;
        let index = index.ok_or_else(|| {
            git2::Error::new(
                git2::ErrorCode::NotFound,
                git2::ErrorClass::Reference,
                "stash ID not found",
            )
        })?;
        self.repo.stash_pop(index, None)
    }

    pub fn branch(&mut self, name: &str, id: git2::Oid) -> Result<(), git2::Error> {
        let commit = self.repo.find_commit(id)?;
        self.repo.branch(name, &commit, true)?;
        Ok(())
    }

    pub fn delete_branch(&mut self, name: &str) -> Result<(), git2::Error> {
        // HACK: We shouldn't limit ourselves to `Local`
        let mut branch = self.repo.find_branch(name, git2::BranchType::Local)?;
        branch.delete()
    }

    pub fn find_local_branch(&self, name: &str) -> Option<Branch> {
        let branch = self.repo.find_branch(name, git2::BranchType::Local).ok()?;
        let id = branch.get().target().unwrap();

        let push_id = self
            .repo
            .find_branch(
                &format!("{}/{}", self.push_remote(), name),
                git2::BranchType::Remote,
            )
            .ok()
            .and_then(|b| b.get().target());
        let pull_id = self
            .repo
            .find_branch(
                &format!("{}/{}", self.pull_remote(), name),
                git2::BranchType::Remote,
            )
            .ok()
            .and_then(|b| b.get().target());

        Some(Branch {
            name: name.to_owned(),
            id,
            push_id,
            pull_id,
        })
    }

    pub fn local_branches(&self) -> impl Iterator<Item = Branch> + '_ {
        log::trace!("Loading branches");
        self.repo
            .branches(Some(git2::BranchType::Local))
            .into_iter()
            .flatten()
            .flat_map(move |branch| {
                let (branch, _) = branch.ok()?;
                let name = if let Some(name) = branch.name().ok().flatten() {
                    name
                } else {
                    log::debug!(
                        "Ignoring non-UTF8 branch {:?}",
                        branch.name_bytes().unwrap().as_bstr()
                    );
                    return None;
                };
                let id = branch.get().target().unwrap();

                let push_id = self
                    .repo
                    .find_branch(
                        &format!("{}/{}", self.push_remote(), name),
                        git2::BranchType::Remote,
                    )
                    .ok()
                    .and_then(|b| b.get().target());
                let pull_id = self
                    .repo
                    .find_branch(
                        &format!("{}/{}", self.pull_remote(), name),
                        git2::BranchType::Remote,
                    )
                    .ok()
                    .and_then(|b| b.get().target());

                Some(Branch {
                    name: name.to_owned(),
                    id,
                    push_id,
                    pull_id,
                })
            })
    }

    pub fn detach(&mut self) -> Result<(), git2::Error> {
        let head_id = self
            .repo
            .head()
            .unwrap()
            .resolve()
            .unwrap()
            .target()
            .unwrap();
        self.repo.set_head_detached(head_id)?;
        Ok(())
    }

    pub fn switch(&mut self, name: &str) -> Result<(), git2::Error> {
        // HACK: We shouldn't limit ourselves to `Local`
        let branch = self.repo.find_branch(name, git2::BranchType::Local)?;
        self.repo.set_head(branch.get().name().unwrap())?;
        let mut builder = git2::build::CheckoutBuilder::new();
        builder.force();
        self.repo.checkout_head(Some(&mut builder))?;
        Ok(())
    }

    fn intern_string(&self, data: &str) -> std::rc::Rc<str> {
        let mut interned_strings = self.interned_strings.borrow_mut();
        if let Some(interned) = interned_strings.get(data) {
            std::rc::Rc::clone(interned)
        } else {
            let interned = std::rc::Rc::from(data);
            interned_strings.insert(std::rc::Rc::clone(&interned));
            interned
        }
    }
}

impl std::fmt::Debug for GitRepo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        f.debug_struct("GitRepo")
            .field("repo", &self.repo.workdir())
            .field("push_remote", &self.push_remote.as_deref())
            .field("pull_remote", &self.pull_remote.as_deref())
            .finish()
    }
}

impl Repo for GitRepo {
    fn user(&self) -> Option<std::rc::Rc<str>> {
        self.user()
    }

    fn is_dirty(&self) -> bool {
        self.is_dirty()
    }

    fn merge_base(&self, one: git2::Oid, two: git2::Oid) -> Option<git2::Oid> {
        self.merge_base(one, two)
    }

    fn find_commit(&self, id: git2::Oid) -> Option<std::rc::Rc<Commit>> {
        self.find_commit(id)
    }

    fn head_commit(&self) -> std::rc::Rc<Commit> {
        self.head_commit()
    }

    fn head_branch(&self) -> Option<Branch> {
        self.head_branch()
    }

    fn resolve(&self, revspec: &str) -> Option<std::rc::Rc<Commit>> {
        self.resolve(revspec)
    }

    fn commits_from(
        &self,
        head_id: git2::Oid,
    ) -> Box<dyn Iterator<Item = std::rc::Rc<Commit>> + '_> {
        Box::new(self.commits_from(head_id))
    }

    fn contains_commit(
        &self,
        haystack_id: git2::Oid,
        needle_id: git2::Oid,
    ) -> Result<bool, git2::Error> {
        self.contains_commit(haystack_id, needle_id)
    }

    fn cherry_pick(
        &mut self,
        head_id: git2::Oid,
        cherry_id: git2::Oid,
    ) -> Result<git2::Oid, git2::Error> {
        self.cherry_pick(head_id, cherry_id)
    }

    fn squash(&mut self, head_id: git2::Oid, into_id: git2::Oid) -> Result<git2::Oid, git2::Error> {
        self.squash(head_id, into_id)
    }

    fn stash_push(&mut self, message: Option<&str>) -> Result<git2::Oid, git2::Error> {
        self.stash_push(message)
    }

    fn stash_pop(&mut self, stash_id: git2::Oid) -> Result<(), git2::Error> {
        self.stash_pop(stash_id)
    }

    fn branch(&mut self, name: &str, id: git2::Oid) -> Result<(), git2::Error> {
        self.branch(name, id)
    }

    fn delete_branch(&mut self, name: &str) -> Result<(), git2::Error> {
        self.delete_branch(name)
    }

    fn find_local_branch(&self, name: &str) -> Option<Branch> {
        self.find_local_branch(name)
    }

    fn local_branches(&self) -> Box<dyn Iterator<Item = Branch> + '_> {
        Box::new(self.local_branches())
    }

    fn detach(&mut self) -> Result<(), git2::Error> {
        self.detach()
    }

    fn switch(&mut self, name: &str) -> Result<(), git2::Error> {
        self.switch(name)
    }
}

#[derive(Debug)]
pub struct InMemoryRepo {
    commits: std::collections::HashMap<git2::Oid, (Option<git2::Oid>, std::rc::Rc<Commit>)>,
    branches: std::collections::HashMap<String, Branch>,
    head_id: Option<git2::Oid>,

    last_id: std::sync::atomic::AtomicUsize,
}

impl InMemoryRepo {
    pub fn new() -> Self {
        Self {
            commits: Default::default(),
            branches: Default::default(),
            head_id: Default::default(),
            last_id: std::sync::atomic::AtomicUsize::new(1),
        }
    }

    pub fn clear(&mut self) {
        *self = InMemoryRepo::new()
    }

    pub fn gen_id(&mut self) -> git2::Oid {
        let last_id = self
            .last_id
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let sha = format!("{:040x}", last_id);
        git2::Oid::from_str(&sha).unwrap()
    }

    pub fn push_commit(&mut self, parent_id: Option<git2::Oid>, commit: Commit) {
        if let Some(parent_id) = parent_id {
            assert!(self.commits.contains_key(&parent_id));
        }
        self.head_id = Some(commit.id);
        self.commits
            .insert(commit.id, (parent_id, std::rc::Rc::new(commit)));
    }

    pub fn head_id(&mut self) -> Option<git2::Oid> {
        self.head_id
    }

    pub fn set_head(&mut self, head_id: git2::Oid) {
        assert!(self.commits.contains_key(&head_id));
        self.head_id = Some(head_id)
    }

    pub fn mark_branch(&mut self, branch: Branch) {
        assert!(self.commits.contains_key(&branch.id));
        self.branches.insert(branch.name.clone(), branch);
    }

    fn user(&self) -> Option<std::rc::Rc<str>> {
        None
    }

    pub fn is_dirty(&self) -> bool {
        false
    }

    pub fn merge_base(&self, one: git2::Oid, two: git2::Oid) -> Option<git2::Oid> {
        let one_ancestors: Vec<_> = self.commits_from(one).collect();
        self.commits_from(two)
            .filter(|two_ancestor| one_ancestors.contains(two_ancestor))
            .map(|c| c.id)
            .next()
    }

    pub fn find_commit(&self, id: git2::Oid) -> Option<std::rc::Rc<Commit>> {
        self.commits.get(&id).map(|c| c.1.clone())
    }

    pub fn head_commit(&self) -> std::rc::Rc<Commit> {
        self.commits.get(&self.head_id.unwrap()).cloned().unwrap().1
    }

    pub fn head_branch(&self) -> Option<Branch> {
        self.branches
            .values()
            .find(|b| b.id == self.head_id.unwrap())
            .cloned()
    }

    pub fn resolve(&self, revspec: &str) -> Option<std::rc::Rc<Commit>> {
        let branch = self.branches.get(revspec)?;
        self.find_commit(branch.id)
    }

    pub fn commits_from(
        &self,
        head_id: git2::Oid,
    ) -> impl Iterator<Item = std::rc::Rc<Commit>> + '_ {
        let next = self.commits.get(&head_id).cloned();
        CommitsFrom {
            commits: &self.commits,
            next,
        }
    }

    pub fn contains_commit(
        &self,
        haystack_id: git2::Oid,
        needle_id: git2::Oid,
    ) -> Result<bool, git2::Error> {
        // Because we don't have the information for likeness matches, just checking for Oid
        let mut next = Some(haystack_id);
        while let Some(current) = next {
            if current == needle_id {
                return Ok(true);
            }
            next = self.commits.get(&current).and_then(|c| c.0);
        }
        Ok(false)
    }

    pub fn cherry_pick(
        &mut self,
        head_id: git2::Oid,
        cherry_id: git2::Oid,
    ) -> Result<git2::Oid, git2::Error> {
        let cherry_commit = self.find_commit(cherry_id).ok_or_else(|| {
            git2::Error::new(
                git2::ErrorCode::NotFound,
                git2::ErrorClass::Reference,
                format!("could not find commit {:?}", cherry_id),
            )
        })?;
        let mut cherry_commit = Commit::clone(&cherry_commit);
        let new_id = self.gen_id();
        cherry_commit.id = new_id;
        self.commits
            .insert(new_id, (Some(head_id), std::rc::Rc::new(cherry_commit)));
        Ok(new_id)
    }

    pub fn squash(
        &mut self,
        head_id: git2::Oid,
        into_id: git2::Oid,
    ) -> Result<git2::Oid, git2::Error> {
        self.commits.get(&head_id).cloned().ok_or_else(|| {
            git2::Error::new(
                git2::ErrorCode::NotFound,
                git2::ErrorClass::Reference,
                format!("could not find commit {:?}", head_id),
            )
        })?;
        let (intos_parent, into_commit) = self.commits.get(&into_id).cloned().ok_or_else(|| {
            git2::Error::new(
                git2::ErrorCode::NotFound,
                git2::ErrorClass::Reference,
                format!("could not find commit {:?}", into_id),
            )
        })?;
        let intos_parent = intos_parent.unwrap();

        let mut squashed_commit = Commit::clone(&into_commit);
        let new_id = self.gen_id();
        squashed_commit.id = new_id;
        self.commits.insert(
            new_id,
            (Some(intos_parent), std::rc::Rc::new(squashed_commit)),
        );
        Ok(new_id)
    }

    pub fn stash_push(&mut self, _message: Option<&str>) -> Result<git2::Oid, git2::Error> {
        Err(git2::Error::new(
            git2::ErrorCode::NotFound,
            git2::ErrorClass::Reference,
            "stash is unsupported",
        ))
    }

    pub fn stash_pop(&mut self, _stash_id: git2::Oid) -> Result<(), git2::Error> {
        Err(git2::Error::new(
            git2::ErrorCode::NotFound,
            git2::ErrorClass::Reference,
            "stash is unsupported",
        ))
    }

    pub fn branch(&mut self, name: &str, id: git2::Oid) -> Result<(), git2::Error> {
        self.branches.insert(
            name.to_owned(),
            Branch {
                name: name.to_owned(),
                id,
                push_id: None,
                pull_id: None,
            },
        );
        Ok(())
    }

    pub fn delete_branch(&mut self, name: &str) -> Result<(), git2::Error> {
        self.branches.remove(name).map(|_| ()).ok_or_else(|| {
            git2::Error::new(
                git2::ErrorCode::NotFound,
                git2::ErrorClass::Reference,
                format!("could not remove branch {:?}", name),
            )
        })
    }

    pub fn find_local_branch(&self, name: &str) -> Option<Branch> {
        self.branches.get(name).cloned()
    }

    pub fn local_branches(&self) -> impl Iterator<Item = Branch> + '_ {
        self.branches.values().cloned()
    }

    pub fn detach(&mut self) -> Result<(), git2::Error> {
        Ok(())
    }

    pub fn switch(&mut self, name: &str) -> Result<(), git2::Error> {
        let branch = self.find_local_branch(name).ok_or_else(|| {
            git2::Error::new(
                git2::ErrorCode::NotFound,
                git2::ErrorClass::Reference,
                format!("could not find branch {:?}", name),
            )
        })?;
        self.head_id = Some(branch.id);
        Ok(())
    }
}

impl Default for InMemoryRepo {
    fn default() -> Self {
        Self::new()
    }
}

struct CommitsFrom<'c> {
    commits: &'c std::collections::HashMap<git2::Oid, (Option<git2::Oid>, std::rc::Rc<Commit>)>,
    next: Option<(Option<git2::Oid>, std::rc::Rc<Commit>)>,
}

impl<'c> Iterator for CommitsFrom<'c> {
    type Item = std::rc::Rc<Commit>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut current = None;
        std::mem::swap(&mut current, &mut self.next);
        let current = current?;
        if let Some(parent_id) = current.0 {
            self.next = self.commits.get(&parent_id).cloned();
        }
        Some(current.1)
    }
}

impl Repo for InMemoryRepo {
    fn user(&self) -> Option<std::rc::Rc<str>> {
        self.user()
    }

    fn is_dirty(&self) -> bool {
        self.is_dirty()
    }

    fn merge_base(&self, one: git2::Oid, two: git2::Oid) -> Option<git2::Oid> {
        self.merge_base(one, two)
    }

    fn find_commit(&self, id: git2::Oid) -> Option<std::rc::Rc<Commit>> {
        self.find_commit(id)
    }

    fn head_commit(&self) -> std::rc::Rc<Commit> {
        self.head_commit()
    }

    fn resolve(&self, revspec: &str) -> Option<std::rc::Rc<Commit>> {
        self.resolve(revspec)
    }

    fn commits_from(
        &self,
        head_id: git2::Oid,
    ) -> Box<dyn Iterator<Item = std::rc::Rc<Commit>> + '_> {
        Box::new(self.commits_from(head_id))
    }

    fn contains_commit(
        &self,
        haystack_id: git2::Oid,
        needle_id: git2::Oid,
    ) -> Result<bool, git2::Error> {
        self.contains_commit(haystack_id, needle_id)
    }

    fn cherry_pick(
        &mut self,
        head_id: git2::Oid,
        cherry_id: git2::Oid,
    ) -> Result<git2::Oid, git2::Error> {
        self.cherry_pick(head_id, cherry_id)
    }

    fn squash(&mut self, head_id: git2::Oid, into_id: git2::Oid) -> Result<git2::Oid, git2::Error> {
        self.squash(head_id, into_id)
    }

    fn head_branch(&self) -> Option<Branch> {
        self.head_branch()
    }

    fn stash_push(&mut self, message: Option<&str>) -> Result<git2::Oid, git2::Error> {
        self.stash_push(message)
    }

    fn stash_pop(&mut self, stash_id: git2::Oid) -> Result<(), git2::Error> {
        self.stash_pop(stash_id)
    }

    fn branch(&mut self, name: &str, id: git2::Oid) -> Result<(), git2::Error> {
        self.branch(name, id)
    }

    fn delete_branch(&mut self, name: &str) -> Result<(), git2::Error> {
        self.delete_branch(name)
    }

    fn find_local_branch(&self, name: &str) -> Option<Branch> {
        self.find_local_branch(name)
    }

    fn local_branches(&self) -> Box<dyn Iterator<Item = Branch> + '_> {
        Box::new(self.local_branches())
    }

    fn detach(&mut self) -> Result<(), git2::Error> {
        self.detach()
    }

    fn switch(&mut self, name: &str) -> Result<(), git2::Error> {
        self.switch(name)
    }
}

// From git2 crate
#[cfg(unix)]
fn bytes2path(b: &[u8]) -> &std::path::Path {
    use std::os::unix::prelude::*;
    std::path::Path::new(std::ffi::OsStr::from_bytes(b))
}

// From git2 crate
#[cfg(windows)]
fn bytes2path(b: &[u8]) -> &std::path::Path {
    use std::str;
    std::path::Path::new(str::from_utf8(b).unwrap())
}

pub fn stash_push(repo: &mut dyn Repo, context: &str) -> Option<git2::Oid> {
    let branch = repo.head_branch();
    let stash_msg = format!(
        "WIP on {} ({})",
        branch.as_ref().map(|b| b.name.as_str()).unwrap_or("HEAD"),
        context
    );
    match repo.stash_push(Some(&stash_msg)) {
        Ok(stash_id) => {
            log::info!(
                "Saved working directory and index state {}: {}",
                stash_msg,
                stash_id
            );
            Some(stash_id)
        }
        Err(err) => {
            log::debug!("Failed to stash: {}", err);
            None
        }
    }
}

pub fn stash_pop(repo: &mut dyn Repo, stash_id: Option<git2::Oid>) {
    if let Some(stash_id) = stash_id {
        match repo.stash_pop(stash_id) {
            Ok(()) => {
                log::info!("Dropped refs/stash {}", stash_id);
            }
            Err(err) => {
                log::error!("Failed to pop {} from stash: {}", stash_id, err);
            }
        }
    }
}
