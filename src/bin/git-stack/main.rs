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

    if let Some(output_path) = args.dump_config.as_deref() {
        dump_config(&args, output_path)?;
    } else if args.show {
        show(&args)?;
    }

    Ok(())
}

fn dump_config(args: &Args, output_path: &std::path::Path) -> proc_exit::ExitResult {
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

    git_stack::log::init_logging(args.verbose.log_level(), colored_stderr);

    log::debug!("Initializing");
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

fn show(args: &Args) -> proc_exit::ExitResult {
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

    git_stack::log::init_logging(args.verbose.log_level(), colored_stderr);

    log::debug!("Initializing");
    let cwd = std::env::current_dir().with_code(proc_exit::Code::USAGE_ERR)?;
    let repo = git2::Repository::discover(&cwd).with_code(proc_exit::Code::USAGE_ERR)?;
    let git_config = git2::Config::open_default().with_code(proc_exit::Code::CONFIG_ERR)?;

    let base_name = args
        .base
        .as_deref()
        .unwrap_or_else(|| git_stack::git::default_branch(&git_config));
    let base_branch =
        git_stack::git::resolve_branch(&repo, base_name).with_code(proc_exit::Code::USAGE_ERR)?;

    let head_branch =
        git_stack::git::resolve_head_branch(&repo).with_code(proc_exit::Code::USAGE_ERR)?;

    let root = git_stack::dag::graph(&repo, base_branch, head_branch, args.dependents)
        .with_code(proc_exit::Code::CONFIG_ERR)?;

    let mut tree = treeline::Tree::root(RenderNode { node: Some(&root) });
    to_tree(root.children.as_slice(), &mut tree, colored_stdout);
    writeln!(std::io::stdout(), "{}", tree)?;

    Ok(())
}

fn to_tree<'r, 'n>(
    nodes: &'n [Vec<git_stack::dag::Node<'r>>],
    tree: &mut treeline::Tree<RenderNode<'r, 'n>>,
    colored: bool,
) {
    for branch in nodes {
        let mut branch_root = treeline::Tree::root(RenderNode { node: None });
        for node in branch {
            if node.branches.is_empty() && node.children.is_empty() {
                log::debug!("Skipping commit {}", node.local_commit.id());
                continue;
            }
            let mut child_tree = treeline::Tree::root(RenderNode { node: Some(node) });
            to_tree(node.children.as_slice(), &mut child_tree, colored);
            branch_root.push(child_tree);
        }
        tree.push(branch_root);
    }
}

struct RenderNode<'r, 'n> {
    node: Option<&'n git_stack::dag::Node<'r>>,
}

impl<'r, 'n> std::fmt::Display for RenderNode<'r, 'n> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        if let Some(node) = self.node.as_ref() {
            if node.branches.is_empty() {
                write!(f, "{}", node.local_commit.id())?;
            } else {
                write!(
                    f,
                    "{}",
                    node.branches
                        .iter()
                        .map(|b| { b.name().ok().flatten().unwrap_or("<>") })
                        .join(", ")
                )?;
            }
        } else {
            write!(f, "o")?;
        }
        Ok(())
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

    /// Write the current configuration to file with `-` for stdout
    #[structopt(long, group = "mode")]
    dump_config: Option<std::path::PathBuf>,

    /// Visually edit history in your $EDITOR`
    #[structopt(short, long)]
    interactive: bool,

    /// Apply all fixups
    #[structopt(long)]
    fix: bool,

    /// Include all dependent branches as well
    #[structopt(short, long)]
    dependents: bool,

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
