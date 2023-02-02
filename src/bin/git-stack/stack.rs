use std::collections::VecDeque;
use std::io::Write;

use bstr::ByteSlice;
use eyre::WrapErr;
use itertools::Itertools;
use proc_exit::prelude::*;

struct State {
    repo: git_stack::legacy::git::GitRepo,
    branches: git_stack::legacy::git::Branches,
    protected_branches: git_stack::legacy::git::Branches,
    head_commit: std::rc::Rc<git_stack::legacy::git::Commit>,
    stacks: Vec<StackState>,

    rebase: bool,
    pull: bool,
    push: bool,
    fixup: git_stack::config::Fixup,
    repair: bool,
    dry_run: bool,
    snapshot_capacity: Option<usize>,
    protect_commit_count: Option<usize>,
    protect_commit_age: std::time::Duration,
    protect_commit_time: std::time::SystemTime,

    show_format: git_stack::config::Format,
    show_commits: git_stack::config::ShowCommits,
    show_stacked: bool,
}

impl State {
    fn new(
        mut repo: git_stack::legacy::git::GitRepo,
        args: &crate::args::Args,
    ) -> Result<Self, proc_exit::Exit> {
        let repo_config = git_stack::config::RepoConfig::from_all(repo.raw())
            .with_code(proc_exit::sysexits::CONFIG_ERR)?
            .update(args.to_config());

        let mut rebase = args.rebase;
        let pull = args.pull;
        if pull {
            log::trace!("`--pull` implies `--rebase`");
            rebase = true;
        }
        let rebase = rebase;

        let fixup = match (args.fixup, args.rebase) {
            (Some(fixup), _) => fixup,
            (_, true) => repo_config.auto_fixup(),
            _ => {
                // Assume the user is only wanting to show the tree and not modify it.
                let no_op = git_stack::config::Fixup::Ignore;
                if no_op != repo_config.auto_fixup() {
                    log::trace!(
                        "Ignoring `auto-fixup={}` without an explicit `--rebase`",
                        repo_config.auto_fixup()
                    );
                }
                no_op
            }
        };
        let repair = match (args.repair(), args.rebase) {
            (Some(repair), _) => repair,
            (_, true) => repo_config.auto_repair(),
            _ => {
                // Assume the user is only wanting to show the tree and not modify it.
                if repo_config.auto_repair() {
                    log::trace!(
                        "Ignoring `auto-repair={}` without an explicit `--rebase`",
                        repo_config.auto_repair()
                    );
                }
                false
            }
        };
        let push = args.push;
        let protected = git_stack::legacy::git::ProtectedBranches::new(
            repo_config.protected_branches().iter().map(|s| s.as_str()),
        )
        .with_code(proc_exit::sysexits::CONFIG_ERR)?;
        let dry_run = args.dry_run;
        let snapshot_capacity = repo_config.capacity();
        let protect_commit_count = repo_config.protect_commit_count();
        let protect_commit_age = repo_config.protect_commit_age();
        let protect_commit_time = std::time::SystemTime::now() - protect_commit_age;
        let show_format = repo_config.show_format();
        let show_commits = repo_config.show_commits();
        let show_stacked = repo_config.show_stacked();

        repo.set_push_remote(repo_config.push_remote());
        repo.set_pull_remote(repo_config.pull_remote());
        let config = repo
            .raw()
            .config()
            .with_code(proc_exit::sysexits::CONFIG_ERR)?;
        repo.set_sign(
            config
                .get_bool("stack.gpgSign")
                .or_else(|_| config.get_bool("commit.gpgSign"))
                .unwrap_or_default(),
        )
        .with_code(proc_exit::Code::FAILURE)?;

        let mut branches = git_stack::legacy::git::Branches::new([]);
        let mut protected_branches = git_stack::legacy::git::Branches::new([]);
        for branch in repo.local_branches() {
            if protected.is_protected(&branch.name) {
                log::trace!("Branch {} is protected", branch);
                if let Some(remote) = repo.find_remote_branch(repo.pull_remote(), &branch.name) {
                    protected_branches.insert(remote.clone());
                    branches.insert(remote);
                } else {
                    protected_branches.insert(branch.clone());
                }
            }
            branches.insert(branch);
        }
        let head_commit = repo.head_commit();
        let base = args
            .base
            .as_deref()
            .map(|name| resolve_explicit_base(&repo, name))
            .transpose()
            .with_code(proc_exit::sysexits::USAGE_ERR)?;
        let onto = args
            .onto
            .as_deref()
            .map(|name| resolve_explicit_base(&repo, name))
            .transpose()
            .with_code(proc_exit::sysexits::USAGE_ERR)?;

        let stacks = match (base, onto, repo_config.stack()) {
            (Some(base), Some(onto), git_stack::config::Stack::All) => {
                vec![StackState::new(base, onto, branches.all())]
            }
            (Some(base), None, git_stack::config::Stack::All) => {
                let onto = resolve_onto_from_base(&repo, &base);
                vec![StackState::new(base, onto, branches.all())]
            }
            (None, Some(onto), git_stack::config::Stack::All) => {
                let base = resolve_base_from_onto(&repo, &onto);
                vec![StackState::new(base, onto, branches.all())]
            }
            (None, None, git_stack::config::Stack::All) => {
                let mut stack_branches = std::collections::BTreeMap::new();
                for (branch_id, branch) in branches.iter() {
                    let base_branch = resolve_implicit_base(
                        &repo,
                        branch_id,
                        &branches,
                        &protected_branches,
                        repo_config.auto_base_commit_count(),
                    );
                    stack_branches
                        .entry(base_branch)
                        .or_insert_with(git_stack::legacy::git::Branches::default)
                        .extend(branch.iter().cloned());
                }
                stack_branches
                    .into_iter()
                    .map(|(onto, branches)| {
                        let base = resolve_base_from_onto(&repo, &onto);
                        StackState::new(base, onto, branches)
                    })
                    .collect()
            }
            (base, onto, stack) => {
                let (base, onto) = match (base, onto) {
                    (Some(base), Some(onto)) => (base, onto),
                    (Some(base), None) => {
                        let onto = resolve_onto_from_base(&repo, &base);
                        (base, onto)
                    }
                    (None, Some(onto)) => {
                        let base = resolve_implicit_base(
                            &repo,
                            head_commit.id,
                            &branches,
                            &protected_branches,
                            repo_config.auto_base_commit_count(),
                        );
                        // HACK: Since `base` might have come back with a remote branch, treat it as an
                        // "onto" to find the local version.
                        let base = resolve_base_from_onto(&repo, &base);
                        (base, onto)
                    }
                    (None, None) => {
                        let onto = resolve_implicit_base(
                            &repo,
                            head_commit.id,
                            &branches,
                            &protected_branches,
                            repo_config.auto_base_commit_count(),
                        );
                        let base = resolve_base_from_onto(&repo, &onto);
                        (base, onto)
                    }
                };
                let merge_base_oid = repo
                    .merge_base(base.id, head_commit.id)
                    .ok_or_else(|| {
                        git2::Error::new(
                            git2::ErrorCode::NotFound,
                            git2::ErrorClass::Reference,
                            format!("could not find base between {} and HEAD", base),
                        )
                    })
                    .with_code(proc_exit::sysexits::USAGE_ERR)?;
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
                    git_stack::config::Stack::All => {
                        unreachable!("Covered in another branch")
                    }
                };
                vec![StackState::new(base, onto, stack_branches)]
            }
        };

