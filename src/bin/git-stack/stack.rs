use std::io::Write;

use bstr::ByteSlice;
use eyre::WrapErr;
use itertools::Itertools;
use proc_exit::WithCodeResultExt;

struct State {
    repo: git_stack::git::GitRepo,
    branches: git_stack::git::Branches,
    protected_branches: git_stack::git::Branches,
    head_commit: std::rc::Rc<git_stack::git::Commit>,
    stacks: Vec<StackState>,

    rebase: bool,
    pull: bool,
    push: bool,
    dry_run: bool,

    show_format: git_stack::config::Format,
    show_stacked: bool,
}

impl State {
    fn new(
        mut repo: git_stack::git::GitRepo,
        args: &crate::args::Args,
    ) -> Result<Self, proc_exit::Exit> {
        let repo_config = git_stack::config::RepoConfig::from_all(repo.raw())
            .with_code(proc_exit::Code::CONFIG_ERR)?
            .update(args.to_config());

        let mut rebase = args.rebase;
        let pull = args.pull;
        if pull {
            log::trace!("`--pull` implies `--rebase`");
            rebase = true;
        }
        let push = args.push;
        let protected = git_stack::git::ProtectedBranches::new(
            repo_config.protected_branches().iter().map(|s| s.as_str()),
        )
        .with_code(proc_exit::Code::CONFIG_ERR)?;
        let dry_run = args.dry_run;

        let show_format = repo_config.show_format();
        let show_stacked = repo_config.show_stacked();

        repo.set_push_remote(repo_config.push_remote());
        repo.set_pull_remote(repo_config.pull_remote());

        let branches = git_stack::git::Branches::new(repo.local_branches());
        let protected_branches = branches.protected(&protected);
        let head_commit = repo.head_commit();
        let base = args
            .base
            .as_deref()
            .map(|name| resolve_explicit_base(&repo, name))
            .transpose()
            .with_code(proc_exit::Code::USAGE_ERR)?;
        let onto = args
            .onto
            .as_deref()
            .map(|name| resolve_explicit_base(&repo, name))
            .transpose()
            .with_code(proc_exit::Code::USAGE_ERR)?;
        let stacks = match (base, onto, repo_config.stack()) {
            (Some(base), None, git_stack::config::Stack::All) => {
                let onto = base.clone();
                vec![StackState {
                    base,
                    onto,
                    branches: branches.all(),
                }]
            }
            (None, Some(onto), git_stack::config::Stack::All) => {
                let base = onto.clone();
                vec![StackState {
                    base,
                    onto,
                    branches: branches.all(),
                }]
            }
            (None, None, git_stack::config::Stack::All) => {
                let mut stack_branches = std::collections::HashMap::new();
                for (branch_id, branch) in branches.iter() {
                    let base_branch =
                        resolve_implicit_base(&repo, branch_id, &branches, &protected_branches)
                            .with_code(proc_exit::Code::USAGE_ERR)?;
                    stack_branches
                        .entry(base_branch)
                        .or_insert_with(git_stack::git::Branches::default)
                        .extend(branch.iter().cloned());
                }
                stack_branches
                    .into_iter()
                    .map(|(base, branches)| {
                        let onto = base.clone();
                        StackState {
                            base,
                            onto,
                            branches,
                        }
                    })
                    .collect()
            }
            (base, onto, stack) => {
                let base = base
                    .map(Result::Ok)
                    .unwrap_or_else(|| {
                        resolve_implicit_base(&repo, head_commit.id, &branches, &protected_branches)
                    })
                    .with_code(proc_exit::Code::USAGE_ERR)?;
                let onto = onto.unwrap_or_else(|| base.clone());
                let merge_base_oid = repo
                    .merge_base(base.id, head_commit.id)
                    .ok_or_else(|| {
                        git2::Error::new(
                            git2::ErrorCode::NotFound,
                            git2::ErrorClass::Reference,
                            format!("could not find base between {} and HEAD", base.name),
                        )
                    })
                    .with_code(proc_exit::Code::USAGE_ERR)?;
                let stack_branches = match stack {
                    git_stack::config::Stack::Current => {
                        branches.branch(&repo, merge_base_oid, head_commit.id)
                    }
                    git_stack::config::Stack::Dependents => {
                        branches.dependents(&repo, merge_base_oid, head_commit.id)
                    }
                    git_stack::config::Stack::Descendants => {
                        branches.descendants(&repo, merge_base_oid)
                    }
                    git_stack::config::Stack::All => unreachable!("Covered in another branch"),
                };
                vec![StackState {
                    base,
                    onto,
                    branches: stack_branches,
                }]
            }
        };

        Ok(Self {
            repo,
            branches,
            protected_branches,
            head_commit,
            stacks,

            rebase,
            pull,
            push,
            dry_run,
            show_format,
            show_stacked,
        })
    }

