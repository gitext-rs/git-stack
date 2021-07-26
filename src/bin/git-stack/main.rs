use std::io::Write;

use proc_exit::WithCodeResultExt;
use structopt::StructOpt;

fn main() {
    human_panic::setup_panic!();
    let result = run();
    proc_exit::exit(result);
}

fn run() -> proc_exit::ExitResult {
    // clap's `get_matches` uses Failure rather than Usage, so bypass it for `get_matches_safe`.
    let args = match Args::from_args_safe() {
        Ok(args) => args,
        Err(e) if e.use_stderr() => {
            return Err(proc_exit::Code::USAGE_ERR.with_message(e));
        }
        Err(e) => {
            writeln!(std::io::stdout(), "{}", e)?;
            return proc_exit::Code::SUCCESS.ok();
        }
    };

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

fn stack(args: &Args, colored_stdout: bool) -> proc_exit::ExitResult {
    log::trace!("Initializing");
    let cwd = std::env::current_dir().with_code(proc_exit::Code::USAGE_ERR)?;
    let repo = git2::Repository::discover(&cwd).with_code(proc_exit::Code::USAGE_ERR)?;

    let repo_config = git_stack::config::RepoConfig::from_all(&repo)
        .with_code(proc_exit::Code::CONFIG_ERR)?
        .update(args.to_config());
    let protected = git_stack::protect::ProtectedBranches::new(
        repo_config
            .protected_branches
            .iter()
            .flatten()
            .map(|s| s.as_str()),
    )
    .with_code(proc_exit::Code::CONFIG_ERR)?;

    let repo = git_stack::repo::GitRepo::new(repo);

    let branches = git_stack::branches::Branches::new(repo.local_branches());

    let protected_branches = branches.protected(&protected);

    let head_commit = repo.head_commit();
    let head_oid = head_commit.id;
    let _head_branch = if let Some(branch) = branches.get(head_oid) {
        IntoIterator::into_iter(branch).next().unwrap()
    } else {
        return Err(eyre::eyre!("Must not be in a detached HEAD state."))
            .with_code(proc_exit::Code::USAGE_ERR);
    };

    let base_branch = match args.base.as_deref() {
        Some(branch_name) => repo
            .find_local_branch(branch_name)
            .ok_or_else(|| {
                git2::Error::new(
                    git2::ErrorCode::NotFound,
                    git2::ErrorClass::Reference,
                    format!("could not find branch {:?}", branch_name),
                )
            })
            .with_code(proc_exit::Code::USAGE_ERR)?,
        None => {
            let branch =
                git_stack::branches::find_protected_base(&repo, &protected_branches, head_oid)
                    .ok_or_else(|| {
                        git2::Error::new(
                            git2::ErrorCode::NotFound,
                            git2::ErrorClass::Reference,
                            "could not find a protected branch to use as a base",
                        )
                    })
                    .with_code(proc_exit::Code::USAGE_ERR)?;
            log::debug!("Chose branch {} as the base", branch.name);
            branch.clone()
        }
    };

    let base_oid = base_branch.id;
    let merge_base_oid = repo
        .merge_base(base_oid, head_oid)
        .ok_or_else(|| {
            git2::Error::new(
                git2::ErrorCode::NotFound,
                git2::ErrorClass::Reference,
                format!("could not find base between {} and HEAD", base_branch.name),
            )
        })
        .with_code(proc_exit::Code::USAGE_ERR)?;

    let graphed_branches = match repo_config.branch.expect("resolved") {
        git_stack::config::Branch::Current => branches.branch(&repo, merge_base_oid, head_oid),
        git_stack::config::Branch::Dependents => {
            branches.dependents(&repo, merge_base_oid, head_oid)
        }
        git_stack::config::Branch::All => {
            // TODO: we need to be sure to rebase against each branch's protected base, if we are
            // auto-detecting one.
            branches.all()
        }
    };

    let mut root = git_stack::dag::graph(
        &repo,
        merge_base_oid,
        head_oid,
        &protected_branches,
        graphed_branches,
    )
    .with_code(proc_exit::Code::CONFIG_ERR)?;
    git_stack::dag::protect_branches(&mut root, &repo, &protected_branches)
        .with_code(proc_exit::Code::CONFIG_ERR)?;
    if !repo_config.show_stacked.expect("resolved") {
        git_stack::dag::delinearize(&mut root);
    }

    let root = if args.show { root } else { root };

    match repo_config.show_format.expect("resolved") {
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

#[derive(structopt::StructOpt)]
#[structopt(
        setting = structopt::clap::AppSettings::UnifiedHelpMessage,
        setting = structopt::clap::AppSettings::DeriveDisplayOrder,
        setting = structopt::clap::AppSettings::DontCollapseArgsInUsage
    )]
#[structopt(group = structopt::clap::ArgGroup::with_name("mode").multiple(false))]
struct Args {
    /// Visually edit history in your $EDITOR`
    #[structopt(short, long)]
    _interactive: bool,

    /// Apply all fixups
    #[structopt(long)]
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

    /// Branch to rebase onto (default: base)
    #[structopt(long)]
    _onto: Option<String>,

    /// Only show stack relationship
    #[structopt(short, long)]
    show: bool,

    #[structopt(
        long,
        possible_values(&git_stack::config::Format::variants()),
        case_insensitive(true),
    )]
    format: Option<git_stack::config::Format>,

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
            show_format: self.format,
            branch: self.branch,
            show_stacked: None,
        }
    }
}