        for stack in &stacks {
            if let Some(branch) = stack.base.branch.clone() {
                protected_branches.insert(branch);
            }
            if let Some(branch) = stack.onto.branch.clone() {
                protected_branches.insert(branch);
            }
        }

        Ok(Self {
            repo,
            branches,
            protected_branches,
            head_commit,
            stacks,

            rebase,
            pull,
            push,
            fixup,
            repair,
            dry_run,
            snapshot_capacity,
            protect_commit_count,
            protect_commit_age,
            protect_commit_time,

            show_format,
            show_commits,
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

#[derive(Debug)]
struct StackState {
    base: AnnotatedOid,
    onto: AnnotatedOid,
    branches: git_stack::legacy::git::Branches,
}

impl StackState {
    fn new(
        base: AnnotatedOid,
        onto: AnnotatedOid,
        mut branches: git_stack::legacy::git::Branches,
    ) -> Self {
        if let Some(base) = &base.branch {
            branches.insert(base.clone());
        }
        if let Some(onto) = &onto.branch {
            branches.insert(onto.clone());
        }
        Self {
            base,
            onto,
            branches,
        }
    }

    fn update(&mut self, repo: &dyn git_stack::legacy::git::Repo) -> eyre::Result<()> {
        self.base.update(repo)?;
        self.onto.update(repo)?;
        self.branches.update(repo);
        Ok(())
    }
}

pub fn stack(
    args: &crate::args::Args,
    colored_stdout: bool,
    colored_stderr: bool,
) -> proc_exit::ExitResult {
    log::trace!("Initializing");
    let cwd = std::env::current_dir().with_code(proc_exit::sysexits::USAGE_ERR)?;
    let repo = git2::Repository::discover(&cwd).with_code(proc_exit::sysexits::USAGE_ERR)?;
    let repo = git_stack::legacy::git::GitRepo::new(repo);
    let mut state = State::new(repo, args)?;

    if state.pull {
        // Update status of remote unprotected branches
        let mut push_branches: Vec<_> = state
            .stacks
            .iter()
            .flat_map(|stack| stack.branches.iter())
            .filter(|(oid, _)| !state.protected_branches.contains_oid(*oid))
            .flat_map(|(_, b)| b.iter())
            .filter_map(|b| b.push_id.and_then(|_| b.local_name()))
            .collect();
        push_branches.sort_unstable();
        if !push_branches.is_empty() {
            match git_prune_development(&mut state.repo, &push_branches, state.dry_run) {
                Ok(_) => (),
                Err(err) => {
                    log::warn!("Skipping fetch of `{}`, {}", state.repo.push_remote(), err);
                }
            }
        }

        for stack in state.stacks.iter() {
            if let Some(branch) = &stack.onto.branch {
                if let Some(remote) = &branch.remote {
                    match git_fetch_upstream(remote, branch.name.as_str()) {
                        Ok(_) => (),
                        Err(err) => {
                            log::warn!("Skipping pull of `{}`, {}", branch, err);
                        }
                    }
                } else {
                    log::warn!("Skipping pull of `{}` local branch", branch);
                }
            }
        }
        state.update().with_code(proc_exit::Code::FAILURE)?;
    }

    let mut success = true;
    let mut backed_up = false;
    let mut stash_id = None;
    if state.rebase || state.fixup != git_stack::config::Fixup::Ignore || state.repair {
        if stash_id.is_none() && !state.dry_run {
            stash_id = git_stack::legacy::git::stash_push(&mut state.repo, "branch-stash");
        }
        if state.repo.is_dirty() {
            let message = "Working tree is dirty, aborting";
            if state.dry_run {
                log::error!("{}", message);
            } else {
                git_stack::legacy::git::stash_pop(&mut state.repo, stash_id);
                return Err(proc_exit::sysexits::USAGE_ERR.with_message(message));
            }
        }

        {
            let stash_repo =
                git2::Repository::discover(&cwd).with_code(proc_exit::sysexits::USAGE_ERR)?;
            let stash_repo = git_branch_stash::GitRepo::new(stash_repo);
            let mut snapshots =
                git_branch_stash::Stack::new(crate::ops::STASH_STACK_NAME, &stash_repo);
            snapshots.capacity(state.snapshot_capacity);
            let snapshot = git_branch_stash::Snapshot::from_repo(&stash_repo)
                .with_code(proc_exit::Code::FAILURE)?;
            if !state.dry_run {
                snapshots.push(snapshot).to_sysexits()?;
                backed_up = true;
            }
        }

        let mut head_branch = state
            .repo
            .head_branch()
            .ok_or_else(|| eyre::eyre!("Must not be in a detached HEAD state."))
            .with_code(proc_exit::sysexits::USAGE_ERR)?
            .name;

        let scripts: Result<Vec<_>, proc_exit::Exit> = state
            .stacks
            .iter()
            .map(|stack| {
                let script = plan_changes(&state, stack).with_code(proc_exit::Code::FAILURE)?;
                if script.is_branch_deleted(&head_branch) {
                    // Current branch is deleted, fallback to the local version of the onto branch,
                    // if possible.
                    if let Some(local_name) = stack
                        .onto
                        .branch
                        .as_ref()
                        .map(|b| b.name.as_str())
                        .filter(|n| state.repo.find_local_branch(n).is_some())
                    {
                        head_branch = local_name.to_owned();
                    }
                }
                Ok(script)
            })
            .collect();
        let scripts = scripts?;

        let mut executor = git_stack::legacy::git::Executor::new(&state.repo, state.dry_run);
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
            .close(&mut state.repo, &head_branch)
            .with_code(proc_exit::Code::FAILURE)?;
        state.update().with_code(proc_exit::Code::FAILURE)?;
    }

    if state.push {
        push(&mut state).with_code(proc_exit::Code::FAILURE)?;
        state.update().with_code(proc_exit::Code::FAILURE)?;
    }

    show(&state, colored_stdout, colored_stderr).with_code(proc_exit::Code::FAILURE)?;

    git_stack::legacy::git::stash_pop(&mut state.repo, stash_id);

    if backed_up {
        let palette_stderr = if colored_stderr {
            crate::ops::Palette::colored()
        } else {
            crate::ops::Palette::plain()
        };
        log::info!(
            "{}",
            palette_stderr.hint.paint(format_args!(
                "To undo, run `git branch-stash pop {}`",
                crate::ops::STASH_STACK_NAME
            ))
        );
    }

    if !success {
        return proc_exit::Code::FAILURE.ok();
    }

    Ok(())
}

fn plan_changes(state: &State, stack: &StackState) -> eyre::Result<git_stack::legacy::git::Script> {
    log::trace!("Planning stack changes with base={}", stack.base,);
    let graphed_branches = stack.branches.clone();
    let base_commit = state
        .repo
        .find_commit(stack.base.id)
        .expect("base branch is valid");
    let mut graph = git_stack::legacy::graph::Graph::from_branches(&state.repo, graphed_branches)?;
    graph.insert(
        &state.repo,
        git_stack::legacy::graph::Node::new(base_commit),
    )?;
    git_stack::legacy::graph::protect_branches(&mut graph, &state.repo, &state.protected_branches);
    if let Some(protect_commit_count) = state.protect_commit_count {
        git_stack::legacy::graph::protect_large_branches(&mut graph, protect_commit_count);
    }
    git_stack::legacy::graph::protect_old_branches(
        &mut graph,
        state.protect_commit_time,
        &[state.head_commit.id],
    );
    if let Some(user) = state.repo.user() {
        git_stack::legacy::graph::protect_foreign_branches(&mut graph, &user, &[]);
    }

    let mut dropped_branches = Vec::new();
    if state.rebase {
        log::trace!("Rebasing onto {}", stack.onto);
        let onto_id = stack.onto.id;
        let pull_start_id = stack.base.id;
        let pull_start_id = state
            .repo
            .merge_base(pull_start_id, onto_id)
            .unwrap_or(onto_id);

        git_stack::legacy::graph::rebase_development_branches(&mut graph, onto_id);
        git_stack::legacy::graph::rebase_pulled_branches(&mut graph, pull_start_id, onto_id);

        let pull_range: Vec<_> =
            git_stack::legacy::git::commit_range(&state.repo, onto_id..pull_start_id)?
                .into_iter()
                .map(|id| state.repo.find_commit(id).unwrap())
                .collect();
        git_stack::legacy::graph::drop_squashed_by_tree_id(
            &mut graph,
            pull_range.iter().map(|c| c.tree_id),
        );
        dropped_branches.extend(git_stack::legacy::graph::drop_merged_branches(
            &mut graph,
            pull_range.iter().map(|c| c.id),
            &state.protected_branches,
        ));
    }
    git_stack::legacy::graph::fixup(&mut graph, state.fixup);
    if state.repair {
        log::trace!("Repairing");
        git_stack::legacy::graph::merge_stacks(&mut graph);
        git_stack::legacy::graph::realign_stacks(&mut graph);
    }

    let mut script = git_stack::legacy::graph::to_script(&graph);
    script.commands.extend(
        dropped_branches
            .into_iter()
            .map(git_stack::legacy::git::Command::DeleteBranch),
    );

    Ok(script)
}

fn push(state: &mut State) -> eyre::Result<()> {
    let mut graphed_branches = git_stack::legacy::git::Branches::new(None.into_iter());
    for stack in state.stacks.iter() {
        let stack_graphed_branches = stack.branches.clone();
        graphed_branches.extend(stack_graphed_branches.into_iter().flat_map(|(_, b)| b));
    }
    let mut graph = git_stack::legacy::graph::Graph::from_branches(&state.repo, graphed_branches)?;
    graph.insert(
        &state.repo,
        git_stack::legacy::graph::Node::new(state.head_commit.clone()),
    )?;

    git_stack::legacy::graph::protect_branches(&mut graph, &state.repo, &state.protected_branches);
    if let Some(protect_commit_count) = state.protect_commit_count {
        git_stack::legacy::graph::protect_large_branches(&mut graph, protect_commit_count);
    }
    git_stack::legacy::graph::protect_old_branches(
        &mut graph,
        state.protect_commit_time,
        &[state.head_commit.id],
    );
    if let Some(user) = state.repo.user() {
        git_stack::legacy::graph::protect_foreign_branches(&mut graph, &user, &[]);
    }

    git_stack::legacy::graph::pushable(&mut graph);

    git_push(&mut state.repo, &graph, state.dry_run)?;

    Ok(())
}

fn show(state: &State, colored_stdout: bool, colored_stderr: bool) -> eyre::Result<()> {
    let palette_stderr = if colored_stderr {
        crate::ops::Palette::colored()
    } else {
        crate::ops::Palette::plain()
    };
    let mut empty_stacks = Vec::new();
    let mut old_stacks = Vec::new();
    let mut foreign_stacks = Vec::new();

    let abbrev_graph = match state.show_format {
        git_stack::config::Format::Silent => false,
        git_stack::config::Format::List => false,
        git_stack::config::Format::Graph => true,
        git_stack::config::Format::Debug => true,
    };

    let mut graphs = Vec::with_capacity(state.stacks.len());
    for stack in state.stacks.iter() {
        let graphed_branches = stack.branches.clone();
        if graphed_branches.len() == 1 && abbrev_graph {
            let branches = graphed_branches.iter().next().unwrap().1;
            if branches.len() == 1 && branches[0].id != state.head_commit.id {
                empty_stacks.push(format!("{}", palette_stderr.info.paint(&branches[0])));
                continue;
            }
        }

        log::trace!("Rendering stack base={}", stack.base,);
        let base_commit = state
            .repo
            .find_commit(stack.base.id)
            .expect("base branch is valid");
        let mut graph =
            git_stack::legacy::graph::Graph::from_branches(&state.repo, graphed_branches)?;
        graph.insert(
            &state.repo,
            git_stack::legacy::graph::Node::new(base_commit),
        )?;
        git_stack::legacy::graph::protect_branches(
            &mut graph,
            &state.repo,
            &state.protected_branches,
        );
        if let Some(protect_commit_count) = state.protect_commit_count {
            let protected =
                git_stack::legacy::graph::protect_large_branches(&mut graph, protect_commit_count);
            if !protected.is_empty() {
                log::warn!(
                    "Branches contain more than {} commits (should these be protected?): {}",
                    protect_commit_count,
                    protected.join("m ")
                );
            }
        }
        if abbrev_graph {
            old_stacks.extend(
                git_stack::legacy::graph::trim_old_branches(
                    &mut graph,
                    state.protect_commit_time,
                    &[state.head_commit.id],
                )
                .into_iter()
                .map(|b| format!("{}", palette_stderr.warn.paint(b))),
            );
            if let Some(user) = state.repo.user() {
                foreign_stacks.extend(
                    git_stack::legacy::graph::trim_foreign_branches(
                        &mut graph,
                        &user,
                        &[state.head_commit.id],
                    )
                    .into_iter()
                    .map(|b| format!("{}", palette_stderr.warn.paint(b))),
                );
                git_stack::legacy::graph::protect_foreign_branches(&mut graph, &user, &[]);
            }
        }

        if state.dry_run {
            // Show as-if we performed all mutations
            if state.rebase {
                log::trace!("Rebasing onto {}", stack.onto);
                let onto_id = stack.onto.id;
                let pull_start_id = stack.base.id;
                let pull_start_id = state
                    .repo
                    .merge_base(pull_start_id, onto_id)
                    .unwrap_or(onto_id);

                git_stack::legacy::graph::rebase_development_branches(&mut graph, onto_id);
                git_stack::legacy::graph::rebase_pulled_branches(
                    &mut graph,
                    pull_start_id,
                    onto_id,
                );

                let pull_range: Vec<_> =
                    git_stack::legacy::git::commit_range(&state.repo, onto_id..pull_start_id)?
                        .into_iter()
                        .map(|id| state.repo.find_commit(id).unwrap())
                        .collect();
                git_stack::legacy::graph::drop_squashed_by_tree_id(
                    &mut graph,
                    pull_range.iter().map(|c| c.tree_id),
                );
                git_stack::legacy::graph::drop_merged_branches(
                    &mut graph,
                    pull_range.iter().map(|c| c.id),
                    &state.protected_branches,
                );
            }
            git_stack::legacy::graph::fixup(&mut graph, state.fixup);
            if state.repair {
                log::trace!("Repairing");
                git_stack::legacy::graph::merge_stacks(&mut graph);
                git_stack::legacy::graph::realign_stacks(&mut graph);
            }
        }

        git_stack::legacy::graph::pushable(&mut graph);

        graphs.push(graph);
    }
    if graphs.is_empty() {
        log::trace!("Rendering empty stack base={}", state.head_commit.id);
        let graph = git_stack::legacy::graph::Graph::new(git_stack::legacy::graph::Node::new(
            state.head_commit.clone(),
        ));
        graphs.push(graph);
    }
    graphs.sort_by_key(|g| {
        let mut revwalk = state
            .repo
            .raw()
            .revwalk()
            .unwrap_or_else(|e| panic!("Unexpected git2 error: {}", e));
        // Reduce the number of commits to walk
        revwalk
            .simplify_first_parent()
            .unwrap_or_else(|e| panic!("Unexpected git2 error: {}", e));
        revwalk
            .push(g.root_id())
            .unwrap_or_else(|e| panic!("Unexpected git2 error: {}", e));
        revwalk.count()
    });

    for graph in graphs {
        match state.show_format {
            git_stack::config::Format::Silent => {}
            git_stack::config::Format::List => {
                let palette = if colored_stdout {
                    crate::ops::Palette::colored()
                } else {
                    crate::ops::Palette::plain()
                };
                list(
                    &mut std::io::stdout(),
                    &state.repo,
                    &graph,
                    &state.protected_branches,
                    &palette,
                )?;
            }
            git_stack::config::Format::Graph => {
                write!(
                    std::io::stdout(),
                    "{}",
                    DisplayTree::new(&state.repo, &graph)
                        .colored(colored_stdout)
                        .show(state.show_commits)
                        .stacked(state.show_stacked)
                        .protected_branches(&state.protected_branches)
                )?;
            }
            git_stack::config::Format::Debug => {
                writeln!(std::io::stdout(), "{:#?}", graph)?;
            }
        }
    }

    if !empty_stacks.is_empty() {
        log::info!("Empty stacks: {}", empty_stacks.join(", "));
    }
    if !old_stacks.is_empty() {
        log::info!(
            "Stacks older than {}: {}",
            humantime::format_duration(state.protect_commit_age),
            old_stacks.join(", ")
        );
    }
    if !foreign_stacks.is_empty() {
        log::info!("Stack from other users: {}", foreign_stacks.join(", "));
    }

    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct AnnotatedOid {
    id: git2::Oid,
    branch: Option<git_stack::legacy::git::Branch>,
}

impl AnnotatedOid {
    fn new(id: git2::Oid) -> Self {
        Self { id, branch: None }
    }

    fn with_branch(branch: git_stack::legacy::git::Branch) -> Self {
        Self {
            id: branch.id,
            branch: Some(branch),
        }
    }

    fn update(&mut self, repo: &dyn git_stack::legacy::git::Repo) -> eyre::Result<()> {
        let branch = self.branch.as_ref().and_then(|branch| {
            if let Some(remote) = &branch.remote {
                repo.find_remote_branch(remote, &branch.name)
            } else {
                repo.find_local_branch(&branch.name)
            }
        });
        if let Some(branch) = branch {
            *self = Self::with_branch(branch);
        }
        Ok(())
    }
}

impl std::fmt::Display for AnnotatedOid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(branch) = &self.branch {
            branch.fmt(f)
        } else {
            self.id.fmt(f)
        }
    }
}

fn resolve_explicit_base(
    repo: &git_stack::legacy::git::GitRepo,
    base: &str,
) -> eyre::Result<AnnotatedOid> {
    let (obj, r) = repo.raw().revparse_ext(base)?;
    if let Some(r) = r {
        if r.is_tag() {
            return Ok(AnnotatedOid::new(obj.id()));
        }

        let branch = if r.is_remote() {
            let (remote, name) = r
                .shorthand()
                .ok_or_else(|| eyre::eyre!("Expected branch, got `{}`", base))?
                .split_once('/')
                .expect("removes should always have at least one `/`");
            repo.find_remote_branch(remote, name)
                .ok_or_else(|| eyre::eyre!("Could not find branch {:?}", r.shorthand()))
        } else {
            repo.find_local_branch(base)
                .ok_or_else(|| eyre::eyre!("Could not find branch {:?}", base))
        }?;
        Ok(AnnotatedOid::with_branch(branch))
    } else {
        Ok(AnnotatedOid::new(obj.id()))
    }
}

fn resolve_implicit_base(
    repo: &dyn git_stack::legacy::git::Repo,
    head_oid: git2::Oid,
    branches: &git_stack::legacy::git::Branches,
    protected_branches: &git_stack::legacy::git::Branches,
    auto_base_commit_count: Option<usize>,
) -> AnnotatedOid {
    match git_stack::legacy::git::find_protected_base(repo, protected_branches, head_oid) {
        Some(branch) => {
            let merge_base_id = repo
                .merge_base(branch.id, head_oid)
                .expect("to be a base, there must be a merge base");
            if let Some(max_commit_count) = auto_base_commit_count {
                let ahead_count = repo
                    .commit_count(merge_base_id, head_oid)
                    .expect("merge_base should ensure a count exists ");
                let behind_count = repo
                    .commit_count(merge_base_id, branch.id)
                    .expect("merge_base should ensure a count exists ");
                if max_commit_count <= ahead_count + behind_count {
                    let assumed_base_oid =
                        git_stack::legacy::git::infer_base(repo, head_oid).unwrap_or(head_oid);
                    log::warn!(
                        "{} is {} ahead and {} behind {}, using {} as --base instead",
                        branches
                            .get(head_oid)
                            .map(|b| b[0].to_string())
                            .or_else(|| {
                                repo.find_commit(head_oid)?
                                    .summary
                                    .to_str()
                                    .ok()
                                    .map(ToOwned::to_owned)
                            })
                            .unwrap_or_else(|| "target".to_owned()),
                        ahead_count,
                        behind_count,
                        branch,
                        assumed_base_oid
                    );
                    return AnnotatedOid::new(assumed_base_oid);
                }
            }

            log::debug!(
                "Chose branch {} as the base for {}",
                branch,
                branches
                    .get(head_oid)
                    .map(|b| b[0].to_string())
                    .or_else(|| {
                        repo.find_commit(head_oid)?
                            .summary
                            .to_str()
                            .ok()
                            .map(ToOwned::to_owned)
                    })
                    .unwrap_or_else(|| "target".to_owned())
            );
            AnnotatedOid::with_branch(branch.to_owned())
        }
        None => {
            let assumed_base_oid =
                git_stack::legacy::git::infer_base(repo, head_oid).unwrap_or(head_oid);
            log::warn!(
                "Could not find protected branch for {}, assuming {}",
                head_oid,
                assumed_base_oid
            );
            AnnotatedOid::new(assumed_base_oid)
        }
    }
}

fn resolve_base_from_onto(
    repo: &git_stack::legacy::git::GitRepo,
    onto: &AnnotatedOid,
) -> AnnotatedOid {
    // HACK: Assuming the local branch is the current base for all the commits
    onto.branch
        .as_ref()
        .filter(|b| b.remote.is_some())
        .and_then(|b| repo.find_local_branch(&b.name))
        .map(AnnotatedOid::with_branch)
        .unwrap_or_else(|| onto.clone())
}

fn resolve_onto_from_base(
    repo: &git_stack::legacy::git::GitRepo,
    base: &AnnotatedOid,
) -> AnnotatedOid {
    // HACK: Assuming the local branch is the current base for all the commits
    base.branch
        .as_ref()
        .filter(|b| b.remote.is_none())
        .and_then(|b| repo.find_remote_branch(repo.pull_remote(), &b.name))
        .map(AnnotatedOid::with_branch)
        .unwrap_or_else(|| base.clone())
}

fn git_prune_development(
    repo: &mut git_stack::legacy::git::GitRepo,
    branches: &[&str],
    dry_run: bool,
) -> eyre::Result<()> {
    if branches.is_empty() {
        return Ok(());
    }

    let remote = repo.push_remote();
    let output = std::process::Command::new("git")
        .arg("ls-remote")
        .arg("--heads")
        .arg(remote)
        .args(branches)
        .stdout(std::process::Stdio::piped())
        .spawn()
        .wrap_err("Could not run `git fetch`")?
        .wait_with_output()?;
    if !output.status.success() {
        eyre::bail!("Could not run `git fetch`");
    }
    let stdout = String::from_utf8(output.stdout).wrap_err("Could not run `git fetch`")?;
    #[allow(clippy::needless_collect)]
    let remote_branches: Vec<_> = stdout
        .lines()
        .filter_map(|l| l.split_once('\t').map(|s| s.1))
        .filter_map(|l| l.strip_prefix("refs/heads/"))
        .collect();

    for branch in branches {
        if !remote_branches.contains(branch) {
            let remote_branch = format!("{}/{}", remote, branch);
            log::info!("Pruning {}", remote_branch);
            if !dry_run {
                let mut branch = repo
                    .raw()
                    .find_branch(&remote_branch, git2::BranchType::Remote)?;
                branch.delete()?;
            }
        }
    }

    Ok(())
}

fn git_fetch_upstream(remote: &str, branch_name: &str) -> eyre::Result<()> {
    log::debug!("git fetch {} {}", remote, branch_name);
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

    Ok(())
}

fn git_push(
    repo: &mut git_stack::legacy::git::GitRepo,
    graph: &git_stack::legacy::graph::Graph,
    dry_run: bool,
) -> eyre::Result<()> {
    let mut failed = Vec::new();

    let mut node_queue = VecDeque::new();
    node_queue.push_back(graph.root_id());
    while let Some(current_id) = node_queue.pop_front() {
        let current = graph.get(current_id).expect("all children exist");

        failed.extend(git_push_node(repo, current, dry_run));

        for child_id in current.children.iter().copied() {
            node_queue.push_back(child_id);
        }
    }

    if failed.is_empty() {
        Ok(())
    } else {
        eyre::bail!("Could not push {}", failed.into_iter().join(", "));
    }
}

fn git_push_node(
    repo: &mut git_stack::legacy::git::GitRepo,
    node: &git_stack::legacy::graph::Node,
    dry_run: bool,
) -> Vec<String> {
    let mut failed = Vec::new();
    for branch in node.branches.iter() {
        let local_branch = if let Some(local_name) = branch.local_name() {
            local_name
        } else {
            continue;
        };

        if node.pushable {
            let raw_branch = repo
                .raw()
                .find_branch(local_branch, git2::BranchType::Local)
                .expect("all referenced branches exist");
            let upstream_set = raw_branch.upstream().is_ok();

            let remote = repo.push_remote();
            let mut args = vec!["push", "--force-with-lease"];
            if !upstream_set {
                args.push("--set-upstream");
            }
            args.push(remote);
            args.push(local_branch);
            log::trace!("git {}", args.join(" "),);
            if !dry_run {
                let status = std::process::Command::new("git").args(&args).status();
                match status {
                    Ok(status) => {
                        if !status.success() {
                            failed.push(local_branch.to_owned());
                        }
                    }
                    Err(err) => {
                        log::debug!("`git push` failed with {}", err);
                        failed.push(local_branch.to_owned());
                    }
                }
            }
        } else if node.action.is_protected() {
            log::debug!("Skipping push of `{}`, protected", branch);
        } else {
            log::debug!("Skipping push of `{}`", branch);
        }
    }

    failed
}

fn list(
    writer: &mut dyn std::io::Write,
    repo: &git_stack::legacy::git::GitRepo,
    graph: &git_stack::legacy::graph::Graph,
    protected_branches: &git_stack::legacy::git::Branches,
    palette: &crate::ops::Palette,
) -> Result<(), std::io::Error> {
    let head_branch = repo.head_branch().unwrap();
    for node in graph.breadth_first_iter() {
        let protected = protected_branches.get(node.commit.id);
        let mut branches: Vec<_> = node.branches.iter().collect();
        branches.sort();
        for b in branches {
            if b.remote.is_some() || protected.into_iter().flatten().contains(&b) {
                // Base, remote, and protected branches are just shown for context, they aren't part of the
                // stack, so skip them here
                continue;
            }
            writeln!(
                writer,
                "{}",
                format_branch_name(b, node, &head_branch, protected_branches, palette)
            )?;
        }
    }

    Ok(())
}

struct DisplayTree<'r> {
    repo: &'r git_stack::legacy::git::GitRepo,
    graph: &'r git_stack::legacy::graph::Graph,
    protected_branches: git_stack::legacy::git::Branches,
    palette: crate::ops::Palette,
    show: git_stack::config::ShowCommits,
    stacked: bool,
}

impl<'r> DisplayTree<'r> {
    pub fn new(
        repo: &'r git_stack::legacy::git::GitRepo,
        graph: &'r git_stack::legacy::graph::Graph,
    ) -> Self {
        Self {
            repo,
            graph,
            protected_branches: Default::default(),
            palette: crate::ops::Palette::plain(),
            show: Default::default(),
            stacked: Default::default(),
        }
    }

