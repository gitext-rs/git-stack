use bstr::ByteSlice;
use itertools::Itertools;

pub type Error = git2::Error;
pub type Result<T, E = Error> = std::result::Result<T, E>;

pub trait Repo {
    fn path(&self) -> Option<&std::path::Path>;
    fn user(&self) -> Option<std::rc::Rc<str>>;
    fn push_remote(&self) -> &str;
    fn pull_remote(&self) -> &str;

    fn is_dirty(&self) -> bool;
    fn merge_base(&self, one: git2::Oid, two: git2::Oid) -> Option<git2::Oid>;

    fn find_commit(&self, id: git2::Oid) -> Option<std::rc::Rc<Commit>>;
    fn head_commit(&self) -> std::rc::Rc<Commit>;
    fn head_branch(&self) -> Option<Branch>;
    fn resolve(&self, revspec: &str) -> Option<std::rc::Rc<Commit>>;
    fn parent_ids(&self, head_id: git2::Oid) -> Result<Vec<git2::Oid>>;
    fn commit_count(&self, base_id: git2::Oid, head_id: git2::Oid) -> Option<usize>;
    fn commit_range(
        &self,
        base_bound: std::ops::Bound<&git2::Oid>,
        head_bound: std::ops::Bound<&git2::Oid>,
    ) -> Result<Vec<git2::Oid>>;
    fn contains_commit(&self, haystack_id: git2::Oid, needle_id: git2::Oid) -> Result<bool>;
    fn cherry_pick(&mut self, head_id: git2::Oid, cherry_id: git2::Oid) -> Result<git2::Oid>;
    fn reword(&mut self, head_oid: git2::Oid, msg: &str) -> Result<git2::Oid>;
    fn squash(&mut self, head_id: git2::Oid, into_id: git2::Oid) -> Result<git2::Oid>;

    fn stash_push(&mut self, message: Option<&str>) -> Result<git2::Oid>;
    fn stash_pop(&mut self, stash_id: git2::Oid) -> Result<()>;

    fn branch(&mut self, name: &str, id: git2::Oid) -> Result<()>;
    fn delete_branch(&mut self, name: &str) -> Result<()>;
    fn find_local_branch(&self, name: &str) -> Option<Branch>;
    fn find_remote_branch(&self, remote: &str, name: &str) -> Option<Branch>;
    fn local_branches(&self) -> Box<dyn Iterator<Item = Branch> + '_>;
    fn remote_branches(&self) -> Box<dyn Iterator<Item = Branch> + '_>;
    fn detach(&mut self) -> Result<()>;
    fn switch_branch(&mut self, name: &str) -> Result<()>;
    fn switch_commit(&mut self, id: git2::Oid) -> Result<()>;
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Branch {
    pub remote: Option<String>,
    pub name: String,
    pub id: git2::Oid,
}

impl Branch {
    pub fn local_name(&self) -> Option<&str> {
        self.remote.is_none().then_some(self.name.as_str())
    }
}

impl std::fmt::Display for Branch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(remote) = self.remote.as_deref() {
            write!(f, "{}/{}", remote, self.name.as_str())
        } else {
            write!(f, "{}", self.name.as_str())
        }
    }
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
            WIP_PREFIXES.iter().find_map(|prefix| {
                self.summary
                    .strip_prefix(*prefix)
                    .map(ByteSlice::trim)
                    .map(ByteSlice::as_bstr)
            })
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
    sign: Option<git2_ext::ops::UserSign>,
    push_remote: Option<String>,
    pull_remote: Option<String>,
    commits: std::cell::RefCell<std::collections::HashMap<git2::Oid, std::rc::Rc<Commit>>>,
    interned_strings: std::cell::RefCell<std::collections::HashSet<std::rc::Rc<str>>>,
    bases: std::cell::RefCell<std::collections::HashMap<(git2::Oid, git2::Oid), Option<git2::Oid>>>,
    counts: std::cell::RefCell<std::collections::HashMap<(git2::Oid, git2::Oid), Option<usize>>>,
}

