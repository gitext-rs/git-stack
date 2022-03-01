#[derive(clap::Parser)]
#[clap(about, author, version)]
#[clap(
        setting = clap::AppSettings::DeriveDisplayOrder,
        dont_collapse_args_in_usage = true,
        color = concolor_clap::color_choice(),
    )]
#[clap(group = clap::ArgGroup::new("mode").multiple(false))]
pub struct Args {
    /// Rebase the selected stacks
    #[clap(short, long, group = "mode")]
    pub rebase: bool,

    /// Pull the parent branch and rebase onto it.
    #[clap(long)]
    pub pull: bool,

    /// Push all ready branches
    #[clap(long)]
    pub push: bool,

    /// Which branch stacks to include
    #[clap(short, long, arg_enum, ignore_case = true)]
    pub stack: Option<git_stack::config::Stack>,

    /// Branch to evaluate from (default: most-recent protected branch)
    #[clap(long)]
    pub base: Option<String>,

    /// Branch to rebase onto (default: base)
    #[clap(long)]
    pub onto: Option<String>,

    /// Action to perform with fixup-commits
    #[clap(long, arg_enum, ignore_case = true)]
    pub fixup: Option<git_stack::config::Fixup>,

    /// Repair diverging branches.
    #[clap(long, overrides_with("no-repair"))]
    repair: bool,
    #[clap(long, overrides_with("repair"), hide = true)]
    no_repair: bool,

    #[clap(short = 'n', long)]
    pub dry_run: bool,

    #[clap(long, arg_enum, ignore_case = true)]
    pub format: Option<git_stack::config::Format>,

    /// See what branches are protected
    #[clap(long, group = "mode")]
    pub protected: bool,

    /// Append a protected branch to the repository's config (gitignore syntax)
    #[clap(long, group = "mode")]
    pub protect: Option<String>,

    /// Write the current configuration to file with `-` for stdout
    #[clap(long, parse(from_os_str), group = "mode")]
    pub dump_config: Option<std::path::PathBuf>,

    #[clap(flatten)]
    pub(crate) color: concolor_clap::Color,

    #[clap(flatten)]
    pub verbose: clap_verbosity_flag::Verbosity<clap_verbosity_flag::InfoLevel>,
}

impl Args {
    pub fn to_config(&self) -> git_stack::config::RepoConfig {
        git_stack::config::RepoConfig {
            protected_branches: None,
            protect_commit_count: None,
            protect_commit_age: None,
            stack: self.stack,
            push_remote: None,
            pull_remote: None,
            show_format: self.format,
            show_stacked: None,
            auto_fixup: None,
            auto_repair: None,

            capacity: None,
        }
    }

    pub fn repair(&self) -> Option<bool> {
        resolve_bool_arg(self.repair, self.no_repair)
    }
}

fn resolve_bool_arg(yes: bool, no: bool) -> Option<bool> {
    match (yes, no) {
        (true, false) => Some(true),
        (false, true) => Some(false),
        (false, false) => None,
        (_, _) => unreachable!("clap should make this impossible"),
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn verify_app() {
        use clap::CommandFactory;
        Args::command().debug_assert()
    }
}
