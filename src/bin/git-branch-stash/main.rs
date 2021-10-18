#![allow(clippy::collapsible_else_if)]

use std::io::Write;

use proc_exit::WithCodeResultExt;
use structopt::StructOpt;

mod args;

fn main() {
    human_panic::setup_panic!();
    let result = run();
    proc_exit::exit(result);
}

fn run() -> proc_exit::ExitResult {
    // clap's `get_matches` uses Failure rather than Usage, so bypass it for `get_matches_safe`.
    let args = match args::Args::from_args_safe() {
        Ok(args) => args,
        Err(e) if e.use_stderr() => {
            return Err(proc_exit::Code::USAGE_ERR.with_message(e));
        }
        Err(e) => {
            writeln!(std::io::stdout(), "{}", e)?;
            return proc_exit::Code::SUCCESS.ok();
        }
    };

    args.color.apply();
    let colored_stdout = concolor_control::get(concolor_control::Stream::Stdout).ansi_color();
    let colored_stderr = concolor_control::get(concolor_control::Stream::Stderr).ansi_color();

    git_stack::log::init_logging(args.verbose.clone(), colored_stderr);

    let subcommand = args.subcommand;
    let push_args = args.push;
    match subcommand.unwrap_or(args::Subcommand::Push(push_args)) {
        args::Subcommand::Push(sub_args) => push(sub_args),
        args::Subcommand::List(sub_args) => list(sub_args, colored_stdout),
        args::Subcommand::Clear(sub_args) => clear(sub_args),
        args::Subcommand::Drop(sub_args) => drop(sub_args),
        args::Subcommand::Pop(sub_args) => apply(sub_args, true),
        args::Subcommand::Apply(sub_args) => apply(sub_args, false),
        args::Subcommand::Stacks(sub_args) => stacks(sub_args),
    }
}

fn push(args: args::PushArgs) -> proc_exit::ExitResult {
    let cwd = std::env::current_dir().with_code(proc_exit::Code::USAGE_ERR)?;
    let repo = git2::Repository::discover(&cwd).with_code(proc_exit::Code::USAGE_ERR)?;
    let repo = git_stack::git::GitRepo::new(repo);
    let mut stack = git_stack::stash::Stack::new(&args.stack, &repo);

    let repo_config = git_stack::config::RepoConfig::from_all(repo.raw())
        .with_code(proc_exit::Code::CONFIG_ERR)?;
    let protected = git_stack::git::ProtectedBranches::new(
        repo_config.protected_branches().iter().map(|s| s.as_str()),
    )
    .with_code(proc_exit::Code::USAGE_ERR)?;
    let branches = git_stack::git::Branches::new(repo.local_branches());
    let protected_branches = branches.protected(&protected);

    stack.capacity(repo_config.capacity());

    if repo.is_dirty() {
        log::warn!("Working tree is dirty, only capturing committed changes");
    }

    let mut snapshot =
        git_stack::stash::Snapshot::from_repo(&repo).with_code(proc_exit::Code::FAILURE)?;
    if let Some(message) = args.message.as_deref() {
        snapshot.insert_message(message);
    }
    snapshot.insert_parent(&repo, &branches, &protected_branches);
    stack.push(snapshot)?;

    Ok(())
}

fn list(args: args::ListArgs, colored: bool) -> proc_exit::ExitResult {
    let palette = if colored {
        Palette::colored()
    } else {
        Palette::plain()
    };

    let cwd = std::env::current_dir().with_code(proc_exit::Code::USAGE_ERR)?;
    let repo = git2::Repository::discover(&cwd).with_code(proc_exit::Code::USAGE_ERR)?;
    let repo = git_stack::git::GitRepo::new(repo);
    let stack = git_stack::stash::Stack::new(&args.stack, &repo);

    let snapshots: Vec<_> = stack.iter().collect();
    for (i, snapshot_path) in snapshots.iter().enumerate() {
        let style = if i < snapshots.len() - 1 {
            palette.info
        } else {
            palette.good
        };
        let snapshot = match git_stack::stash::Snapshot::load(snapshot_path) {
            Ok(snapshot) => snapshot,
            Err(err) => {
                log::error!(
                    "Failed to load snapshot {}: {}",
                    snapshot_path.display(),
                    err
                );
                continue;
            }
        };
        match snapshot.metadata.get("message") {
            Some(message) => {
                writeln!(
                    std::io::stdout(),
                    "{}",
                    style.paint(format_args!("Message: {}", message))
                )?;
            }
            None => {
                writeln!(
                    std::io::stdout(),
                    "{}",
                    style.paint(format_args!("Path: {}", snapshot_path.display()))
                )?;
            }
        }
        for branch in snapshot.branches.iter() {
            let summary = if let Some(summary) = branch.metadata.get("summary") {
                summary.to_string()
            } else {
                branch.id.to_string()
            };
            let name =
                if let Some(serde_json::Value::String(parent)) = branch.metadata.get("parent") {
                    format!("{}..{}", parent, branch.name)
                } else {
                    branch.name.clone()
                };
            writeln!(
                std::io::stdout(),
                "{}",
                style.paint(format_args!("- {}: {}", name, summary))
            )?;
        }
        writeln!(std::io::stdout())?;
    }

    Ok(())
}

