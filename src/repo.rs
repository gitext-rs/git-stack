use bstr::ByteSlice;
use itertools::Itertools;

pub trait Repo {
    fn is_dirty(&self) -> bool;
    fn merge_base(&self, one: git2::Oid, two: git2::Oid) -> Option<git2::Oid>;

    fn find_commit(&self, id: git2::Oid) -> Option<std::rc::Rc<Commit>>;
    fn head_commit(&self) -> std::rc::Rc<Commit>;
    fn resolve(&self, revspec: &str) -> Option<std::rc::Rc<Commit>>;
    fn commits_from(
        &self,
        head_id: git2::Oid,
    ) -> Box<dyn Iterator<Item = std::rc::Rc<Commit>> + '_>;

    fn find_local_branch(&self, name: &str) -> Option<Branch>;
    fn local_branches(&self) -> Box<dyn Iterator<Item = Branch> + '_>;
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Branch {
    pub name: String,
    pub id: git2::Oid,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Commit {
    pub id: git2::Oid,
    pub summary: bstr::BString,
    pub is_merge: bool,
}

impl Commit {
    pub fn fixup_summary(&self) -> Option<&bstr::BStr> {
        self.summary
            .strip_prefix(b"fixup! ")
            .map(ByteSlice::as_bstr)
    }

    pub fn wip_summary(&self) -> Option<&bstr::BStr> {
        static WIP_PREFIXES: &[&[u8]] = &[b"WIP:", b"draft:", b"Draft:"];

        WIP_PREFIXES
            .iter()
            .filter_map(|prefix| self.summary.strip_prefix(*prefix).map(ByteSlice::as_bstr))
            .next()
    }
}

pub struct GitRepo {
    repo: git2::Repository,
    commits: std::cell::RefCell<std::collections::HashMap<git2::Oid, std::rc::Rc<Commit>>>,
}

impl GitRepo {
    pub fn new(repo: git2::Repository) -> Self {
        Self {
            repo,
            commits: Default::default(),
        }
    }

    pub fn is_dirty(&self) -> bool {
        if self.repo.state() != git2::RepositoryState::Clean {
            log::trace!("Repository status is unclean: {:?}", self.repo.state());
            return false;
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
            let commit = std::rc::Rc::new(Commit {
                id: commit.id(),
                summary,
                is_merge: 1 < commit.parent_count(),
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

        revwalk
            .filter_map(Result::ok)
            .filter_map(move |oid| self.find_commit(oid))
    }

    pub fn find_local_branch(&self, name: &str) -> Option<Branch> {
        let branch = self.repo.find_branch(name, git2::BranchType::Local).ok()?;
        let id = branch.get().target().unwrap();
        Some(Branch {
            name: name.to_owned(),
            id,
        })
    }

    pub fn local_branches(&self) -> impl Iterator<Item = Branch> + '_ {
        log::trace!("Loading branches");
        self.repo
            .branches(Some(git2::BranchType::Local))
            .into_iter()
            .flatten()
            .flat_map(|branch| {
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
                Some(Branch {
                    name: name.to_owned(),
                    id,
                })
            })
    }
}

impl Repo for GitRepo {
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

    fn find_local_branch(&self, name: &str) -> Option<Branch> {
        self.find_local_branch(name)
    }

    fn local_branches(&self) -> Box<dyn Iterator<Item = Branch> + '_> {
        Box::new(self.local_branches())
    }
}

pub struct InMemoryRepo {
    commits: std::collections::HashMap<git2::Oid, (Option<git2::Oid>, std::rc::Rc<Commit>)>,
    branches: std::collections::HashMap<String, Branch>,
    head_id: Option<git2::Oid>,

    last_id: usize,
}

impl InMemoryRepo {
    pub fn new() -> Self {
        Self {
            commits: Default::default(),
            branches: Default::default(),
            head_id: Default::default(),
            last_id: Default::default(),
        }
    }

    pub fn clear(&mut self) {
        *self = InMemoryRepo::new()
    }

    pub fn gen_id(&mut self) -> git2::Oid {
        let sha = format!("{:x}", self.last_id);
        self.last_id += 1;
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

    pub fn find_local_branch(&self, name: &str) -> Option<Branch> {
        self.branches.get(name).cloned()
    }

    pub fn local_branches(&self) -> impl Iterator<Item = Branch> + '_ {
        self.branches.values().cloned()
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

    fn find_local_branch(&self, name: &str) -> Option<Branch> {
        self.find_local_branch(name)
    }

    fn local_branches(&self) -> Box<dyn Iterator<Item = Branch> + '_> {
        Box::new(self.local_branches())
    }
}