    fn update(&mut self) -> eyre::Result<()> {
        self.head_commit = self.repo.head_commit();
        self.branches.update(&self.repo);
        self.protected_branches.update(&self.repo);

        for stack in self.stacks.iter_mut() {
            stack.update(&self.repo)?;
        }

        Ok(())
    }
}

struct StackState {
    base: git_stack::git::Branch,
    onto: git_stack::git::Branch,
    branches: git_stack::git::Branches,
}

impl StackState {
    fn update(&mut self, repo: &dyn git_stack::git::Repo) -> eyre::Result<()> {
        self.base = repo
            .find_local_branch(self.base.name.as_str())
            .ok_or_else(|| eyre::eyre!("can no longer find branch {}", self.base.name))?;
        self.onto = repo
            .find_local_branch(self.onto.name.as_str())
            .ok_or_else(|| eyre::eyre!("can no longer find branch {}", self.onto.name))?;
        self.branches.update(repo);
        Ok(())
    }
}

pub fn stack(args: &crate::args::Args, colored_stdout: bool) -> proc_exit::ExitResult {
    log::trace!("Initializing");
    let cwd = std::env::current_dir().with_code(proc_exit::Code::USAGE_ERR)?;
    let repo = git2::Repository::discover(&cwd).with_code(proc_exit::Code::USAGE_ERR)?;
    let repo = git_stack::git::GitRepo::new(repo);
    let mut state = State::new(repo, args)?;

    if state.pull {
        let mut pulled = false;
        for stack in state.stacks.iter() {
            if state.protected_branches.contains_oid(stack.onto.id) {
                match git_pull(&mut state.repo, stack.onto.name.as_str(), state.dry_run) {
                    Ok(_) => {
                        pulled = true;
                    }
                    Err(err) => {
                        log::warn!("Skipping pull of `{}`, {}", stack.onto.name, err);
                    }
                }
            } else {
                log::warn!(
                    "Skipping pull of `{}`, not a protected branch",
                    stack.onto.name
                );
            }
        }
        if pulled {
            state.update().with_code(proc_exit::Code::FAILURE)?;
        }
    }

    let mut success = true;
    if state.rebase {
        let head_oid = state.repo.head_commit().id;
        let head_branch = if let Some(branches) = state.branches.get(head_oid) {
            branches[0].clone()
        } else {
            return Err(eyre::eyre!("Must not be in a detached HEAD state."))
                .with_code(proc_exit::Code::USAGE_ERR);
        };

        let scripts: Result<Vec<_>, proc_exit::Exit> = state
            .stacks
            .iter()
            .map(|stack| plan_rebase(&state, stack).with_code(proc_exit::Code::FAILURE))
            .collect();
        let scripts = scripts?;

        let mut executor = git_stack::git::Executor::new(&state.repo, state.dry_run);
        for script in scripts {
            let results = executor.run_script(&mut state.repo, &script);
            for (err, name, dependents) in results.iter() {
                success = false;
                log::error!("Failed to re-stack branch `{}`: {}", name, err);
                if !dependents.is_empty() {
                    log::error!("  Blocked dependents: {}", dependents.iter().join(", "));
                }
            }
        }
        executor
            .close(&mut state.repo, &head_branch.name)
            .with_code(proc_exit::Code::FAILURE)?;
        state.update().with_code(proc_exit::Code::FAILURE)?;
    }

    if state.push {
        push(&mut state).with_code(proc_exit::Code::FAILURE)?;
        state.update().with_code(proc_exit::Code::FAILURE)?;
    }

    show(&state, colored_stdout).with_code(proc_exit::Code::FAILURE)?;

    if !success {
        return proc_exit::Code::FAILURE.ok();
    }

    Ok(())
}