#[derive(Copy, Clone, Debug)]
struct Palette {
    error: yansi::Style,
    warn: yansi::Style,
    info: yansi::Style,
    good: yansi::Style,
    hint: yansi::Style,
}

impl Palette {
    pub fn colored() -> Self {
        Self {
            error: yansi::Style::new(yansi::Color::Red),
            warn: yansi::Style::new(yansi::Color::Yellow),
            info: yansi::Style::new(yansi::Color::Blue),
            good: yansi::Style::new(yansi::Color::Green),
            hint: yansi::Style::new(yansi::Color::Blue).dimmed(),
        }
    }

    pub fn plain() -> Self {
        Self {
            error: yansi::Style::default(),
            warn: yansi::Style::default(),
            info: yansi::Style::default(),
            good: yansi::Style::default(),
            hint: yansi::Style::default(),
        }
    }
}

fn clear(args: args::ClearArgs) -> proc_exit::ExitResult {
    let cwd = std::env::current_dir().with_code(proc_exit::Code::USAGE_ERR)?;
    let repo = git2::Repository::discover(&cwd).with_code(proc_exit::Code::USAGE_ERR)?;
    let repo = git_stack::git::GitRepo::new(repo);
    let mut stack = git_stack::stash::Stack::new(&args.stack, &repo);

    stack.clear();

    Ok(())
}

fn drop(args: args::DropArgs) -> proc_exit::ExitResult {
    let cwd = std::env::current_dir().with_code(proc_exit::Code::USAGE_ERR)?;
    let repo = git2::Repository::discover(&cwd).with_code(proc_exit::Code::USAGE_ERR)?;
    let repo = git_stack::git::GitRepo::new(repo);
    let mut stack = git_stack::stash::Stack::new(&args.stack, &repo);

    stack.pop();

    Ok(())
}

fn apply(args: args::ApplyArgs, pop: bool) -> proc_exit::ExitResult {
    let cwd = std::env::current_dir().with_code(proc_exit::Code::USAGE_ERR)?;
    let repo = git2::Repository::discover(&cwd).with_code(proc_exit::Code::USAGE_ERR)?;
    let mut repo = git_stack::git::GitRepo::new(repo);
    let mut stack = git_stack::stash::Stack::new(&args.stack, &repo);

    if repo.is_dirty() {
        return Err(proc_exit::Code::USAGE_ERR.with_message("Working tree is dirty, aborting"));
    }

    match stack.peek() {
        Some(last) => {
            let snapshot =
                git_stack::stash::Snapshot::load(&last).with_code(proc_exit::Code::FAILURE)?;
            snapshot
                .apply(&mut repo)
                .with_code(proc_exit::Code::FAILURE)?;
            if pop {
                let _ = std::fs::remove_file(&last);
            }
        }
        None => {
            log::warn!("Nothing to apply");
        }
    }

    Ok(())
}

fn stacks(_args: args::StacksArgs) -> proc_exit::ExitResult {
    let cwd = std::env::current_dir().with_code(proc_exit::Code::USAGE_ERR)?;
    let repo = git2::Repository::discover(&cwd).with_code(proc_exit::Code::USAGE_ERR)?;
    let repo = git_stack::git::GitRepo::new(repo);

    for stack in git_stack::stash::Stack::all(&repo) {
        writeln!(std::io::stdout(), "{}", stack.name)?;
    }

    Ok(())
}