    pub fn colored(mut self, yes: bool) -> Self {
        if yes {
            self.palette = crate::ops::Palette::colored()
        } else {
            self.palette = crate::ops::Palette::plain()
        }
        self
    }

    pub fn show(mut self, show: git_stack::config::ShowCommits) -> Self {
        self.show = show;
        self
    }

    pub fn stacked(mut self, stacked: bool) -> Self {
        self.stacked = stacked;
        self
    }

    pub fn protected_branches(
        mut self,
        protected_branches: &git_stack::legacy::git::Branches,
    ) -> Self {
        self.protected_branches = protected_branches.clone();
        self
    }
}

impl<'r> std::fmt::Display for DisplayTree<'r> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        let head_branch = self.repo.head_branch().unwrap();

        let is_visible: Box<dyn Fn(&git_stack::legacy::graph::Node) -> bool> = match self.show {
            git_stack::config::ShowCommits::All => Box::new(|_| true),
            git_stack::config::ShowCommits::Unprotected => Box::new(|node| {
                let interesting_commit = node.commit.id == head_branch.id
                    || node.commit.id == self.graph.root_id()
                    || node.children.is_empty();
                let boring_commit = node.branches.is_empty() && node.children.len() == 1;
                let protected = node.action.is_protected();
                interesting_commit || !boring_commit || !protected
            }),
            git_stack::config::ShowCommits::None => Box::new(|node| {
                let interesting_commit = node.commit.id == head_branch.id
                    || node.commit.id == self.graph.root_id()
                    || node.children.is_empty();
                let boring_commit = node.branches.is_empty() && node.children.len() == 1;
                interesting_commit || !boring_commit
            }),
        };