fn plan_rebase(state: &State, stack: &StackState) -> eyre::Result<git_stack::git::Script> {
    let mut graphed_branches = stack.branches.clone();
    if !graphed_branches.contains_oid(stack.base.id) {
        graphed_branches.insert(stack.base.clone());
    }
    if !graphed_branches.contains_oid(stack.onto.id) {
        graphed_branches.insert(stack.onto.clone());
    }
    let mut root = git_stack::graph::Node::new(state.head_commit.clone(), &mut graphed_branches);
    root = root.extend(&state.repo, graphed_branches)?;

    git_stack::graph::protect_branches(&mut root, &state.repo, &state.protected_branches)?;

    git_stack::graph::rebase_branches(&mut root, stack.onto.id)?;

    // TODO Identify commits to drop by tree id
    // TODO Identify commits to drop by guessing
    // TODO Snap branches to be on branches, but how do we know which is the base for the other?
    // Only do for HEAD?
    // TODO Re-arrange fixup commits
    // TODO Re-stack branches that have been individually rebased
    git_stack::graph::delinearize(&mut root);

    let script = git_stack::graph::to_script(&root);

    Ok(script)
}

fn push(state: &mut State) -> eyre::Result<()> {
    let mut graphed_branches = git_stack::git::Branches::new(None.into_iter());
    for stack in state.stacks.iter() {
        graphed_branches.extend(stack.branches.iter().flat_map(|(_, b)| b.to_owned()));
    }
    for stack in state.stacks.iter() {
        if !graphed_branches.contains_oid(stack.base.id) {
            graphed_branches.insert(stack.base.clone());
        }
        if !graphed_branches.contains_oid(stack.onto.id) {
            graphed_branches.insert(stack.onto.clone());
        }
    }
    let mut root = git_stack::graph::Node::new(state.head_commit.clone(), &mut graphed_branches);
    root = root.extend(&state.repo, graphed_branches)?;

    git_stack::graph::protect_branches(&mut root, &state.repo, &state.protected_branches)?;
    git_stack::graph::pushable(&mut root)?;

    git_push(&mut state.repo, &root, state.dry_run)?;

    Ok(())
}

fn show(state: &State, colored_stdout: bool) -> eyre::Result<()> {
    let mut graphed_branches = git_stack::git::Branches::new(None.into_iter());
    for stack in state.stacks.iter() {
        graphed_branches.extend(stack.branches.iter().flat_map(|(_, b)| b.to_owned()));
    }
    for stack in state.stacks.iter() {
        if !graphed_branches.contains_oid(stack.base.id) {
            graphed_branches.insert(stack.base.clone());
        }
        if !graphed_branches.contains_oid(stack.onto.id) {
            graphed_branches.insert(stack.onto.clone());
        }
    }
    let mut root = git_stack::graph::Node::new(state.head_commit.clone(), &mut graphed_branches);
    root = root.extend(&state.repo, graphed_branches)?;

    git_stack::graph::protect_branches(&mut root, &state.repo, &state.protected_branches)?;
    git_stack::graph::pushable(&mut root)?;
    if !state.show_stacked {
        git_stack::graph::delinearize(&mut root);
    }

    match state.show_format {
        git_stack::config::Format::Silent => (),
        git_stack::config::Format::Brief => {
            writeln!(
                std::io::stdout(),
                "{}",
                DisplayTree::new(&state.repo, &root)
                    .colored(colored_stdout)
                    .all(false)
            )?;
        }
        git_stack::config::Format::Full => {
            writeln!(
                std::io::stdout(),
                "{}",
                DisplayTree::new(&state.repo, &root)
                    .colored(colored_stdout)
                    .all(true)
            )?;
        }
    }

    Ok(())
}

