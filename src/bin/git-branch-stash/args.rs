#[derive(clap::Parser)]
#[clap(about, author, version)]
#[clap(
        setting = clap::AppSettings::DeriveDisplayOrder,
        dont_collapse_args_in_usage = true,
        color = concolor_clap::color_choice(),
    )]
pub struct Args {
    #[clap(subcommand)]
    pub subcommand: Option<Subcommand>,

    #[clap(flatten)]
    pub push: PushArgs,

    #[clap(flatten)]
    pub(crate) color: concolor_clap::Color,

    #[clap(flatten)]
    pub verbose: clap_verbosity_flag::Verbosity<clap_verbosity_flag::InfoLevel>,
}

#[derive(clap::Subcommand)]
pub enum Subcommand {
    /// Stash all branches
    Push(PushArgs),
    /// List all stashed snapshots
    List(ListArgs),
    /// Clear all snapshots
    Clear(ClearArgs),
    /// Delete the last snapshot
    Drop(DropArgs),
    /// Apply the last snapshot, deleting it
    Pop(ApplyArgs),
    /// Apply the last snapshot
    Apply(ApplyArgs),
    /// List all snapshot stacks
    Stacks(StacksArgs),
}

#[derive(clap::Args)]
pub struct PushArgs {
    /// Specify which stash stack to use
    #[clap(default_value = git_stack::stash::Stack::DEFAULT_STACK)]
    pub stack: String,

    /// Annotate the snapshot with the given message
    #[clap(short, long)]
    pub message: Option<String>,
}

#[derive(clap::Args)]
pub struct ListArgs {
    /// Specify which stash stack to use
    #[clap(default_value = git_stack::stash::Stack::DEFAULT_STACK)]
    pub stack: String,
}

#[derive(clap::Args)]
pub struct ClearArgs {
    /// Specify which stash stack to use
    #[clap(default_value = git_stack::stash::Stack::DEFAULT_STACK)]
    pub stack: String,
}

#[derive(clap::Args)]
pub struct DropArgs {
    /// Specify which stash stack to use
    #[clap(default_value = git_stack::stash::Stack::DEFAULT_STACK)]
    pub stack: String,
}

#[derive(clap::Args)]
pub struct ApplyArgs {
    /// Specify which stash stack to use
    #[clap(default_value = git_stack::stash::Stack::DEFAULT_STACK)]
    pub stack: String,
}

#[derive(clap::Args)]
pub struct StacksArgs {}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn verify_app() {
        use clap::CommandFactory;
        Args::command().debug_assert()
    }
}
