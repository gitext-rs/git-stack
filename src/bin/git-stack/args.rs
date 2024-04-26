#[derive(clap::Parser)]
#[command(about, author, version)]
#[command(group = clap::ArgGroup::new("mode").multiple(false))]
#[command(args_conflicts_with_subcommands = true)]
pub(crate) struct Args {
    /// Rebase the selected stacks
    #[arg(short, long, group = "mode")]
    pub(crate) rebase: bool,

    /// Pull the parent branch and rebase onto it.
    #[arg(long)]
    pub(crate) pull: bool,

    /// Push all ready branches
    #[arg(long)]
    pub(crate) push: bool,

    /// Which branch stacks to include
    #[arg(short, long, value_enum)]
    pub(crate) stack: Option<git_stack::config::Stack>,

    /// Branch to evaluate from (default: most-recent protected branch)
    #[arg(long)]
    pub(crate) base: Option<String>,

    /// Branch to rebase onto (default: base)
    #[arg(long)]
    pub(crate) onto: Option<String>,

    /// Action to perform with fixup-commits
    #[arg(long, value_enum)]
    pub(crate) fixup: Option<git_stack::config::Fixup>,

    /// Repair diverging branches.
    #[arg(long, overrides_with("no_repair"))]
    repair: bool,
    #[arg(long, overrides_with("repair"), hide = true)]
    no_repair: bool,

    #[arg(short = 'n', long)]
    pub(crate) dry_run: bool,

    #[arg(long, value_enum)]
    pub(crate) format: Option<git_stack::config::Format>,

    #[arg(long, value_enum)]
    pub(crate) show_commits: Option<git_stack::config::ShowCommits>,

    /// See what branches are protected
    #[arg(long, group = "mode")]
    pub(crate) protected: bool,

    /// Append a protected branch to the repository's config (gitignore syntax)
    #[arg(long, group = "mode")]
    pub(crate) protect: Option<String>,

    /// Run as if git was started in `PATH` instead of the current working directory.
    ///
    /// When multiple -C options are given, each subsequent
    /// non-absolute -C <path> is interpreted relative to the preceding -C <path>. If <path> is present but empty, e.g.  -C "", then the
    /// current working directory is left unchanged.
    ///
    /// This option affects options that expect path name like --git-dir and --work-tree in that their interpretations of the path names
    /// would be made relative to the working directory caused by the -C option. For example the following invocations are equivalent:
    ///
    ///     git --git-dir=a.git --work-tree=b -C c status
    ///     git --git-dir=c/a.git --work-tree=c/b status
    #[arg(short = 'C', hide = true, value_name = "PATH")]
    pub(crate) current_dir: Option<Vec<std::path::PathBuf>>,

    /// Write the current configuration to file with `-` for stdout
    #[arg(long, group = "mode")]
    pub(crate) dump_config: Option<std::path::PathBuf>,

    #[command(flatten)]
    pub(crate) color: colorchoice_clap::Color,

    #[command(flatten)]
    pub(crate) verbose: clap_verbosity_flag::Verbosity<clap_verbosity_flag::InfoLevel>,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(clap::Subcommand)]
pub(crate) enum Command {
    #[command(alias = "prev")]
    Previous(crate::prev::PrevArgs),
    Next(crate::next::NextArgs),
    Reword(crate::reword::RewordArgs),
    Amend(crate::amend::AmendArgs),
    Sync(crate::sync::SyncArgs),
    Run(crate::run::RunArgs),
    Alias(crate::alias::AliasArgs),
}

impl Args {
    pub(crate) fn exec(&self) -> proc_exit::ExitResult {
        match &self.command {
            Some(Command::Previous(c)) => c.exec(),
            Some(Command::Next(c)) => c.exec(),
            Some(Command::Reword(c)) => c.exec(),
            Some(Command::Amend(c)) => c.exec(),
            Some(Command::Sync(c)) => c.exec(),
            Some(Command::Run(c)) => c.exec(),
            Some(Command::Alias(c)) => c.exec(),
            None => {
                if let Some(output_path) = self.dump_config.as_deref() {
                    crate::config::dump_config(self, output_path)
                } else if let Some(ignore) = self.protect.as_deref() {
                    crate::config::protect(self, ignore)
                } else if self.protected {
                    crate::config::protected(self)
                } else {
                    crate::stack::stack(self)
                }
            }
        }
    }

    pub(crate) fn to_config(&self) -> git_stack::config::RepoConfig {
        git_stack::config::RepoConfig {
            editor: None,
            protected_branches: None,
            protect_commit_count: None,
            protect_commit_age: None,
            auto_base_commit_count: None,
            stack: self.stack,
            push_remote: None,
            pull_remote: None,
            show_format: self.format,
            show_commits: self.show_commits,
            show_stacked: None,
            auto_fixup: None,
            auto_repair: None,

            capacity: None,
        }
    }

    pub(crate) fn repair(&self) -> Option<bool> {
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
        Args::command().debug_assert();
    }
}
