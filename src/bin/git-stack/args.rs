#[derive(structopt::StructOpt)]
#[structopt(
        setting = structopt::clap::AppSettings::UnifiedHelpMessage,
        setting = structopt::clap::AppSettings::DeriveDisplayOrder,
        setting = structopt::clap::AppSettings::DontCollapseArgsInUsage
    )]
#[structopt(group = structopt::clap::ArgGroup::with_name("mode").multiple(false))]
pub struct Args {
    /// Rebase the selected stacks
    #[structopt(short, long, group = "mode")]
    pub rebase: bool,

    /// Pull the parent branch and rebase onto it.
    #[structopt(long)]
    pub pull: bool,

    /// Push all ready branches
    #[structopt(long)]
    pub push: bool,

    /// Which branch stacks to include
    #[structopt(
        short,
        long,
        possible_values(&git_stack::config::Stack::variants()),
        case_insensitive(true),
    )]
    pub stack: Option<git_stack::config::Stack>,

    /// Branch to evaluate from (default: most-recent protected branch)
    #[structopt(long)]
    pub base: Option<String>,

    /// Branch to rebase onto (default: base)
    #[structopt(long)]
    pub onto: Option<String>,

    #[structopt(short = "n", long)]
    pub dry_run: bool,

    #[structopt(
        long,
        possible_values(&git_stack::config::Format::variants()),
        case_insensitive(true),
    )]
    pub format: Option<git_stack::config::Format>,

    /// See what branches are protected
    #[structopt(long, group = "mode")]
    pub protected: bool,

    /// Append a protected branch to the repository's config (gitignore syntax)
    #[structopt(long, group = "mode")]
    pub protect: Option<String>,

    /// Write the current configuration to file with `-` for stdout
    #[structopt(long, group = "mode")]
    pub dump_config: Option<std::path::PathBuf>,

    #[structopt(flatten)]
    pub color: git_stack::color::ColorArgs,

    #[structopt(flatten)]
    pub verbose: clap_verbosity_flag::Verbosity,
}

impl Args {
    pub fn to_config(&self) -> git_stack::config::RepoConfig {
        git_stack::config::RepoConfig {
            protected_branches: None,
            stack: self.stack,
            push_remote: None,
            pull_remote: None,
            show_format: self.format,
            show_stacked: None,

            capacity: None,
        }
    }
}
