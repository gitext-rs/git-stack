#[derive(Clone, Default, Debug)]
pub struct Script {
    batches: Vec<Batch>,
}

impl Script {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn is_branch_deleted(&self, name: &str) -> bool {
        self.batches
            .iter()
            .flat_map(|b| b.commands.values())
            .flatten()
            .any(|c| {
                if let Command::DeleteBranch(current) = c {
                    current == name
                } else {
                    false
                }
            })
    }

    pub fn iter(&self) -> impl Iterator<Item = &'_ Batch> {
        self.batches.iter()
    }

    pub fn display<'a>(&'a self, labels: &'a dyn Labels) -> impl std::fmt::Display + 'a {
        ScriptDisplay {
            script: self,
            labels,
        }
    }

    fn infer_marks(&mut self) {
        let expected_marks = self
            .batches
            .iter()
            .map(|b| b.onto_mark())
            .collect::<Vec<_>>();
        for expected_mark in expected_marks {
            for batch in &mut self.batches {
                batch.infer_mark(expected_mark);
            }
        }
    }
}

impl From<Vec<Batch>> for Script {
    fn from(batches: Vec<Batch>) -> Self {
        // TODO: we should partition so its not all-or-nothing
        let graph = gen_graph(&batches);
        let batches = sort_batches(batches, &graph);
        let mut script = Self { batches };
        script.infer_marks();
        script
    }
}

impl<'s> IntoIterator for &'s Script {
    type Item = &'s Batch;
    type IntoIter = std::slice::Iter<'s, Batch>;

    fn into_iter(self) -> Self::IntoIter {
        self.batches.iter()
    }
}

impl std::fmt::Display for Script {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if !self.batches.is_empty() {
            let onto_id = self.batches[0].onto_mark();
            let labels = NamedLabels::new();
            labels.register_onto(onto_id);
            self.display(&labels).fmt(f)?;
        }

        Ok(())
    }
}

impl PartialEq for Script {
    fn eq(&self, other: &Self) -> bool {
        self.batches == other.batches
    }
}

impl Eq for Script {}

struct ScriptDisplay<'a> {
    script: &'a Script,
    labels: &'a dyn Labels,
}

impl<'a> std::fmt::Display for ScriptDisplay<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if !self.script.batches.is_empty() {
            writeln!(f, "label onto")?;
            for batch in &self.script.batches {
                writeln!(f)?;
                write!(f, "{}", batch.display(self.labels))?;
            }
        }

        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Batch {
    onto_mark: git2::Oid,
    commands: indexmap::IndexMap<git2::Oid, indexmap::IndexSet<Command>>,
    marks: indexmap::IndexSet<git2::Oid>,
}

impl Batch {
    pub fn new(onto_mark: git2::Oid) -> Self {
        Self {
            onto_mark,
            commands: Default::default(),
            marks: Default::default(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }

    pub fn onto_mark(&self) -> git2::Oid {
        self.onto_mark
    }

    pub fn branch(&self) -> Option<&str> {
        for (_, commands) in self.commands.iter().rev() {
            for command in commands.iter().rev() {
                if let Command::CreateBranch(name) = command {
                    return Some(name);
                }
            }
        }

        None
    }

    pub fn push(&mut self, id: git2::Oid, command: Command) {
        if let Command::RegisterMark(mark) = command {
            self.marks.insert(mark);
        }
        self.commands.entry(id).or_default().insert(command);
        if let Some((last_key, _)) = self.commands.last() {
            assert_eq!(*last_key, id, "gaps aren't allowed between ids");
        }
    }

    pub fn display<'a>(&'a self, labels: &'a dyn Labels) -> impl std::fmt::Display + 'a {
        BatchDisplay {
            batch: self,
            labels,
        }
    }

    fn id(&self) -> git2::Oid {
        *self
            .commands
            .first()
            .expect("called after filtering out empty")
            .0
    }

    fn infer_mark(&mut self, mark: git2::Oid) {
        if mark == self.onto_mark {
        } else if let Some(commands) = self.commands.get_mut(&mark) {
            self.marks.insert(mark);
            commands.insert(Command::RegisterMark(mark));
        }
    }
}

fn gen_graph(batches: &[Batch]) -> petgraph::graphmap::DiGraphMap<(git2::Oid, bool), usize> {
    let mut graph = petgraph::graphmap::DiGraphMap::new();
    for batch in batches {
        graph.add_edge((batch.onto_mark(), false), (batch.id(), true), 0);
        for mark in &batch.marks {
            graph.add_edge((batch.id(), true), (*mark, false), 0);
        }
    }
    graph
}

fn sort_batches(
    mut batches: Vec<Batch>,
    graph: &petgraph::graphmap::DiGraphMap<(git2::Oid, bool), usize>,
) -> Vec<Batch> {
    let mut unsorted = batches
        .drain(..)
        .map(|b| (b.id(), b))
        .collect::<std::collections::HashMap<_, _>>();
    for id in petgraph::algo::toposort(&graph, None)
        .unwrap()
        .into_iter()
        .filter_map(|(id, is_batch)| is_batch.then_some(id))
    {
        batches.push(unsorted.remove(&id).unwrap());
    }
    batches
}

struct BatchDisplay<'a> {
    batch: &'a Batch,
    labels: &'a dyn Labels,
}

impl<'a> std::fmt::Display for BatchDisplay<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let label = self.labels.get(self.batch.onto_mark());
        writeln!(f, "# Formerly {}", self.batch.onto_mark())?;
        writeln!(f, "reset {}", label)?;
        for (_, commands) in &self.batch.commands {
            for command in commands {
                match command {
                    Command::RegisterMark(mark_oid) => {
                        let label = self.labels.get(*mark_oid);
                        writeln!(f, "label {}", label)?;
                    }
                    Command::CherryPick(cherry_oid) => {
                        writeln!(f, "pick {}", cherry_oid)?;
                    }
                    Command::Reword(_msg) => {
                        writeln!(f, "reword")?;
                    }
                    Command::Fixup(squash_oid) => {
                        writeln!(f, "fixup {}", squash_oid)?;
                    }
                    Command::CreateBranch(name) => {
                        writeln!(f, "exec git switch --force-create {}", name)?;
                    }
                    Command::DeleteBranch(name) => {
                        writeln!(f, "exec git branch -D {}", name)?;
                    }
                }
            }
        }
        Ok(())
    }
}

