#[derive(structopt::StructOpt)]
#[structopt(
        setting = structopt::clap::AppSettings::UnifiedHelpMessage,
        setting = structopt::clap::AppSettings::DeriveDisplayOrder,
        setting = structopt::clap::AppSettings::DontCollapseArgsInUsage,
        setting = concolor_clap::color_choice(),
    )]
pub struct Args {
    #[structopt(subcommand)]
    pub subcommand: Option<Subcommand>,

    #[structopt(flatten)]
    pub push: PushArgs,

    #[structopt(flatten)]
    pub(crate) color: concolor_clap::Color,

    #[structopt(flatten)]
    pub verbose: clap_verbosity_flag::Verbosity,
}

#[derive(structopt::StructOpt)]
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
    Pop(PopArgs),
    /// Apply the last snapshot
    Apply(ApplyArgs),
    /// List all snapshot stacks
    Stacks(StacksArgs),
}

#[derive(structopt::StructOpt)]
pub struct PushArgs {
    /// Specify which stash stack to use
    #[structopt(default_value = git_stack::stash::Stack::DEFAULT_STACK)]
    pub stack: String,

    /// Annotate the snapshot with the given message
    #[structopt(short, long)]
    pub message: Option<String>,
}

#[derive(structopt::StructOpt)]
pub struct ListArgs {
    /// Specify which stash stack to use
    #[structopt(default_value = git_stack::stash::Stack::DEFAULT_STACK)]
    pub stack: String,
}

#[derive(structopt::StructOpt)]
pub struct ClearArgs {
    /// Specify which stash stack to use
    #[structopt(default_value = git_stack::stash::Stack::DEFAULT_STACK)]
    pub stack: String,
}

#[derive(structopt::StructOpt)]
pub struct DropArgs {
    /// Specify which stash stack to use
    #[structopt(default_value = git_stack::stash::Stack::DEFAULT_STACK)]
    pub stack: String,
}

#[derive(structopt::StructOpt)]
pub struct PopArgs {
    /// Specify which stash stack to use
    #[structopt(default_value = git_stack::stash::Stack::DEFAULT_STACK)]
    pub stack: String,
}

#[derive(structopt::StructOpt)]
pub struct ApplyArgs {
    /// Specify which stash stack to use
    #[structopt(default_value = git_stack::stash::Stack::DEFAULT_STACK)]
    pub stack: String,
}

#[derive(structopt::StructOpt)]
pub struct StacksArgs {}