fn resolve_explicit_base(
    repo: &dyn git_stack::git::Repo,
    base: &str,
) -> eyre::Result<git_stack::git::Branch> {
    repo.find_local_branch(base)
        .ok_or_else(|| eyre::eyre!("could not find branch {:?}", base))
}

fn resolve_implicit_base(
    repo: &dyn git_stack::git::Repo,
    head_oid: git2::Oid,
    branches: &git_stack::git::Branches,
    protected_branches: &git_stack::git::Branches,
) -> eyre::Result<git_stack::git::Branch> {
    let branch = git_stack::git::find_protected_base(repo, protected_branches, head_oid)
        .ok_or_else(|| eyre::eyre!("could not find a protected branch to use as a base"))?;
    log::debug!(
        "Chose branch {} as the base for {}",
        branch.name,
        branches
            .get(head_oid)
            .map(|b| b[0].name.clone())
            .or_else(|| {
                repo.find_commit(head_oid)?
                    .summary
                    .to_str()
                    .ok()
                    .map(ToOwned::to_owned)
            })
            .unwrap_or_else(|| "target".to_owned())
    );
    Ok(branch.clone())
}

fn git_pull(
    repo: &mut git_stack::git::GitRepo,
    branch_name: &str,
    dry_run: bool,
) -> eyre::Result<git2::Oid> {
    let remote = repo.pull_remote();
    log::debug!("git pull --rebase {} {}", remote, branch_name);
    let remote_branch_name = format!("{}/{}", remote, branch_name);
    if dry_run {
        return Ok(repo.find_local_branch(branch_name).unwrap().id);
    }

    let mut last_id;
    {
        // A little uncertain about some of the weirder authentication needs, just deferring to `git`
        // instead of using `libgit2`
        let status = std::process::Command::new("git")
            .arg("fetch")
            .arg(remote)
            .arg(branch_name)
            .status()
            .wrap_err("Could not run `git fetch`")?;
        if !status.success() {
            eyre::bail!("`git fetch {} {}` failed", remote, branch_name,);
        }

        let local_branch = repo
            .raw()
            .find_branch(branch_name, git2::BranchType::Local)
            .wrap_err_with(|| eyre::eyre!("local branch `{}` doesn't exist", branch_name))?;
        let local_branch_annotated = {
            repo.raw()
                .reference_to_annotated_commit(local_branch.get())?
        };
        log::trace!(
            "rebase local {}={}",
            branch_name,
            local_branch_annotated.id()
        );

        let remote_branch = repo
            .raw()
            .find_branch(&remote_branch_name, git2::BranchType::Remote)
            .wrap_err_with(|| {
                eyre::eyre!("remote branch `{}` doesn't exist", remote_branch_name)
            })?;
        let remote_branch_annotated = repo
            .raw()
            .reference_to_annotated_commit(remote_branch.get())?;
        log::trace!(
            "rebase remote {}={}",
            remote_branch_name,
            remote_branch_annotated.id()
        );
        last_id = remote_branch_annotated.id();

        let base_id = repo
            .merge_base(local_branch_annotated.id(), remote_branch_annotated.id())
            .ok_or_else(|| {
                eyre::eyre!(
                    "No common base between {} and {}",
                    branch_name,
                    remote_branch_name
                )
            })?;
        let base_annotated = repo.raw().find_annotated_commit(base_id).unwrap();
        log::trace!("rebase base {}", base_annotated.id());

        if repo.merge_base(local_branch_annotated.id(), remote_branch_annotated.id())
            == Some(remote_branch_annotated.id())
        {
            log::debug!("{} is up-to-date with {}", branch_name, remote_branch_name);
            return Ok(local_branch_annotated.id());
        }

        let mut rebase = repo
            .raw()
            .rebase(
                Some(&local_branch_annotated),
                Some(&base_annotated),
                Some(&remote_branch_annotated),
                Some(git2::RebaseOptions::new().inmemory(true)),
            )
            .wrap_err_with(|| {
                eyre::eyre!(
                    "failed to rebase `{}` onto `{}`",
                    branch_name,
                    remote_branch_name
                )
            })?;

        while let Some(op) = rebase.next() {
            let op = op
                .map_err(|e| {
                    let _ = rebase.abort();
                    e
                })
                .wrap_err_with(|| {
                    eyre::eyre!(
                        "failed to rebase `{}` onto `{}`",
                        branch_name,
                        remote_branch_name
                    )
                })?;
            log::trace!("Rebase: {:?} {}", op.kind(), op.id());
            if rebase.inmemory_index().unwrap().has_conflicts() {
                eyre::bail!(
                    "conflicts between {} and {}",
                    branch_name,
                    remote_branch_name
                );
            }

            let sig = repo.raw().signature().unwrap();
            let commit_id = rebase
                .commit(None, &sig, None)
                .map_err(|e| {
                    let _ = rebase.abort();
                    e
                })
                .wrap_err_with(|| {
                    eyre::eyre!(
                        "failed to rebase `{}` onto `{}`",
                        branch_name,
                        remote_branch_name
                    )
                })?;
            last_id = commit_id;
        }

        rebase.finish(None).wrap_err_with(|| {
            eyre::eyre!(
                "failed to rebase `{}` onto `{}`",
                branch_name,
                remote_branch_name
            )
        })?;
    }

    let local_branch = repo.find_local_branch(branch_name).unwrap();
    if local_branch.id == repo.head_commit().id {
        log::trace!("Updating {} (HEAD)", branch_name);
        repo.detach().wrap_err_with(|| {
            eyre::eyre!(
                "failed to update `{}` to `{}`",
                branch_name,
                remote_branch_name
            )
        })?;
        repo.branch(branch_name, last_id).wrap_err_with(|| {
            eyre::eyre!(
                "failed to update `{}` to `{}`",
                branch_name,
                remote_branch_name
            )
        })?;
        repo.switch(branch_name).wrap_err_with(|| {
            eyre::eyre!(
                "failed to update `{}` to `{}`",
                branch_name,
                remote_branch_name
            )
        })?;
    } else {
        log::trace!("Updating {}", branch_name);
        repo.branch(branch_name, last_id).wrap_err_with(|| {
            eyre::eyre!(
                "failed to update `{}` to `{}`",
                branch_name,
                remote_branch_name
            )
        })?;
    }

    Ok(last_id)
}