        let mut tree = node_to_tree(
            self.repo,
            &head_branch,
            self.graph,
            self.graph.root_id(),
            &is_visible,
        );
        if self.stacked {
            tree.linearize();
        } else {
            tree.sort();
        }
        let tree = tree.into_display(
            self.repo,
            &head_branch,
            &self.protected_branches,
            &self.palette,
        );
        tree.fmt(f)
    }
}

fn node_to_tree<'r>(
    repo: &'r git_stack::legacy::git::GitRepo,
    head_branch: &'r git_stack::legacy::git::Branch,
    graph: &'r git_stack::legacy::graph::Graph,
    mut node_id: git2::Oid,
    is_visible: &dyn Fn(&git_stack::legacy::graph::Node) -> bool,
) -> Tree<'r> {
    for ellide_count in 0.. {
        let node = graph.get(node_id).expect("all children exist");
        // The API requires us to handle 0 or many children, so not checking visibility
        if node.children.len() == 1 && !is_visible(node) {
            node_id = node.children.iter().copied().next().unwrap();
            continue;
        }

        let mut tree = Tree {
            root: node,
            weight: default_weight(node, head_branch),
            stacks: Default::default(),
        };

        append_children(&mut tree, repo, head_branch, graph, node, is_visible);

        tree.weight += ellide_count;

        return tree;
    }

    unreachable!("above loop always hits `return`")
}