impl GitRepo {
    pub fn new(repo: git2::Repository) -> Self {
        Self {
            repo,
            sign: None,
            push_remote: None,
            pull_remote: None,
            commits: Default::default(),
            interned_strings: Default::default(),
            bases: Default::default(),
            counts: Default::default(),
        }
    }

    pub fn set_sign(&mut self, yes: bool) -> Result<(), git2::Error> {
        if yes {
            let config = self.repo.config()?;
            let sign = git2_ext::ops::UserSign::from_config(&self.repo, &config)?;
            self.sign = Some(sign);
        } else {
            self.sign = None;
        }
        Ok(())
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
        let status = self
            .repo
            .statuses(Some(git2::StatusOptions::new().include_ignored(false)))
            .unwrap_or_else(|e| panic!("Unexpected git2 error: {e}"));
        if status.is_empty() {
            false
        } else {
            log::trace!(
                "Repository is dirty: {}",
                status
                    .iter()
                    .filter_map(|s| s.path().map(|s| s.to_owned()))
                    .join(", ")
            );
            true
        }
    }

    pub fn merge_base(&self, one: git2::Oid, two: git2::Oid) -> Option<git2::Oid> {
        if one == two {
            return Some(one);
        }

        let (smaller, larger) = if one < two { (one, two) } else { (two, one) };
        *self
            .bases
            .borrow_mut()
            .entry((smaller, larger))
            .or_insert_with(|| self.merge_base_raw(smaller, larger))
    }

    fn merge_base_raw(&self, one: git2::Oid, two: git2::Oid) -> Option<git2::Oid> {
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
            .unwrap_or_else(|e| panic!("Unexpected git2 error: {e}"))
            .resolve()
            .unwrap_or_else(|e| panic!("Unexpected git2 error: {e}"))
            .target()
            .unwrap();
        self.find_commit(head_id).unwrap()
    }

    pub fn head_branch(&self) -> Option<Branch> {
        if self.repo.head_detached().unwrap_or(true) {
            return None;
        }

        let resolved = self
            .repo
            .head()
            .unwrap_or_else(|e| panic!("Unexpected git2 error: {e}"))
            .resolve()
            .unwrap_or_else(|e| panic!("Unexpected git2 error: {e}"));
        let name = resolved.shorthand()?;
        let id = resolved.target()?;

        Some(Branch {
            remote: None,
            name: name.to_owned(),
            id,
        })
    }

    pub fn resolve(&self, revspec: &str) -> Option<std::rc::Rc<Commit>> {
        let id = self.repo.revparse_single(revspec).ok()?.id();
        self.find_commit(id)
    }

    pub fn parent_ids(&self, head_id: git2::Oid) -> Result<Vec<git2::Oid>> {
        let commit = self.repo.find_commit(head_id)?;
        Ok(commit.parent_ids().collect())
    }

    pub fn commit_count(&self, base_id: git2::Oid, head_id: git2::Oid) -> Option<usize> {
        if base_id == head_id {
            return Some(0);
        }

        *self
            .counts
            .borrow_mut()
            .entry((base_id, head_id))
            .or_insert_with(|| self.commit_count_raw(base_id, head_id))
    }

    fn commit_count_raw(&self, base_id: git2::Oid, head_id: git2::Oid) -> Option<usize> {
        let merge_base_id = self.merge_base(base_id, head_id)?;
        if merge_base_id != base_id {
            return None;
        }
        let mut revwalk = self
            .repo
            .revwalk()
            .unwrap_or_else(|e| panic!("Unexpected git2 error: {e}"));
        revwalk
            .push(head_id)
            .unwrap_or_else(|e| panic!("Unexpected git2 error: {e}"));
        revwalk
            .hide(base_id)
            .unwrap_or_else(|e| panic!("Unexpected git2 error: {e}"));
        Some(revwalk.count())
    }

