use std::io::Write;

use eyre::WrapErr;
use itertools::Itertools;
use proc_exit::WithCodeResultExt;
use structopt::StructOpt;

fn main() {
    human_panic::setup_panic!();
    let result = run();
    proc_exit::exit(result);
}

fn run() -> proc_exit::ExitResult {
    // clap's `get_matches` uses Failure rather than Usage, so bypass it for `get_matches_safe`.
    let mut args = match Args::from_args_safe() {
        Ok(args) => args,
        Err(e) if e.use_stderr() => {
            return Err(proc_exit::Code::USAGE_ERR.with_message(e));
        }
        Err(e) => {
            writeln!(std::io::stdout(), "{}", e)?;
            return proc_exit::Code::SUCCESS.ok();
        }
    };
    if args.pull {
        log::trace!("`--pull` implies `--rebase`");
        args.rebase = true;
    }

    let colored = args.color.colored().or_else(git_stack::color::colored_env);
    let mut colored_stdout = colored
        .or_else(git_stack::color::colored_stdout)
        .unwrap_or(true);
    let mut colored_stderr = colored
        .or_else(git_stack::color::colored_stderr)
        .unwrap_or(true);
    if (colored_stdout || colored_stderr) && !yansi::Paint::enable_windows_ascii() {
        colored_stdout = false;
        colored_stderr = false;
    }

    git_stack::log::init_logging(args.verbose.clone(), colored_stderr);

    if let Some(output_path) = args.dump_config.as_deref() {
        dump_config(&args, output_path)?;
    } else if let Some(ignore) = args.protect.as_deref() {
        protect(&args, ignore)?;
    } else if args.protected {
        protected(&args)?;
    } else {
        stack(&args, colored_stdout)?;
    }

    Ok(())
}

fn dump_config(args: &Args, output_path: &std::path::Path) -> proc_exit::ExitResult {
    log::trace!("Initializing");
    let cwd = std::env::current_dir().with_code(proc_exit::Code::USAGE_ERR)?;
    let repo = git2::Repository::discover(&cwd).with_code(proc_exit::Code::USAGE_ERR)?;

    let repo_config = git_stack::config::RepoConfig::from_all(&repo)
        .with_code(proc_exit::Code::CONFIG_ERR)?
        .update(args.to_config());

    // TODO: Format dumped output as `.gitconfig`
    let output = toml::to_string_pretty(&repo_config).with_code(proc_exit::Code::FAILURE)?;

    if output_path == std::path::Path::new("-") {
        std::io::stdout().write_all(output.as_bytes())?;
    } else {
        std::fs::write(output_path, &output)?;
    }

    Ok(())
}

fn protect(args: &Args, ignore: &str) -> proc_exit::ExitResult {
    log::trace!("Initializing");
    let cwd = std::env::current_dir().with_code(proc_exit::Code::USAGE_ERR)?;
    let repo = git2::Repository::discover(&cwd).with_code(proc_exit::Code::USAGE_ERR)?;

    let mut repo_config = git_stack::config::RepoConfig::from_repo(&repo)
        .with_code(proc_exit::Code::CONFIG_ERR)?
        .update(args.to_config());
    repo_config
        .protected_branches
        .get_or_insert_with(Vec::new)
        .push(ignore.to_owned());

    repo_config
        .write_repo(&repo)
        .with_code(proc_exit::Code::FAILURE)?;

    Ok(())
}

fn protected(args: &Args) -> proc_exit::ExitResult {
    log::trace!("Initializing");
    let cwd = std::env::current_dir().with_code(proc_exit::Code::USAGE_ERR)?;
    let repo = git2::Repository::discover(&cwd).with_code(proc_exit::Code::USAGE_ERR)?;

    let repo_config = git_stack::config::RepoConfig::from_all(&repo)
        .with_code(proc_exit::Code::CONFIG_ERR)?
        .update(args.to_config());
    let protected = git_stack::git::ProtectedBranches::new(
        repo_config.protected_branches().iter().map(|s| s.as_str()),
    )
    .with_code(proc_exit::Code::CONFIG_ERR)?;

    let repo = git_stack::git::GitRepo::new(repo);
    let branches = git_stack::git::Branches::new(repo.local_branches());
    let protected_branches = branches.protected(&protected);

    for (branch_id, branches) in branches.iter() {
        if protected_branches.contains_oid(branch_id) {
            for branch in branches {
                writeln!(std::io::stdout(), "{}", branch.name)?;
            }
        } else {
            for branch in branches {
                log::debug!("Unprotected: {}", branch.name);
            }
        }
    }

    Ok(())
}