fn append_children<'r>(
    tree: &mut Tree<'r>,
    repo: &'r git_stack::legacy::git::GitRepo,
    head_branch: &'r git_stack::legacy::git::Branch,
    graph: &'r git_stack::legacy::graph::Graph,
    mut parent_node: &'r git_stack::legacy::graph::Node,
    is_visible: &dyn Fn(&git_stack::legacy::graph::Node) -> bool,
) {
    match parent_node.children.len() {
        0 => {}
        1 => {
            for linear_count in 1.. {
                let node_id = *parent_node.children.iter().next().unwrap();
                let node = graph.get(node_id).expect("all children exist");
                match node.children.len() {
                    0 => {
                        let child_tree = Tree {
                            root: node,
                            weight: default_weight(node, head_branch),
                            stacks: Default::default(),
                        };
                        tree.weight = tree.weight.max(child_tree.weight + linear_count);
                        if tree.stacks.is_empty() {
                            tree.stacks.push(Vec::new());
                        }
                        tree.stacks[0].push(child_tree);
                        break;
                    }
                    1 => {
                        if is_visible(node) {
                            let child_tree = Tree {
                                root: node,
                                weight: default_weight(node, head_branch),
                                stacks: Default::default(),
                            };
                            // `tree.weight`: rely on a terminating case for updating
                            if tree.stacks.is_empty() {
                                tree.stacks.push(Vec::new());
                            }
                            tree.stacks[0].push(child_tree);
                        }
                        parent_node = node;
                        continue;
                    }
                    _ => {
                        let child_tree =
                            node_to_tree(repo, head_branch, graph, node_id, is_visible);
                        tree.weight = tree.weight.max(child_tree.weight + linear_count);
                        if tree.stacks.is_empty() {
                            tree.stacks.push(Vec::new());
                        }
                        tree.stacks[0].push(child_tree);
                        break;
                    }
                }
            }
        }
        _ => {
            for child_id in parent_node.children.iter().copied() {
                let child_tree = node_to_tree(repo, head_branch, graph, child_id, is_visible);
                tree.weight = tree.weight.max(child_tree.weight + 1);
                tree.stacks.push(vec![child_tree]);
            }
        }
    }
}