fn git_push(
    repo: &mut git_stack::git::GitRepo,
    node: &git_stack::graph::Node,
    dry_run: bool,
) -> eyre::Result<()> {
    let failed = git_push_internal(repo, node, dry_run);
    if failed.is_empty() {
        Ok(())
    } else {
        eyre::bail!("could not push {}", failed.into_iter().join(", "));
    }
}

fn git_push_internal(
    repo: &mut git_stack::git::GitRepo,
    node: &git_stack::graph::Node,
    dry_run: bool,
) -> Vec<String> {
    let mut failed = Vec::new();
    for branch in node.branches.iter() {
        if node.pushable {
            let remote = repo.push_remote();
            log::trace!(
                "git push --force-with-lease --set-upstream {} {}",
                remote,
                branch.name
            );
            if !dry_run {
                let status = std::process::Command::new("git")
                    .arg("push")
                    .arg("--force-with-lease")
                    .arg("--set-upstream")
                    .arg(repo.push_remote())
                    .arg(&branch.name)
                    .status();
                match status {
                    Ok(status) => {
                        if !status.success() {
                            failed.push(branch.name.clone());
                        }
                    }
                    Err(err) => {
                        log::debug!("`git push` failed with {}", err);
                        failed.push(branch.name.clone());
                    }
                }
            }
        } else if node.action.is_protected() || node.action.is_rebase() {
            log::debug!("Skipping push of `{}`, protected", branch.name);
        } else {
            log::debug!("Skipping push of `{}`", branch.name);
        }
    }

    if failed.is_empty() {
        for child in node.children.iter() {
            for child_node in child.iter() {
                failed.extend(git_push_internal(repo, child_node, dry_run));
            }
        }
    }

    failed
}

