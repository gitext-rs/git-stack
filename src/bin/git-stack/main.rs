use std::io::Write;

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
    } else if args.show {
        show(&args, colored_stdout)?;
    } else {
        rewrite(&args)?;
    }

    Ok(())
}

fn dump_config(_args: &Args, output_path: &std::path::Path) -> proc_exit::ExitResult {
    log::trace!("Initializing");
    let cwd = std::env::current_dir().with_code(proc_exit::Code::USAGE_ERR)?;
    let repo = git2::Repository::discover(&cwd).with_code(proc_exit::Code::USAGE_ERR)?;

    let repo_config =
        git_stack::config::RepoConfig::from_all(&repo).with_code(proc_exit::Code::CONFIG_ERR)?;

    let output = toml::to_string_pretty(&repo_config).with_code(proc_exit::Code::FAILURE)?;

    if output_path == std::path::Path::new("-") {
        std::io::stdout().write_all(output.as_bytes())?;
    } else {
        std::fs::write(output_path, &output)?;
    }

    Ok(())
}

fn protect(_args: &Args, ignore: &str) -> proc_exit::ExitResult {
    log::trace!("Initializing");
    let cwd = std::env::current_dir().with_code(proc_exit::Code::USAGE_ERR)?;
    let repo = git2::Repository::discover(&cwd).with_code(proc_exit::Code::USAGE_ERR)?;

    let mut repo_config =
        git_stack::config::RepoConfig::from_repo(&repo).with_code(proc_exit::Code::CONFIG_ERR)?;
    repo_config
        .protected_branches
        .get_or_insert_with(Vec::new)
        .push(ignore.to_owned());

    repo_config
        .write_repo(&repo)
        .with_code(proc_exit::Code::FAILURE)?;

    Ok(())
}

fn show(args: &Args, colored_stdout: bool) -> proc_exit::ExitResult {
    log::trace!("Initializing");
    let cwd = std::env::current_dir().with_code(proc_exit::Code::USAGE_ERR)?;
    let repo = git2::Repository::discover(&cwd).with_code(proc_exit::Code::USAGE_ERR)?;

    let repo_config =
        git_stack::config::RepoConfig::from_all(&repo).with_code(proc_exit::Code::CONFIG_ERR)?;
    let mut protected_branches = ignore::gitignore::GitignoreBuilder::new("");
    for branch in repo_config.protected_branches.iter().flatten() {
        protected_branches
            .add_line(None, branch)
            .with_code(proc_exit::Code::CONFIG_ERR)?;
    }
    let protected_branches = protected_branches
        .build()
        .with_code(proc_exit::Code::CONFIG_ERR)?;

    let base_branch = args
        .base
        .as_deref()
        .map(|name| git_stack::git::resolve_branch(&repo, name))
        .transpose()
        .with_code(proc_exit::Code::USAGE_ERR)?;

    let head_branch =
        git_stack::git::resolve_head_branch(&repo).with_code(proc_exit::Code::USAGE_ERR)?;

    let root = git_stack::dag::graph(
        &repo,
        base_branch,
        head_branch,
        args.dependents,
        args.all,
        &protected_branches,
    )
    .with_code(proc_exit::Code::CONFIG_ERR)?;

    let palette = if colored_stdout {
        Palette::colored()
    } else {
        Palette::plain()
    };

    let mut tree = treeline::Tree::root(RenderNode {
        node: Some(&root),
        palette: &palette,
    });
    to_tree(root.children.as_slice(), &mut tree, &palette, args.show_all);
    writeln!(std::io::stdout(), "{}", tree)?;

    Ok(())
}

fn rewrite(args: &Args) -> proc_exit::ExitResult {
    if args.interactive {
        log::debug!("--interactive is not implemented yet");
    }
    if args.fix {
        log::debug!("--fix is not implemented yet");
    }
    if args.onto.is_some() {
        log::debug!("--onto is not implemented yet");
    }
    eyre::eyre!("Not implemented yet");

    Ok(())
}