fn stack(args: &Args, colored_stdout: bool) -> proc_exit::ExitResult {
    log::trace!("Initializing");
    let cwd = std::env::current_dir().with_code(proc_exit::Code::USAGE_ERR)?;
    let repo = git2::Repository::discover(&cwd).with_code(proc_exit::Code::USAGE_ERR)?;

    let repo_config = git_stack::config::RepoConfig::from_all(&repo)
        .with_code(proc_exit::Code::CONFIG_ERR)?
        .update(args.to_config());
    let protected = git_stack::git::ProtectedBranches::new(
        repo_config.protected_branches().iter().map(|s| s.as_str()),
    )
    .with_code(proc_exit::Code::CONFIG_ERR)?;

    let mut repo = git_stack::git::GitRepo::new(repo);
    let mut branches = git_stack::git::Branches::new(repo.local_branches());
    let mut protected_branches = branches.protected(&protected);

    if args.rebase {
        let head_oid = repo.head_commit().id;
        let head_branch = if let Some(branches) = branches.get(head_oid) {
            branches[0].clone()
        } else {
            return Err(eyre::eyre!("Must not be in a detached HEAD state."))
                .with_code(proc_exit::Code::USAGE_ERR);
        };

        let script = plan_rebase(
            &mut repo,
            &repo_config,
            &mut branches,
            &mut protected_branches,
            &protected,
            args,
        )
        .with_code(proc_exit::Code::FAILURE)?;

        let mut executor = git_stack::git::Executor::new(&repo, args.dry_run);
        let results = executor.run_script(&mut repo, &script);
        for (err, name, dependents) in results.iter() {
            log::error!("Failed to re-stack branch `{}`: {}", name, err);
            if !dependents.is_empty() {
                log::error!("  Blocked dependents: {}", dependents.iter().join(", "));
            }
        }
        executor
            .close(&mut repo, &head_branch.name)
            .with_code(proc_exit::Code::FAILURE)?;
        if !results.is_empty() {
            return proc_exit::Code::FAILURE.ok();
        }
    }

    show(
        &repo,
        &repo_config,
        &branches,
        &protected_branches,
        args,
        colored_stdout,
    )
    .with_code(proc_exit::Code::FAILURE)?;

    Ok(())
}