fn default_weight(
    node: &git_stack::legacy::graph::Node,
    head_branch: &git_stack::legacy::git::Branch,
) -> Weight {
    if node.action.is_protected() {
        Weight::Protected(0)
    } else if node.commit.id == head_branch.id {
        Weight::Head(0)
    } else {
        Weight::Commit(0)
    }
}

#[derive(Debug)]
struct Tree<'r> {
    root: &'r git_stack::legacy::graph::Node,
    stacks: Vec<Vec<Self>>,
    weight: Weight,
}

impl<'r> Tree<'r> {
    fn sort(&mut self) {
        self.stacks.sort_by_key(|s| s[0].weight);
        for stack in self.stacks.iter_mut() {
            for child in stack.iter_mut() {
                child.sort();
            }
        }
    }

    fn linearize(&mut self) {
        self.stacks.sort_by_key(|s| s[0].weight);
        for stack in self.stacks.iter_mut() {
            for child in stack.iter_mut() {
                child.linearize();
            }
            let append = {
                let last = stack.last_mut().expect("stack always has at least 1");
                if last.stacks.is_empty() {
                    None
                } else {
                    last.stacks.pop()
                }
            };
            stack.extend(append.into_iter().flatten());
        }
    }

    fn into_display(
        self,
        repo: &'r git_stack::legacy::git::GitRepo,
        head_branch: &'r git_stack::legacy::git::Branch,
        protected_branches: &'r git_stack::legacy::git::Branches,
        palette: &'r crate::ops::Palette,
    ) -> termtree::Tree<RenderNode<'r>> {
        let root = RenderNode {
            repo,
            head_branch,
            protected_branches,
            node: Some(self.root),
            palette,
        };
        let mut tree = termtree::Tree::new(root).with_glyphs(GLYPHS);
        let joint = RenderNode {
            repo,
            head_branch,
            protected_branches,
            node: None,
            palette,
        };
        let stacks_len = self.stacks.len();
        for (i, stack) in self.stacks.into_iter().enumerate() {
            if i < stacks_len - 1 {
                let mut stack_tree = termtree::Tree::new(joint).with_glyphs(JOINT_GLYPHS);
                for child_tree in stack.into_iter() {
                    stack_tree.push(child_tree.into_display(
                        repo,
                        head_branch,
                        protected_branches,
                        palette,
                    ));
                }
                tree.push(stack_tree);
            } else {
                let stack_len = stack.len();
                for (j, child_tree) in stack.into_iter().enumerate() {
                    if i != 0 && j == 0 {
                        tree.push(termtree::Tree::new(joint).with_glyphs(SPACE_GLYPHS));
                    }
                    let child = RenderNode {
                        repo,
                        head_branch,
                        protected_branches,
                        node: Some(child_tree.root),
                        palette,
                    };
                    tree.push(termtree::Tree::new(child).with_glyphs(GLYPHS));
                    if !child_tree.stacks.is_empty() {
                        for child_stack in child_tree.stacks.into_iter() {
                            let mut stack_tree =
                                termtree::Tree::new(joint).with_glyphs(JOINT_GLYPHS);
                            for child_tree in child_stack.into_iter() {
                                stack_tree.push(child_tree.into_display(
                                    repo,
                                    head_branch,
                                    protected_branches,
                                    palette,
                                ));
                            }
                            tree.push(stack_tree);
                        }
                        if j < stack_len {
                            tree.push(termtree::Tree::new(joint).with_glyphs(SPACE_GLYPHS));
                        }
                    }
                }
            }
        }
        tree
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Weight {
    Commit(usize),
    Head(usize),
    Protected(usize),
}

impl Weight {
    fn max(self, other: Self) -> Self {
        match (self, other) {
            (Self::Protected(s), Self::Protected(o)) => Self::Protected(s.max(o)),
            (Self::Protected(s), _) => Self::Protected(s),
            (_, Self::Protected(o)) => Self::Protected(o),
            (Self::Head(s), Self::Head(o)) => Self::Head(s.max(o)),
            (Self::Head(s), _) => Self::Head(s),
            (_, Self::Head(s)) => Self::Head(s),
            (Self::Commit(s), Self::Commit(o)) => Self::Commit(s.max(o)),
        }
    }
}

impl std::ops::Add<usize> for Weight {
    type Output = Self;