fn to_tree<'r, 'n, 'p>(
    nodes: &'n [Vec<git_stack::dag::Node<'r>>],
    tree: &mut treeline::Tree<RenderNode<'r, 'n, 'p>>,
    palette: &'p Palette,
    show_all: bool,
) {
    for branch in nodes {
        let mut branch_root = treeline::Tree::root(RenderNode {
            node: None,
            palette,
        });
        for node in branch {
            if node.branches.is_empty() && node.children.is_empty() && !show_all {
                log::trace!("Skipping commit {}", node.local_commit.id());
                continue;
            }
            let mut child_tree = treeline::Tree::root(RenderNode {
                node: Some(node),
                palette,
            });
            to_tree(node.children.as_slice(), &mut child_tree, palette, show_all);
            branch_root.push(child_tree);
        }
        tree.push(branch_root);
    }
}

struct RenderNode<'r, 'n, 'p> {
    node: Option<&'n git_stack::dag::Node<'r>>,
    palette: &'p Palette,
}

impl<'r, 'n, 'p> std::fmt::Display for RenderNode<'r, 'n, 'p> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        if let Some(node) = self.node.as_ref() {
            if node.branches.is_empty() {
                write!(
                    f,
                    "{} {}",
                    self.palette.commit.paint(node.local_commit.id()),
                    self.palette
                        .summary
                        .paint(node.local_commit.summary().unwrap_or("<No summary>"))
                )?;
            } else {
                write!(
                    f,
                    "{} {}",
                    self.palette.branch.paint(
                        node.branches
                            .iter()
                            .map(|b| { b.name().ok().flatten().unwrap_or("<>") })
                            .join(", ")
                    ),
                    self.palette
                        .summary
                        .paint(node.local_commit.summary().unwrap_or("<No summary>"))
                )?;
            }
        } else {
            write!(f, "o")?;
        }
        Ok(())
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Palette {
    error: yansi::Style,
    warn: yansi::Style,
    info: yansi::Style,
    branch: yansi::Style,
    commit: yansi::Style,
    summary: yansi::Style,
}

impl Palette {
    pub fn colored() -> Self {
        Self {
            error: yansi::Style::new(yansi::Color::Red),
            warn: yansi::Style::new(yansi::Color::Yellow),
            info: yansi::Style::new(yansi::Color::Blue),
            branch: yansi::Style::new(yansi::Color::Green),
            commit: yansi::Style::new(yansi::Color::Blue),
            summary: yansi::Style::new(yansi::Color::Blue).dimmed(),
        }
    }

    pub fn plain() -> Self {
        Self {
            error: yansi::Style::default(),
            warn: yansi::Style::default(),
            info: yansi::Style::default(),
            branch: yansi::Style::default(),
            commit: yansi::Style::default(),
            summary: yansi::Style::default(),
        }
    }
}

#[derive(structopt::StructOpt)]
#[structopt(
        setting = structopt::clap::AppSettings::UnifiedHelpMessage,
        setting = structopt::clap::AppSettings::DeriveDisplayOrder,
        setting = structopt::clap::AppSettings::DontCollapseArgsInUsage
    )]
#[structopt(group = structopt::clap::ArgGroup::with_name("mode").multiple(false))]
struct Args {
    /// Show stack relationship
    #[structopt(short, long, group = "mode")]
    show: bool,

    /// Show all commits
    #[structopt(long)]
    show_all: bool,

    /// Write the current configuration to file with `-` for stdout
    #[structopt(long, group = "mode")]
    dump_config: Option<std::path::PathBuf>,

    /// Append a protected branch to the repository's config (gitignore syntax)
    #[structopt(long, group = "mode")]
    protect: Option<String>,

    /// Visually edit history in your $EDITOR`
    #[structopt(short, long)]
    interactive: bool,

    /// Apply all fixups
    #[structopt(long)]
    fix: bool,

    /// Include all dependent branches as well
    #[structopt(short, long)]
    dependents: bool,

    /// Include all branches
    #[structopt(short, long)]
    all: bool,

    /// Branch to evaluate from (default: last protected branch)
    #[structopt(long)]
    base: Option<String>,

    /// Branch to rebase onto (default: base)
    #[structopt(long)]
    onto: Option<String>,

    #[structopt(flatten)]
    color: git_stack::color::ColorArgs,

    #[structopt(flatten)]
    verbose: clap_verbosity_flag::Verbosity,
}