fn plan_rebase(
    repo: &mut git_stack::git::GitRepo,
    repo_config: &git_stack::config::RepoConfig,
    branches: &mut git_stack::git::Branches,
    protected_branches: &mut git_stack::git::Branches,
    protected: &git_stack::git::ProtectedBranches,
    args: &Args,
) -> eyre::Result<git_stack::git::Script> {
    let head_oid = repo.head_commit().id;

    let base_branch = resolve_base(repo, args.base.as_deref(), head_oid, protected_branches)?;

    let onto_branch = if let Some(onto_name) = args.onto.as_deref() {
        if let Some(onto_branch) = repo.find_local_branch(onto_name) {
            itertools::Either::Left(onto_branch)
        } else if let Some(onto_commit) = repo.resolve(onto_name) {
            if let Some(onto_branches) = protected_branches.get(onto_commit.id) {
                let onto_branch = onto_branches.first().unwrap();
                itertools::Either::Left(onto_branch.clone())
            } else {
                itertools::Either::Right((onto_name, onto_commit.id))
            }
        } else {
            eyre::bail!("Could not resolve `{}` as a commit", onto_name);
        }
    } else {
        itertools::Either::Left(base_branch.clone())
    };

    let onto_oid = match onto_branch {
        itertools::Either::Left(onto_branch) => {
            if args.pull {
                if protected_branches.contains_oid(onto_branch.id) {
                    if let Err(err) =
                        git_pull(repo, repo_config.pull_remote(), onto_branch.name.as_str())
                    {
                        log::warn!("Skipping pull, {}", err);
                    } else {
                        *branches = git_stack::git::Branches::new(repo.local_branches());
                        *protected_branches = branches.protected(protected);
                    }
                } else {
                    log::warn!(
                        "Skipping pull, `{}` isn't a protected branch",
                        onto_branch.name
                    );
                }
            }
            onto_branch.id
        }
        itertools::Either::Right((name, oid)) => {
            if args.pull {
                log::warn!("Skipping pull, `{}` isn't a branch", name);
            }
            oid
        }
    };

    let merge_base_oid = repo.merge_base(base_branch.id, head_oid).ok_or_else(|| {
        git2::Error::new(
            git2::ErrorCode::NotFound,
            git2::ErrorClass::Reference,
            format!("could not find base between {} and HEAD", base_branch.name),
        )
    })?;
    let graphed_branches = match repo_config.branch() {
        git_stack::config::Branch::Current => branches.branch(repo, merge_base_oid, head_oid),
        git_stack::config::Branch::Dependents => {
            branches.dependents(repo, merge_base_oid, head_oid)
        }
        git_stack::config::Branch::Descendants => branches.descendants(repo, merge_base_oid),
        git_stack::config::Branch::All => branches.all(),
    };
    let mut root = graph(repo, merge_base_oid, head_oid, graphed_branches)?;

    git_stack::graph::protect_branches(&mut root, repo, &protected_branches)?;

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

fn show(
    repo: &git_stack::git::GitRepo,
    repo_config: &git_stack::config::RepoConfig,
    branches: &git_stack::git::Branches,
    protected_branches: &git_stack::git::Branches,
    args: &Args,
    colored_stdout: bool,
) -> eyre::Result<()> {
    let head_oid = repo.head_commit().id;
    let base_branch = resolve_base(repo, args.base.as_deref(), head_oid, protected_branches)?;
    let merge_base_oid = repo.merge_base(base_branch.id, head_oid).ok_or_else(|| {
        git2::Error::new(
            git2::ErrorCode::NotFound,
            git2::ErrorClass::Reference,
            format!("could not find base between {} and HEAD", base_branch.name),
        )
    })?;
    let mut graphed_branches = match repo_config.branch() {
        git_stack::config::Branch::Current => branches.branch(repo, merge_base_oid, head_oid),
        git_stack::config::Branch::Dependents => {
            branches.dependents(repo, merge_base_oid, head_oid)
        }
        git_stack::config::Branch::Descendants => branches.descendants(repo, merge_base_oid),
        git_stack::config::Branch::All => branches.all(),
    };
    if !graphed_branches.contains_oid(base_branch.id) {
        graphed_branches.insert(base_branch.clone());
    }
    let mut root = graph(repo, merge_base_oid, head_oid, graphed_branches)?;
    git_stack::graph::protect_branches(&mut root, repo, protected_branches)?;
    // TODO: Show unblocked branches
    if !repo_config.show_stacked() {
        git_stack::graph::delinearize(&mut root);
    }

    match repo_config.show_format() {
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

fn resolve_base(
    repo: &dyn git_stack::git::Repo,
    base: Option<&str>,
    head_oid: git2::Oid,
    protected_branches: &git_stack::git::Branches,
) -> eyre::Result<git_stack::git::Branch> {
    match base {
        Some(branch_name) => resolve_explicit_base(repo, branch_name),
        None => resolve_implicit_base(repo, head_oid, protected_branches),
    }
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
) -> eyre::Result<()> {
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
            return Ok(());
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

    Ok(())
}

#[derive(structopt::StructOpt)]
#[structopt(
        setting = structopt::clap::AppSettings::UnifiedHelpMessage,
        setting = structopt::clap::AppSettings::DeriveDisplayOrder,
        setting = structopt::clap::AppSettings::DontCollapseArgsInUsage
    )]
#[structopt(group = structopt::clap::ArgGroup::with_name("mode").multiple(false))]
struct Args {
    /// Rebase the selected branch
    #[structopt(short, long, group = "mode")]
    rebase: bool,

    /// Visually edit history in your $EDITOR`
    #[structopt(short, long)]
    // TODO: --interactive support
    _interactive: bool,

    /// Apply all fixups
    #[structopt(long)]
    // TODO: --fix support
    _fix: bool,

    /// Which branches to include
    #[structopt(
        short,
        long,
        possible_values(&git_stack::config::Branch::variants()),
        case_insensitive(true),
    )]
    branch: Option<git_stack::config::Branch>,

    /// Branch to evaluate from (default: last protected branch)
    #[structopt(long)]
    base: Option<String>,

    /// Pull the parent branch and rebase onto it.
    #[structopt(long)]
    // TODO: Add push unblocked branch support (no WIP, directly on protected)
    pull: bool,

    /// Branch to rebase onto (default: base)
    #[structopt(long)]
    onto: Option<String>,

    #[structopt(short = "n", long)]
    dry_run: bool,

    #[structopt(
        long,
        possible_values(&git_stack::config::Format::variants()),
        case_insensitive(true),
    )]
    format: Option<git_stack::config::Format>,

    /// See what branches are protected
    #[structopt(long, group = "mode")]
    protected: bool,

    /// Append a protected branch to the repository's config (gitignore syntax)
    #[structopt(long, group = "mode")]
    protect: Option<String>,

    /// Write the current configuration to file with `-` for stdout
    #[structopt(long, group = "mode")]
    dump_config: Option<std::path::PathBuf>,

    #[structopt(flatten)]
    color: git_stack::color::ColorArgs,

    #[structopt(flatten)]
    verbose: clap_verbosity_flag::Verbosity,
}

impl Args {
    fn to_config(&self) -> git_stack::config::RepoConfig {
        git_stack::config::RepoConfig {
            protected_branches: None,
            branch: self.branch,
            pull_remote: None,
            show_format: self.format,
            show_stacked: None,
        }
    }
}
