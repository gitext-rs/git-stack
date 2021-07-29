use std::io::Write;

use eyre::WrapErr;
use itertools::Itertools;
use proc_exit::WithCodeResultExt;

struct State {
    repo: git_stack::git::GitRepo,
    branches: git_stack::git::Branches,
    protected_branches: git_stack::git::Branches,
    head_commit: std::rc::Rc<git_stack::git::Commit>,
    base: git_stack::git::Branch,
    onto: git_stack::git::Branch,

    rebase: bool,
    pull: bool,
    pull_remote: String,
    stack: git_stack::config::Stack,
    protected: git_stack::git::ProtectedBranches,
    dry_run: bool,

    show_format: git_stack::config::Format,
    show_stacked: bool,
}

impl State {
    fn new(
        repo: git_stack::git::GitRepo,
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
        let pull_remote = repo_config.pull_remote().to_owned();
        let stack = repo_config.stack();
        let protected = git_stack::git::ProtectedBranches::new(
            repo_config.protected_branches().iter().map(|s| s.as_str()),
        )
        .with_code(proc_exit::Code::CONFIG_ERR)?;
        let dry_run = args.dry_run;

        let show_format = repo_config.show_format();
        let show_stacked = repo_config.show_stacked();

        let branches = git_stack::git::Branches::new(repo.local_branches());
        let protected_branches = branches.protected(&protected);
        let head_commit = repo.head_commit();
        let base = args
            .base
            .as_deref()
            .map(|name| resolve_explicit_base(&repo, name))
            .transpose()
            .with_code(proc_exit::Code::USAGE_ERR)?;
        let base = base
            .map(Result::Ok)
            .unwrap_or_else(|| resolve_implicit_base(&repo, head_commit.id, &protected_branches))
            .with_code(proc_exit::Code::USAGE_ERR)?;
        let onto = args
            .onto
            .as_deref()
            .map(|name| resolve_explicit_base(&repo, name))
            .transpose()
            .with_code(proc_exit::Code::USAGE_ERR)?
            .unwrap_or_else(|| base.clone());

        Ok(Self {
            repo,
            branches,
            protected_branches,
            head_commit,
            base,
            onto,

            rebase,
            pull,
            pull_remote,
            stack,
            dry_run,
            protected,
            show_format,
            show_stacked,
        })
    }

    fn update(&mut self) -> eyre::Result<()> {
        let head_commit = self.repo.head_commit();
        let branches = git_stack::git::Branches::new(self.repo.local_branches());
        let protected_branches = branches.protected(&self.protected);
        let base = self
            .repo
            .find_local_branch(self.base.name.as_str())
            .ok_or_else(|| eyre::eyre!("can no longer find branch {}", self.base.name))?;
        let onto = self
            .repo
            .find_local_branch(self.onto.name.as_str())
            .ok_or_else(|| eyre::eyre!("can no longer find branch {}", self.onto.name))?;

        self.head_commit = head_commit;
        self.branches = branches;
        self.protected_branches = protected_branches;
        self.base = base;
        self.onto = onto;

        Ok(())
    }
}

pub fn stack(args: &crate::args::Args, colored_stdout: bool) -> proc_exit::ExitResult {
    log::trace!("Initializing");
    let cwd = std::env::current_dir().with_code(proc_exit::Code::USAGE_ERR)?;
    let repo = git2::Repository::discover(&cwd).with_code(proc_exit::Code::USAGE_ERR)?;
    let repo = git_stack::git::GitRepo::new(repo);
    let mut state = State::new(repo, args)?;

    if state.rebase {
        let head_oid = state.repo.head_commit().id;
        let head_branch = if let Some(branches) = state.branches.get(head_oid) {
            branches[0].clone()
        } else {
            return Err(eyre::eyre!("Must not be in a detached HEAD state."))
                .with_code(proc_exit::Code::USAGE_ERR);
        };

        let script = plan_rebase(&mut state).with_code(proc_exit::Code::FAILURE)?;

        let mut executor = git_stack::git::Executor::new(&state.repo, state.dry_run);
        let results = executor.run_script(&mut state.repo, &script);
        for (err, name, dependents) in results.iter() {
            log::error!("Failed to re-stack branch `{}`: {}", name, err);
            if !dependents.is_empty() {
                log::error!("  Blocked dependents: {}", dependents.iter().join(", "));
            }
        }
        executor
            .close(&mut state.repo, &head_branch.name)
            .with_code(proc_exit::Code::FAILURE)?;
        if !results.is_empty() {
            return proc_exit::Code::FAILURE.ok();
        }
    }

    show(&state, colored_stdout).with_code(proc_exit::Code::FAILURE)?;

    Ok(())
}