    pub fn commit_range(
        &self,
        base_bound: std::ops::Bound<&git2::Oid>,
        head_bound: std::ops::Bound<&git2::Oid>,
    ) -> Result<Vec<git2::Oid>> {
        let head_id = match head_bound {
            std::ops::Bound::Included(head_id) | std::ops::Bound::Excluded(head_id) => *head_id,
            std::ops::Bound::Unbounded => panic!("commit_range's HEAD cannot be unbounded"),
        };
        let skip = if matches!(head_bound, std::ops::Bound::Included(_)) {
            0
        } else {
            1
        };

        let base_id = match base_bound {
            std::ops::Bound::Included(base_id) | std::ops::Bound::Excluded(base_id) => {
                debug_assert_eq!(self.merge_base(*base_id, head_id), Some(*base_id));
                Some(*base_id)
            }
            std::ops::Bound::Unbounded => None,
        };

        let mut revwalk = self.repo.revwalk()?;
        revwalk.push(head_id)?;
        if let Some(base_id) = base_id {
            revwalk.hide(base_id)?;
        }
        revwalk.set_sorting(git2::Sort::TOPOLOGICAL)?;
        let mut result = revwalk
            .filter_map(Result::ok)
            .skip(skip)
            .take_while(|id| Some(*id) != base_id)
            .collect::<Vec<_>>();
        if let std::ops::Bound::Included(base_id) = base_bound {
            result.push(*base_id);
        }
        Ok(result)
    }

    pub fn contains_commit(&self, haystack_id: git2::Oid, needle_id: git2::Oid) -> Result<bool> {
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
            let inmemory_index = rebase
                .inmemory_index()
                .unwrap_or_else(|e| panic!("Unexpected git2 error: {e}"));
            if inmemory_index.has_conflicts() {
                return Ok(false);
            }

            let sig = self
                .repo
                .signature()
                .unwrap_or_else(|e| panic!("Unexpected git2 error: {e}"));
            let result = rebase.commit(None, &sig, None).map_err(|e| {
                let _ = rebase.abort();
                e
            });
            match result {
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

    pub fn cherry_pick(&mut self, head_id: git2::Oid, cherry_id: git2::Oid) -> Result<git2::Oid> {
        git2_ext::ops::cherry_pick(
            &self.repo,
            head_id,
            cherry_id,
            self.sign.as_ref().map(|s| s as &dyn git2_ext::ops::Sign),
        )
    }

    pub fn reword(&mut self, head_oid: git2::Oid, msg: &str) -> Result<git2::Oid> {
        git2_ext::ops::reword(
            &self.repo,
            head_oid,
            msg,
            self.sign.as_ref().map(|s| s as &dyn git2_ext::ops::Sign),
        )
    }

    pub fn squash(&mut self, head_id: git2::Oid, into_id: git2::Oid) -> Result<git2::Oid> {
        git2_ext::ops::squash(
            &self.repo,
            head_id,
            into_id,
            self.sign.as_ref().map(|s| s as &dyn git2_ext::ops::Sign),
        )
    }

    pub fn stash_push(&mut self, message: Option<&str>) -> Result<git2::Oid> {
        let signature = self.repo.signature()?;
        self.repo.stash_save2(&signature, message, None)
    }

    pub fn stash_pop(&mut self, stash_id: git2::Oid) -> Result<()> {
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
            Error::new(
                git2::ErrorCode::NotFound,
                git2::ErrorClass::Reference,
                "stash ID not found",
            )
        })?;
        self.repo.stash_pop(index, None)
    }

    pub fn branch(&mut self, name: &str, id: git2::Oid) -> Result<()> {
        let commit = self.repo.find_commit(id)?;
        self.repo.branch(name, &commit, true)?;
        Ok(())
    }

    pub fn delete_branch(&mut self, name: &str) -> Result<()> {
        // HACK: We shouldn't limit ourselves to `Local`
        let mut branch = self.repo.find_branch(name, git2::BranchType::Local)?;
        branch.delete()
    }

    pub fn find_local_branch(&self, name: &str) -> Option<Branch> {
        let branch = self.repo.find_branch(name, git2::BranchType::Local).ok()?;
        self.load_local_branch(&branch, name).ok()
    }

    pub fn find_remote_branch(&self, remote: &str, name: &str) -> Option<Branch> {
        let qualified = format!("{remote}/{name}");
        let branch = self
            .repo
            .find_branch(&qualified, git2::BranchType::Remote)
            .ok()?;
        self.load_remote_branch(&branch, remote, name).ok()
    }