pub trait Labels {
    fn get(&self, mark_id: git2::Oid) -> &str;
}

#[derive(Default)]
pub struct NamedLabels {
    generator: std::cell::RefCell<names::Generator<'static>>,
    names: elsa::FrozenMap<git2::Oid, String>,
}

impl NamedLabels {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn register_onto(&self, onto_id: git2::Oid) {
        self.names.insert(onto_id, "onto".to_owned());
    }

    pub fn get(&self, mark_id: git2::Oid) -> &str {
        if let Some(label) = self.names.get(&mark_id) {
            return label;
        }

        let label = self.generator.borrow_mut().next().unwrap();
        self.names.insert(mark_id, label)
    }
}

impl Labels for NamedLabels {
    fn get(&self, mark_id: git2::Oid) -> &str {
        self.get(mark_id)
    }
}

#[derive(Default)]
#[non_exhaustive]
pub struct OidLabels {
    onto_id: std::cell::Cell<Option<git2::Oid>>,
    names: elsa::FrozenMap<git2::Oid, String>,
}

impl OidLabels {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn register_onto(&self, onto_id: git2::Oid) {
        self.onto_id.set(Some(onto_id));
    }

    pub fn get(&self, mark_id: git2::Oid) -> &str {
        if let Some(label) = self.names.get(&mark_id) {
            return label;
        }

        let label = match self.onto_id.get() {
            Some(onto_id) if onto_id == mark_id => "onto".to_owned(),
            _ => mark_id.to_string(),
        };

        self.names.insert(mark_id, label)
    }
}