fn plan_rebase(state: &mut State) -> eyre::Result<git_stack::git::Script> {
    let head_oid = state.head_commit.id;

    let onto_oid = if state.pull {
        if state.protected_branches.contains_oid(state.onto.id) {
            match git_pull(
                &mut state.repo,
                &state.pull_remote,
                state.onto.name.as_str(),
            ) {
                Ok(onto_oid) => {
                    state.update()?;
                    onto_oid
                }
                Err(err) => {
                    log::warn!("Skipping pull, {}", err);
                    state.onto.id
                }
            }
        } else {
            log::warn!(
                "Skipping pull, `{}` isn't a protected branch",
                state.onto.name
            );
            state.onto.id
        }
    } else {
        state.onto.id
    };

    let merge_base_oid = state
        .repo
        .merge_base(state.base.id, head_oid)
        .ok_or_else(|| {
            git2::Error::new(
                git2::ErrorCode::NotFound,
                git2::ErrorClass::Reference,
                format!("could not find base between {} and HEAD", state.base.name),
            )
        })?;
    let mut root = match state.stack {
        git_stack::config::Stack::Current => {
            let graphed_branches = state.branches.branch(&state.repo, merge_base_oid, head_oid);
            graph(&state.repo, merge_base_oid, head_oid, graphed_branches)?
        }
        git_stack::config::Stack::Dependents => {
            let graphed_branches = state
                .branches
                .dependents(&state.repo, merge_base_oid, head_oid);
            graph(&state.repo, merge_base_oid, head_oid, graphed_branches)?
        }
        git_stack::config::Stack::Descendants => {
            let graphed_branches = state.branches.descendants(&state.repo, merge_base_oid);
            graph(&state.repo, merge_base_oid, head_oid, graphed_branches)?
        }
        git_stack::config::Stack::All => {
            let mut graphed_branches = state.branches.all();
            let root =
                git_stack::graph::Node::new(state.head_commit.clone(), &mut graphed_branches);
            root.extend(&state.repo, graphed_branches)?
        }
    };

    git_stack::graph::protect_branches(&mut root, &state.repo, &state.protected_branches)?;

    git_stack::graph::rebase_branches(&mut root, onto_oid)?;

    // TODO Identify commits to drop by tree id
    // TODO Identify commits to drop by guessing
    // TODO Snap branches to be on branches
    // TODO Re-arrange fixup commits
    // TODO Re-stack branches that have been individually rebased
    git_stack::graph::delinearize(&mut root);

    let script = git_stack::graph::to_script(&root);

    Ok(script)
}

fn show(state: &State, colored_stdout: bool) -> eyre::Result<()> {
    let head_commit = state.repo.head_commit();
    let head_oid = state.head_commit.id;
    let merge_base_oid = state
        .repo
        .merge_base(state.base.id, head_oid)
        .ok_or_else(|| {
            git2::Error::new(
                git2::ErrorCode::NotFound,
                git2::ErrorClass::Reference,
                format!("could not find base between {} and HEAD", state.base.name),
            )
        })?;

    let mut root = match state.stack {
        git_stack::config::Stack::Current => {
            let mut graphed_branches = state.branches.branch(&state.repo, merge_base_oid, head_oid);
            if !graphed_branches.contains_oid(state.base.id) {
                graphed_branches.insert(state.base.clone());
            }
            graph(&state.repo, merge_base_oid, head_oid, graphed_branches)?
        }
        git_stack::config::Stack::Dependents => {
            let mut graphed_branches =
                state
                    .branches
                    .dependents(&state.repo, merge_base_oid, head_oid);
            if !graphed_branches.contains_oid(state.base.id) {
                graphed_branches.insert(state.base.clone());
            }
            graph(&state.repo, merge_base_oid, head_oid, graphed_branches)?
        }
        git_stack::config::Stack::Descendants => {
            let graphed_branches = state.branches.descendants(&state.repo, merge_base_oid);
            graph(&state.repo, merge_base_oid, head_oid, graphed_branches)?
        }
        git_stack::config::Stack::All => {
            let mut graphed_branches = state.branches.all();
            let root = git_stack::graph::Node::new(head_commit.clone(), &mut graphed_branches);
            root.extend(&state.repo, graphed_branches)?
        }
    };
    git_stack::graph::protect_branches(&mut root, &state.repo, &state.protected_branches)?;
    // TODO: Show unblocked branches
    if !state.show_stacked {
        git_stack::graph::delinearize(&mut root);
    }

    match state.show_format {
        git_stack::config::Format::Silent => (),
        git_stack::config::Format::Brief => {
            writeln!(
                std::io::stdout(),
                "{}",
                root.display().colored(colored_stdout).all(false)
            )?;
        }
        git_stack::config::Format::Full => {
            writeln!(
                std::io::stdout(),
                "{}",
                root.display().colored(colored_stdout).all(true)
            )?;
        }
    }

    Ok(())
}

fn graph(
    repo: &dyn git_stack::git::Repo,
    base_id: git2::Oid,
    head_id: git2::Oid,
    mut graph_branches: git_stack::git::Branches,
) -> eyre::Result<git_stack::graph::Node> {
    let head_commit = repo.find_commit(head_id).unwrap();
    let mut root = git_stack::graph::Node::new(head_commit, &mut graph_branches);
    root = root.insert(
        repo,
        repo.find_commit(base_id).unwrap(),
        &mut graph_branches,
    )?;

    root = root.extend(repo, graph_branches)?;

    Ok(root)
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
    protected_branches: &git_stack::git::Branches,
) -> eyre::Result<git_stack::git::Branch> {
    let branch = git_stack::git::find_protected_base(repo, protected_branches, head_oid)
        .ok_or_else(|| eyre::eyre!("could not find a protected branch to use as a base"))?;
    log::debug!("Chose branch {} as the base", branch.name);
    Ok(branch.clone())
}

fn git_pull(
    repo: &mut git_stack::git::GitRepo,
    remote: &str,
    branch_name: &str,
) -> eyre::Result<git2::Oid> {
    log::debug!("git pull --rebase {} {}", remote, branch_name);
    let remote_branch_name = format!("{}/{}", remote, branch_name);

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