    fn add(self, other: usize) -> Self {
        match self {
            Self::Protected(s) => Self::Protected(s.saturating_add(other)),
            Self::Head(s) => Self::Head(s.saturating_add(other)),
            Self::Commit(s) => Self::Commit(s.saturating_add(other)),
        }
    }
}

impl std::ops::AddAssign<usize> for Weight {
    fn add_assign(&mut self, other: usize) {
        *self = *self + other;
    }
}

#[derive(Copy, Clone, Debug)]
struct RenderNode<'r> {
    repo: &'r git_stack::legacy::git::GitRepo,
    head_branch: &'r git_stack::legacy::git::Branch,
    protected_branches: &'r git_stack::legacy::git::Branches,
    node: Option<&'r git_stack::legacy::graph::Node>,
    palette: &'r crate::ops::Palette,
}

const GLYPHS: termtree::GlyphPalette = termtree::GlyphPalette {
    middle_item: "⌽",
    last_item: "⌽",
    item_indent: " ",
    skip_indent: " ",
    ..termtree::GlyphPalette::new()
};

const SPACE_GLYPHS: termtree::GlyphPalette = termtree::GlyphPalette {
    middle_item: "│",
    last_item: " ",
    item_indent: " ",
    skip_indent: " ",
    ..termtree::GlyphPalette::new()
};