impl Labels for OidLabels {
    fn get(&self, mark_id: git2::Oid) -> &str {
        self.get(mark_id)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Command {
    /// Mark the current commit with an `Oid` for future reference
    RegisterMark(git2::Oid),
    /// Cherry-pick an existing commit
    CherryPick(git2::Oid),
    /// Change the wording of a commit message
    Reword(String),
    /// Squash a commit into prior commit, keeping the parent commits identity
    Fixup(git2::Oid),
    /// Mark a branch for creation at the current commit
    CreateBranch(String),
    /// Mark a branch for deletion
    DeleteBranch(String),
}

pub struct Executor {
    marks: std::collections::HashMap<git2::Oid, git2::Oid>,
    branches: Vec<(git2::Oid, String)>,
    delete_branches: Vec<String>,
    post_rewrite: Vec<(git2::Oid, git2::Oid)>,
    head_id: git2::Oid,
    dry_run: bool,
    detached: bool,
}

impl Executor {
    pub fn new(dry_run: bool) -> Executor {
        Self {
            marks: Default::default(),
            branches: Default::default(),
            delete_branches: Default::default(),
            post_rewrite: Default::default(),
            head_id: git2::Oid::zero(),
            dry_run,
            detached: false,
        }
    }

    pub fn run<'s>(
        &mut self,
        repo: &mut dyn crate::git::Repo,
        script: &'s Script,
    ) -> Vec<(git2::Error, &'s str, Vec<&'s str>)> {
        let mut failures = Vec::new();

        self.head_id = repo.head_commit().id;

        let onto_id = script.batches[0].onto_mark();
        let labels = NamedLabels::new();
        labels.register_onto(onto_id);
        for (i, batch) in script.batches.iter().enumerate() {
            let branch_name = batch.branch().unwrap_or("detached");
            if !failures.is_empty() {
                log::trace!("Ignoring `{}`", branch_name);
                log::trace!("Script:\n{}", batch.display(&labels));
                continue;
            }

            log::trace!("Applying `{}`", branch_name);
            log::trace!("Script:\n{}", batch.display(&labels));
            let res = self.stage_batch(repo, batch);
            match res.and_then(|_| self.commit(repo)) {
                Ok(()) => {
                    log::trace!("         `{}` succeeded", branch_name);
                }
                Err(err) => {
                    log::trace!("         `{}` failed: {}", branch_name, err);
                    self.abandon();
                    let dependent_branches = script.batches[(i + 1)..]
                        .iter()
                        .flat_map(|b| b.branch())
                        .collect::<Vec<_>>();
                    failures.push((err, branch_name, dependent_branches));
                }
            }
        }

        failures
    }

    fn stage_batch(
        &mut self,
        repo: &mut dyn crate::git::Repo,
        batch: &Batch,
    ) -> Result<(), git2::Error> {
        let onto_mark = batch.onto_mark();
        let onto_id = self.marks.get(&onto_mark).copied().unwrap_or(onto_mark);
        let commit = repo.find_commit(onto_id).ok_or_else(|| {
            git2::Error::new(
                git2::ErrorCode::NotFound,
                git2::ErrorClass::Reference,
                format!("could not find commit {:?}", onto_id),
            )
        })?;
        log::trace!("git checkout {}  # {}", onto_id, commit.summary);
        let mut head_oid = onto_id;
        for (_, commands) in &batch.commands {
            for command in commands {
                match command {
                    Command::RegisterMark(mark_oid) => {
                        let target_oid = head_oid;
                        self.marks.insert(*mark_oid, target_oid);
                    }
                    Command::CherryPick(cherry_oid) => {
                        let cherry_commit = repo.find_commit(*cherry_oid).ok_or_else(|| {
                            git2::Error::new(
                                git2::ErrorCode::NotFound,
                                git2::ErrorClass::Reference,
                                format!("could not find commit {:?}", cherry_oid),
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
                            repo.cherry_pick(head_oid, *cherry_oid)?
                        };
                        self.update_head(*cherry_oid, updated_oid);
                        self.post_rewrite.push((*cherry_oid, updated_oid));
                        head_oid = updated_oid;
                    }
                    Command::Reword(msg) => {
                        log::trace!("git commit --amend");
                        let updated_oid = if self.dry_run {
                            head_oid
                        } else {
                            repo.reword(head_oid, msg)?
                        };
                        self.update_head(head_oid, updated_oid);
                        for (_old_oid, new_oid) in &mut self.post_rewrite {
                            if *new_oid == head_oid {
                                *new_oid = updated_oid;
                            }
                        }
                        head_oid = updated_oid;
                    }
                    Command::Fixup(squash_oid) => {
                        let cherry_commit = repo.find_commit(*squash_oid).ok_or_else(|| {
                            git2::Error::new(
                                git2::ErrorCode::NotFound,
                                git2::ErrorClass::Reference,
                                format!("could not find commit {:?}", squash_oid),
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
                            repo.squash(*squash_oid, head_oid)?
                        };
                        self.update_head(head_oid, updated_oid);
                        self.update_head(*squash_oid, updated_oid);
                        for (_old_oid, new_oid) in &mut self.post_rewrite {
                            if *new_oid == head_oid {
                                *new_oid = updated_oid;
                            }
                        }
                        self.post_rewrite.push((*squash_oid, updated_oid));
                        head_oid = updated_oid;
                    }
                    Command::CreateBranch(name) => {
                        let branch_oid = head_oid;
                        self.branches.push((branch_oid, name.to_owned()));
                    }
                    Command::DeleteBranch(name) => {
                        self.delete_branches.push(name.to_owned());
                    }
                }
            }
        }

        Ok(())
    }

    pub fn update_head(&mut self, old_id: git2::Oid, new_id: git2::Oid) {
        if self.head_id == old_id && old_id != new_id {
            log::trace!("head changed from {} to {}", old_id, new_id);
            self.head_id = new_id;
        }
    }

    pub fn commit(&mut self, repo: &mut dyn crate::git::Repo) -> Result<(), git2::Error> {
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
                log::trace!("git switch --force-create {}", name);
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
            tx.committed()
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

    pub fn abandon(&mut self) {
        self.branches.clear();
        self.delete_branches.clear();
        self.post_rewrite.clear();
    }

    pub fn close(
        &mut self,
        repo: &mut dyn crate::git::Repo,
        restore_branch: Option<&str>,
    ) -> Result<(), git2::Error> {
        assert_eq!(&self.branches, &[]);
        assert_eq!(self.delete_branches, Vec::<String>::new());
        if let Some(restore_branch) = restore_branch {
            log::trace!("git switch {}", restore_branch);
            if !self.dry_run && self.detached {
                repo.switch_branch(restore_branch)?;
            }
        } else if self.head_id != git2::Oid::zero() {
            log::trace!("git switch {}", self.head_id);
            if !self.dry_run && self.detached {
                repo.switch_commit(self.head_id)?;
            }
        }

        Ok(())
    }
}