struct DisplayTree<'r, 'n> {
    repo: &'r git_stack::git::GitRepo,
    root: &'n git_stack::graph::Node,
    palette: Palette,
    all: bool,
}

impl<'r, 'n> DisplayTree<'r, 'n> {
    pub fn new(repo: &'r git_stack::git::GitRepo, root: &'n git_stack::graph::Node) -> Self {
        Self {
            repo,
            root,
            palette: Palette::plain(),
            all: false,
        }
    }

    pub fn colored(mut self, yes: bool) -> Self {
        if yes {
            self.palette = Palette::colored()
        } else {
            self.palette = Palette::plain()
        }
        self
    }

    pub fn all(mut self, yes: bool) -> Self {
        self.all = yes;
        self
    }
}

impl<'r, 'n> std::fmt::Display for DisplayTree<'r, 'n> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        let mut tree = treeline::Tree::root(RenderNode {
            repo: self.repo,
            node: Some(self.root),
            palette: &self.palette,
        });
        to_tree(
            self.repo,
            self.root.children.as_slice(),
            &mut tree,
            &self.palette,
            self.all,
        );
        tree.fmt(f)
    }
}

fn to_tree<'r, 'n, 'p>(
    repo: &'r git_stack::git::GitRepo,
    nodes: &'n [Vec<git_stack::graph::Node>],
    tree: &mut treeline::Tree<RenderNode<'r, 'n, 'p>>,
    palette: &'p Palette,
    show_all: bool,
) {
    for branch in nodes {
        let mut branch_root = treeline::Tree::root(RenderNode {
            repo,
            node: None,
            palette,
        });
        for node in branch {
            if node.branches.is_empty() && node.children.is_empty() && !show_all {
                log::trace!("Skipping commit {}", node.local_commit.id);
                continue;
            }
            let mut child_tree = treeline::Tree::root(RenderNode {
                repo,
                node: Some(node),
                palette,
            });
            to_tree(
                repo,
                node.children.as_slice(),
                &mut child_tree,
                palette,
                show_all,
            );
            branch_root.push(child_tree);
        }
        tree.push(branch_root);
    }
}

struct RenderNode<'r, 'n, 'p> {
    repo: &'r git_stack::git::GitRepo,
    node: Option<&'n git_stack::graph::Node>,
    palette: &'p Palette,
}

