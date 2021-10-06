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
    /// Backup all branches
    Push(PushArgs),
    /// List all backups
    List(ListArgs),
    /// Clear all backups
    Clear(ClearArgs),
    /// Delete the last backup
    Drop(DropArgs),
    /// Apply the last backup, deleting it
    Pop(PopArgs),
    /// Apply the last backup
    Apply(ApplyArgs),
    /// List all backup stacks
    Stacks(StacksArgs),
}

#[derive(structopt::StructOpt)]
pub struct PushArgs {
    /// Specify which backup stack to use
    #[structopt(default_value = git_stack::backup::Stack::DEFAULT_STACK)]
    pub stack: String,

    /// Annotate the backup with the given message
    #[structopt(short, long)]
    pub message: Option<String>,
}

#[derive(structopt::StructOpt)]
pub struct ListArgs {
    /// Specify which backup stack to use
    #[structopt(default_value = git_stack::backup::Stack::DEFAULT_STACK)]
    pub stack: String,
}

#[derive(structopt::StructOpt)]
pub struct ClearArgs {
    /// Specify which backup stack to use
    #[structopt(default_value = git_stack::backup::Stack::DEFAULT_STACK)]
    pub stack: String,
}

#[derive(structopt::StructOpt)]
pub struct DropArgs {
    /// Specify which backup stack to use
    #[structopt(default_value = git_stack::backup::Stack::DEFAULT_STACK)]
    pub stack: String,
}

#[derive(structopt::StructOpt)]
pub struct PopArgs {
    /// Specify which backup stack to use
    #[structopt(default_value = git_stack::backup::Stack::DEFAULT_STACK)]
    pub stack: String,
}

#[derive(structopt::StructOpt)]
pub struct ApplyArgs {
    /// Specify which backup stack to use
    #[structopt(default_value = git_stack::backup::Stack::DEFAULT_STACK)]
    pub stack: String,
}

#[derive(structopt::StructOpt)]
pub struct StacksArgs {}