    pub fn local_branches(&self) -> impl Iterator<Item = Branch> + '_ {
        log::trace!("Loading local branches");
        self.repo
            .branches(Some(git2::BranchType::Local))
            .into_iter()
            .flatten()
            .filter_map(move |branch| {
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
                self.load_local_branch(&branch, name).ok()
            })
    }

    pub fn remote_branches(&self) -> impl Iterator<Item = Branch> + '_ {
        log::trace!("Loading remote branches");
        self.repo
            .branches(Some(git2::BranchType::Remote))
            .into_iter()
            .flatten()
            .filter_map(move |branch| {
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
                let (remote, name) = name.split_once('/').unwrap();
                self.load_remote_branch(&branch, remote, name).ok()
            })
    }

    fn load_local_branch(&self, branch: &git2::Branch<'_>, name: &str) -> Result<Branch> {
        let id = branch.get().target().unwrap();

        Ok(Branch {
            remote: None,
            name: name.to_owned(),
            id,
        })
    }

    fn load_remote_branch(
        &self,
        branch: &git2::Branch<'_>,
        remote: &str,
        name: &str,
    ) -> Result<Branch> {
        let id = branch.get().target().unwrap();

        Ok(Branch {
            remote: Some(remote.to_owned()),
            name: name.to_owned(),
            id,
        })
    }

    pub fn detach(&mut self) -> Result<()> {
        let head_id = self
            .repo
            .head()
            .unwrap_or_else(|e| panic!("Unexpected git2 error: {e}"))
            .resolve()
            .unwrap_or_else(|e| panic!("Unexpected git2 error: {e}"))
            .target()
            .unwrap();
        self.repo.set_head_detached(head_id)?;
        Ok(())
    }

    pub fn switch_branch(&mut self, name: &str) -> Result<()> {
        let head_tree_id = self.head_commit().tree_id;
        let branch = self.repo.find_branch(name, git2::BranchType::Local)?;
        let target_id = branch.get().target().unwrap();
        let target_tree_id = self
            .find_commit(target_id)
            .map(|c| c.tree_id)
            .unwrap_or_else(git2::Oid::zero);

        self.repo.set_head(branch.get().name().unwrap())?;

        if head_tree_id != target_tree_id {
            let mut builder = git2::build::CheckoutBuilder::new();
            builder.force();
            self.repo.checkout_head(Some(&mut builder))?;
        }

        Ok(())
    }

    pub fn switch_commit(&mut self, id: git2::Oid) -> Result<()> {
        let head_tree_id = self.head_commit().tree_id;
        let target_tree_id = self
            .find_commit(id)
            .map(|c| c.tree_id)
            .unwrap_or_else(git2::Oid::zero);

        self.repo.set_head_detached(id)?;

        if head_tree_id != target_tree_id {
            let mut builder = git2::build::CheckoutBuilder::new();
            builder.force();
            self.repo.checkout_head(Some(&mut builder))?;
        }
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
    fn path(&self) -> Option<&std::path::Path> {
        Some(self.repo.path())
    }
    fn user(&self) -> Option<std::rc::Rc<str>> {
        self.user()
    }
    fn push_remote(&self) -> &str {
        self.push_remote()
    }
    fn pull_remote(&self) -> &str {
        self.pull_remote()
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

    fn parent_ids(&self, head_id: git2::Oid) -> Result<Vec<git2::Oid>> {
        self.parent_ids(head_id)
    }

    fn commit_count(&self, base_id: git2::Oid, head_id: git2::Oid) -> Option<usize> {
        self.commit_count(base_id, head_id)
    }

    fn commit_range(
        &self,
        base_bound: std::ops::Bound<&git2::Oid>,
        head_bound: std::ops::Bound<&git2::Oid>,
    ) -> Result<Vec<git2::Oid>> {
        self.commit_range(base_bound, head_bound)
    }

    fn contains_commit(&self, haystack_id: git2::Oid, needle_id: git2::Oid) -> Result<bool> {
        self.contains_commit(haystack_id, needle_id)
    }

    fn cherry_pick(&mut self, head_id: git2::Oid, cherry_id: git2::Oid) -> Result<git2::Oid> {
        self.cherry_pick(head_id, cherry_id)
    }

    fn reword(&mut self, head_oid: git2::Oid, msg: &str) -> Result<git2::Oid> {
        self.reword(head_oid, msg)
    }

    fn squash(&mut self, head_id: git2::Oid, into_id: git2::Oid) -> Result<git2::Oid> {
        self.squash(head_id, into_id)
    }

    fn stash_push(&mut self, message: Option<&str>) -> Result<git2::Oid> {
        self.stash_push(message)
    }

    fn stash_pop(&mut self, stash_id: git2::Oid) -> Result<()> {
        self.stash_pop(stash_id)
    }

    fn branch(&mut self, name: &str, id: git2::Oid) -> Result<()> {
        self.branch(name, id)
    }

    fn delete_branch(&mut self, name: &str) -> Result<()> {
        self.delete_branch(name)
    }

    fn find_local_branch(&self, name: &str) -> Option<Branch> {
        self.find_local_branch(name)
    }

    fn find_remote_branch(&self, remote: &str, name: &str) -> Option<Branch> {
        self.find_remote_branch(remote, name)
    }

    fn local_branches(&self) -> Box<dyn Iterator<Item = Branch> + '_> {
        Box::new(self.local_branches())
    }

    fn remote_branches(&self) -> Box<dyn Iterator<Item = Branch> + '_> {
        Box::new(self.remote_branches())
    }

    fn detach(&mut self) -> Result<()> {
        self.detach()
    }

    fn switch_branch(&mut self, name: &str) -> Result<()> {
        self.switch_branch(name)
    }

    fn switch_commit(&mut self, id: git2::Oid) -> Result<()> {
        self.switch_commit(id)
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
        *self = InMemoryRepo::new();
    }

    pub fn gen_id(&mut self) -> git2::Oid {
        let last_id = self
            .last_id
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let sha = format!("{last_id:040x}");
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
        self.head_id = Some(head_id);
    }

    pub fn mark_branch(&mut self, branch: Branch) {
        assert!(self.commits.contains_key(&branch.id));
        self.branches.insert(branch.name.clone(), branch);
    }

    pub fn push_remote(&self) -> &str {
        "origin"
    }

    pub fn pull_remote(&self) -> &str {
        "origin"
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

    pub fn parent_ids(&self, head_id: git2::Oid) -> Result<Vec<git2::Oid>> {
        let next = self
            .commits
            .get(&head_id)
            .and_then(|(parent, _commit)| *parent);
        Ok(next.into_iter().collect())
    }

    fn commits_from(&self, head_id: git2::Oid) -> impl Iterator<Item = std::rc::Rc<Commit>> + '_ {
        let next = self.commits.get(&head_id).cloned();
        CommitsFrom {
            commits: &self.commits,
            next,
        }
    }

    pub fn commit_count(&self, base_id: git2::Oid, head_id: git2::Oid) -> Option<usize> {
        let merge_base_id = self.merge_base(base_id, head_id)?;
        let count = self
            .commits_from(head_id)
            .take_while(move |cur_id| cur_id.id != merge_base_id)
            .count();
        Some(count)
    }

    pub fn commit_range(
        &self,
        base_bound: std::ops::Bound<&git2::Oid>,
        head_bound: std::ops::Bound<&git2::Oid>,
    ) -> Result<Vec<git2::Oid>> {
        let head_id = match head_bound {
            std::ops::Bound::Included(head_id) | std::ops::Bound::Excluded(head_id) => *head_id,
            std::ops::Bound::Unbounded => panic!("commit_range's HEAD cannot be unbounded"),
        };
        let skip = if matches!(head_bound, std::ops::Bound::Included(_)) {
            0
        } else {
            1
        };

        let base_id = match base_bound {
            std::ops::Bound::Included(base_id) | std::ops::Bound::Excluded(base_id) => {
                debug_assert_eq!(self.merge_base(*base_id, head_id), Some(*base_id));
                Some(*base_id)
            }
            std::ops::Bound::Unbounded => None,
        };

        let mut result = self
            .commits_from(head_id)
            .skip(skip)
            .map(|commit| commit.id)
            .take_while(|id| Some(*id) != base_id)
            .collect::<Vec<_>>();
        if let std::ops::Bound::Included(base_id) = base_bound {
            result.push(*base_id);
        }
        Ok(result)
    }

    pub fn contains_commit(&self, haystack_id: git2::Oid, needle_id: git2::Oid) -> Result<bool> {
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

    pub fn cherry_pick(&mut self, head_id: git2::Oid, cherry_id: git2::Oid) -> Result<git2::Oid> {
        let cherry_commit = self.find_commit(cherry_id).ok_or_else(|| {
            Error::new(
                git2::ErrorCode::NotFound,
                git2::ErrorClass::Reference,
                format!("could not find commit {cherry_id:?}"),
            )
        })?;
        let mut cherry_commit = Commit::clone(&cherry_commit);
        let new_id = self.gen_id();
        cherry_commit.id = new_id;
        self.commits
            .insert(new_id, (Some(head_id), std::rc::Rc::new(cherry_commit)));
        Ok(new_id)
    }

    pub fn reword(&mut self, head_id: git2::Oid, msg: &str) -> Result<git2::Oid> {
        let (head_parent, head_commit) = self.commits.get(&head_id).cloned().ok_or_else(|| {
            git2::Error::new(
                git2::ErrorCode::NotFound,
                git2::ErrorClass::Reference,
                format!("could not find commit {head_id:?}"),
            )
        })?;

        let mut reworded_commit = Commit::clone(&head_commit);
        let new_id = self.gen_id();
        reworded_commit.id = new_id;
        reworded_commit.summary = msg.into();
        self.commits
            .insert(new_id, (head_parent, std::rc::Rc::new(reworded_commit)));
        Ok(new_id)
    }

    pub fn squash(&mut self, head_id: git2::Oid, into_id: git2::Oid) -> Result<git2::Oid> {
        self.commits.get(&head_id).cloned().ok_or_else(|| {
            Error::new(
                git2::ErrorCode::NotFound,
                git2::ErrorClass::Reference,
                format!("could not find commit {head_id:?}"),
            )
        })?;
        let (intos_parent, into_commit) = self.commits.get(&into_id).cloned().ok_or_else(|| {
            Error::new(
                git2::ErrorCode::NotFound,
                git2::ErrorClass::Reference,
                format!("could not find commit {into_id:?}"),
            )
        })?;

        let mut squashed_commit = Commit::clone(&into_commit);
        let new_id = self.gen_id();
        squashed_commit.id = new_id;
        self.commits
            .insert(new_id, (intos_parent, std::rc::Rc::new(squashed_commit)));
        Ok(new_id)
    }

    pub fn stash_push(&mut self, _message: Option<&str>) -> Result<git2::Oid> {
        Err(Error::new(
            git2::ErrorCode::NotFound,
            git2::ErrorClass::Reference,
            "stash is unsupported",
        ))
    }

    pub fn stash_pop(&mut self, _stash_id: git2::Oid) -> Result<()> {
        Err(Error::new(
            git2::ErrorCode::NotFound,
            git2::ErrorClass::Reference,
            "stash is unsupported",
        ))
    }

    pub fn branch(&mut self, name: &str, id: git2::Oid) -> Result<()> {
        self.branches.insert(
            name.to_owned(),
            Branch {
                remote: None,
                name: name.to_owned(),
                id,
            },
        );
        Ok(())
    }

    pub fn delete_branch(&mut self, name: &str) -> Result<()> {
        self.branches.remove(name).map(|_| ()).ok_or_else(|| {
            Error::new(
                git2::ErrorCode::NotFound,
                git2::ErrorClass::Reference,
                format!("could not remove branch {name:?}"),
            )
        })
    }

    pub fn find_local_branch(&self, name: &str) -> Option<Branch> {
        self.branches.get(name).cloned()
    }

    pub fn find_remote_branch(&self, _remote: &str, _name: &str) -> Option<Branch> {
        None
    }

    pub fn local_branches(&self) -> impl Iterator<Item = Branch> + '_ {
        self.branches.values().cloned()
    }

    pub fn remote_branches(&self) -> impl Iterator<Item = Branch> + '_ {
        None.into_iter()
    }

    pub fn detach(&mut self) -> Result<()> {
        Ok(())
    }

    pub fn switch_branch(&mut self, name: &str) -> Result<()> {
        let branch = self.find_local_branch(name).ok_or_else(|| {
            Error::new(
                git2::ErrorCode::NotFound,
                git2::ErrorClass::Reference,
                format!("could not find branch {name:?}"),
            )
        })?;
        self.head_id = Some(branch.id);
        Ok(())
    }

    pub fn switch_commit(&mut self, id: git2::Oid) -> Result<()> {
        self.head_id = Some(id);
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

impl Iterator for CommitsFrom<'_> {
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
    fn path(&self) -> Option<&std::path::Path> {
        None
    }
    fn user(&self) -> Option<std::rc::Rc<str>> {
        self.user()
    }
    fn push_remote(&self) -> &str {
        self.push_remote()
    }
    fn pull_remote(&self) -> &str {
        self.pull_remote()
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

    fn parent_ids(&self, head_id: git2::Oid) -> Result<Vec<git2::Oid>> {
        self.parent_ids(head_id)
    }

    fn commit_count(&self, base_id: git2::Oid, head_id: git2::Oid) -> Option<usize> {
        self.commit_count(base_id, head_id)
    }

    fn commit_range(
        &self,
        base_bound: std::ops::Bound<&git2::Oid>,
        head_bound: std::ops::Bound<&git2::Oid>,
    ) -> Result<Vec<git2::Oid>> {
        self.commit_range(base_bound, head_bound)
    }

    fn contains_commit(&self, haystack_id: git2::Oid, needle_id: git2::Oid) -> Result<bool> {
        self.contains_commit(haystack_id, needle_id)
    }

    fn cherry_pick(&mut self, head_id: git2::Oid, cherry_id: git2::Oid) -> Result<git2::Oid> {
        self.cherry_pick(head_id, cherry_id)
    }

    fn reword(&mut self, head_oid: git2::Oid, msg: &str) -> Result<git2::Oid> {
        self.reword(head_oid, msg)
    }

    fn squash(&mut self, head_id: git2::Oid, into_id: git2::Oid) -> Result<git2::Oid> {
        self.squash(head_id, into_id)
    }

    fn head_branch(&self) -> Option<Branch> {
        self.head_branch()
    }

    fn stash_push(&mut self, message: Option<&str>) -> Result<git2::Oid> {
        self.stash_push(message)
    }

    fn stash_pop(&mut self, stash_id: git2::Oid) -> Result<()> {
        self.stash_pop(stash_id)
    }

    fn branch(&mut self, name: &str, id: git2::Oid) -> Result<()> {
        self.branch(name, id)
    }

    fn delete_branch(&mut self, name: &str) -> Result<()> {
        self.delete_branch(name)
    }

    fn find_local_branch(&self, name: &str) -> Option<Branch> {
        self.find_local_branch(name)
    }

    fn find_remote_branch(&self, remote: &str, name: &str) -> Option<Branch> {
        self.find_remote_branch(remote, name)
    }

    fn local_branches(&self) -> Box<dyn Iterator<Item = Branch> + '_> {
        Box::new(self.local_branches())
    }

    fn remote_branches(&self) -> Box<dyn Iterator<Item = Branch> + '_> {
        Box::new(self.remote_branches())
    }

    fn detach(&mut self) -> Result<()> {
        self.detach()
    }

    fn switch_branch(&mut self, name: &str) -> Result<()> {
        self.switch_branch(name)
    }

    fn switch_commit(&mut self, id: git2::Oid) -> Result<()> {
        self.switch_commit(id)
    }
}

pub fn stash_push(repo: &mut dyn Repo, context: &str) -> Option<git2::Oid> {
    let branch = repo.head_branch();
    if !repo.is_dirty() {
        log::debug!("Nothing to stash");
        return None;
    }

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

pub fn commit_range(
    repo: &dyn Repo,
    head_to_base: impl std::ops::RangeBounds<git2::Oid>,
) -> Result<Vec<git2::Oid>> {
    let head_bound = head_to_base.start_bound();
    let base_bound = head_to_base.end_bound();
    repo.commit_range(base_bound, head_bound)
}