const JOINT_GLYPHS: termtree::GlyphPalette = termtree::GlyphPalette {
    item_indent: "─┐",
    ..termtree::GlyphPalette::new()
};

// Shared implementation doesn't mean shared requirements, we want to track according to
// requirements
#[allow(clippy::if_same_then_else)]
impl<'r> std::fmt::Display for RenderNode<'r> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        if let Some(node) = self.node.as_ref() {
            if node.branches.is_empty() {
                let abbrev_id = self
                    .repo
                    .raw()
                    .find_object(node.commit.id, None)
                    .unwrap_or_else(|e| panic!("Unexpected git2 error: {}", e))
                    .short_id()
                    .unwrap_or_else(|e| panic!("Unexpected git2 error: {}", e));
                let style = if self.head_branch.id == node.commit.id {
                    self.palette.highlight
                } else if node.action.is_protected() {
                    self.palette.info
                } else if 1 < node.children.len() {
                    // Branches should be off of other branches
                    self.palette.warn
                } else {
                    self.palette.hint
                };
                write!(f, "{}", style.paint(abbrev_id.as_str().unwrap()))?;
            } else {
                let mut branches: Vec<_> = node.branches.iter().collect();
                branches.sort_by_key(|b| {
                    let is_head = self.head_branch.id == b.id
                        && self.head_branch.remote == b.remote
                        && self.head_branch.name == b.name;
                    let head_first = !is_head;
                    (head_first, &b.remote, &b.name)
                });
                write!(
                    f,
                    "{}",
                    branches
                        .iter()
                        .filter(|b| {
                            if b.remote.is_some() {
                                let local_present = branches
                                    .iter()
                                    .any(|b| b.local_name() == Some(b.name.as_str()));
                                !local_present
                            } else {
                                true
                            }
                        })
                        .map(|b| {
                            format!(
                                "{}{}",
                                format_branch_name(
                                    b,
                                    node,
                                    self.head_branch,
                                    self.protected_branches,
                                    self.palette
                                ),
                                format_branch_status(b, self.repo, node, self.palette),
                            )
                        })
                        .join(", ")
                )?;
            }

            write!(
                f,
                "{} ",
                format_commit_status(self.repo, node, self.palette)
            )?;

            let summary = String::from_utf8_lossy(&node.commit.summary);
            if node.action.is_protected() {
                write!(f, "{}", self.palette.hint.paint(summary))?;
            } else if node.commit.fixup_summary().is_some() {
                // Needs to be squashed
                write!(f, "{}", self.palette.warn.paint(summary))?;
            } else if node.commit.wip_summary().is_some() {
                // Not for pushing implicitly
                write!(f, "{}", self.palette.error.paint(summary))?;
            } else {
                write!(f, "{}", summary)?;
            }
        }
        Ok(())
    }
}

fn format_branch_name<'d>(
    branch: &'d git_stack::legacy::git::Branch,
    node: &'d git_stack::legacy::graph::Node,
    head_branch: &'d git_stack::legacy::git::Branch,
    protected_branches: &'d git_stack::legacy::git::Branches,
    palette: &'d crate::ops::Palette,
) -> impl std::fmt::Display + 'd {
    if head_branch.id == branch.id
        && head_branch.remote == branch.remote
        && head_branch.name == branch.name
    {
        palette.highlight.paint(branch.to_string())
    } else {
        let protected = protected_branches.get(branch.id);
        if protected.into_iter().flatten().contains(&branch) {
            palette.info.paint(branch.to_string())
        } else if branch.remote.is_some() {
            palette.info.paint(branch.to_string())
        } else if node.action.is_protected() {
            // Either haven't started dev or it got merged
            palette.warn.paint(branch.to_string())
        } else {
            palette.good.paint(branch.to_string())
        }
    }
}

fn format_branch_status<'d>(
    branch: &'d git_stack::legacy::git::Branch,
    repo: &'d git_stack::legacy::git::GitRepo,
    node: &'d git_stack::legacy::graph::Node,
    palette: &'d crate::ops::Palette,
) -> String {
    // See format_commit_status
    if node.action.is_protected() {
        if branch.pull_id.is_none() {
            format!(" {}", palette.warn.paint("(no remote)"))
        } else {
            String::new()
        }
    } else if node.action.is_delete() {
        String::new()
    } else if 1 < repo
        .raw()
        .find_commit(node.commit.id)
        .unwrap_or_else(|e| panic!("Unexpected git2 error: {}", e))
        .parent_count()
    {
        String::new()
    } else {
        if node.branches.is_empty() {
            String::new()
        } else {
            let branch = &node.branches[0];
            match commit_relation(repo, branch.id, branch.push_id) {
                Some((0, 0)) => {
                    format!(" {}", palette.good.paint("(pushed)"))
                }
                Some((local, 0)) => {
                    format!(" {}", palette.info.paint(format!("({} ahead)", local)))
                }
                Some((0, remote)) => {
                    format!(" {}", palette.warn.paint(format!("({} behind)", remote)))
                }
                Some((local, remote)) => {
                    format!(
                        " {}",
                        palette
                            .warn
                            .paint(format!("({} ahead, {} behind)", local, remote)),
                    )
                }
                None => {
                    if node.pushable {
                        format!(" {}", palette.info.paint("(ready)"))
                    } else {
                        String::new()
                    }
                }
            }
        }
    }
}

fn format_commit_status<'d>(
    repo: &'d git_stack::legacy::git::GitRepo,
    node: &'d git_stack::legacy::graph::Node,
    palette: &'d crate::ops::Palette,
) -> String {
    // See format_branch_status
    if node.action.is_protected() {
        String::new()
    } else if node.action.is_delete() {
        format!(" {}", palette.error.paint("(drop)"))
    } else if 1 < repo
        .raw()
        .find_commit(node.commit.id)
        .unwrap_or_else(|e| panic!("Unexpected git2 error: {}", e))
        .parent_count()
    {
        format!(" {}", palette.error.paint("(merge commit)"))
    } else {
        String::new()
    }
}

fn commit_relation(
    repo: &git_stack::legacy::git::GitRepo,
    local: git2::Oid,
    remote: Option<git2::Oid>,
) -> Option<(usize, usize)> {
    let remote = remote?;
    if local == remote {
        return Some((0, 0));
    }

    let base = repo.merge_base(local, remote)?;
    let local_count = repo.commit_count(base, local)?;
    let remote_count = repo.commit_count(base, remote)?;
    Some((local_count, remote_count))
}