impl<'r, 'n, 'p> std::fmt::Display for RenderNode<'r, 'n, 'p> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        if let Some(node) = self.node.as_ref() {
            if node.branches.is_empty() {
                write!(f, "{} ", self.palette.info.paint(node.local_commit.id))?;
            } else if node.action == git_stack::graph::Action::Protected {
                write!(
                    f,
                    "{} ",
                    self.palette
                        .info
                        .paint(node.branches.iter().map(|b| b.name.as_str()).join(", ")),
                )?;
                let branch = &node.branches[0];
                match commit_relation(self.repo, branch.id, branch.pull_id) {
                    Some((0, 0)) => {}
                    Some((local, 0)) => {
                        write!(
                            f,
                            "{} ",
                            self.palette.warn.paint(format!("({} ahead)", local)),
                        )?;
                    }
                    Some((0, remote)) => {
                        write!(
                            f,
                            "{} ",
                            self.palette.warn.paint(format!("({} behind)", remote)),
                        )?;
                    }
                    Some((local, remote)) => {
                        write!(
                            f,
                            "{} ",
                            self.palette
                                .warn
                                .paint(format!("({} ahead, {} behind)", local, remote)),
                        )?;
                    }
                    None => {
                        write!(f, "{} ", self.palette.warn.paint("(no remote)"))?;
                    }
                }
            } else {
                write!(
                    f,
                    "{} ",
                    self.palette
                        .branch
                        .paint(node.branches.iter().map(|b| b.name.as_str()).join(", ")),
                )?;
                if node.pushable {
                    write!(f, "{} ", self.palette.info.paint("(ready) "))?;
                } else {
                    let branch = &node.branches[0];
                    match commit_relation(self.repo, branch.id, branch.push_id) {
                        Some((0, 0)) => {
                            write!(f, "{} ", self.palette.info.paint("(pushed)"))?;
                        }
                        Some((local, 0)) => {
                            write!(
                                f,
                                "{} ",
                                self.palette.info.paint(format!("({} ahead)", local)),
                            )?;
                        }
                        Some((0, remote)) => {
                            write!(
                                f,
                                "{} ",
                                self.palette.warn.paint(format!("({} behind)", remote)),
                            )?;
                        }
                        Some((local, remote)) => {
                            write!(
                                f,
                                "{} ",
                                self.palette
                                    .warn
                                    .paint(format!("({} ahead, {} behind)", local, remote)),
                            )?;
                        }
                        None => {
                            write!(f, "{} ", self.palette.info.paint("(no remote)"))?;
                        }
                    }
                }
            }

            let summary = String::from_utf8_lossy(&node.local_commit.summary);
            if node.action == git_stack::graph::Action::Protected {
                write!(f, "{}", self.palette.hint.paint(summary))?;
            } else if 1 < self
                .repo
                .raw()
                .find_commit(node.local_commit.id)
                .unwrap()
                .parent_count()
            {
                write!(f, "{}", self.palette.error.paint("merge commit"))?;
            } else if node.branches.is_empty() && !node.children.is_empty() {
                // Branches should be off of other branches
                write!(f, "{}", self.palette.warn.paint(summary))?;
            } else if node.local_commit.fixup_summary().is_some() {
                // Needs to be squashed
                write!(f, "{}", self.palette.warn.paint(summary))?;
            } else if node.local_commit.wip_summary().is_some() {
                // Not for pushing implicitly
                write!(f, "{}", self.palette.error.paint(summary))?;
            } else {
                write!(f, "{}", self.palette.hint.paint(summary))?;
            }
        } else {
            write!(f, "o")?;
        }
        Ok(())
    }
}

fn commit_relation(
    repo: &git_stack::git::GitRepo,
    local: git2::Oid,
    remote: Option<git2::Oid>,
) -> Option<(usize, usize)> {
    let remote = remote?;
    let base = repo.merge_base(local, remote)?;
    let local_count = repo
        .commits_from(local)
        .take_while(|c| c.id != base)
        .count();
    let remote_count = repo
        .commits_from(remote)
        .take_while(|c| c.id != base)
        .count();
    Some((local_count, remote_count))
}

#[derive(Copy, Clone, Debug)]
struct Palette {
    error: yansi::Style,
    warn: yansi::Style,
    info: yansi::Style,
    branch: yansi::Style,
    hint: yansi::Style,
}

impl Palette {
    pub fn colored() -> Self {
        Self {
            error: yansi::Style::new(yansi::Color::Red),
            warn: yansi::Style::new(yansi::Color::Yellow),
            info: yansi::Style::new(yansi::Color::Blue),
            branch: yansi::Style::new(yansi::Color::Green),
            hint: yansi::Style::new(yansi::Color::Blue).dimmed(),
        }
    }

    pub fn plain() -> Self {
        Self {
            error: yansi::Style::default(),
            warn: yansi::Style::default(),
            info: yansi::Style::default(),
            branch: yansi::Style::default(),
            hint: yansi::Style::default(),
        }
    }
}
