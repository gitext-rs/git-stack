#![allow(clippy::collapsible_else_if)]
#![allow(clippy::let_and_return)]
#![allow(clippy::if_same_then_else)]

use clap::Parser;

mod args;
mod config;
mod stack;

fn main() {
    human_panic::setup_panic!();
    let result = run();
    proc_exit::exit(result);
}

fn run() -> proc_exit::ExitResult {
    // clap's `get_matches` uses Failure rather than Usage, so bypass it for `get_matches_safe`.
    let args = match args::Args::try_parse() {
        Ok(args) => args,
        Err(e) if e.use_stderr() => {
            let _ = e.print();
            return proc_exit::Code::USAGE_ERR.ok();
        }
        Err(e) => {
            let _ = e.print();
            return proc_exit::Code::SUCCESS.ok();
        }
    };

    args.color.apply();
    let colored_stdout = concolor::get(concolor::Stream::Stdout).ansi_color();
    let colored_stderr = concolor::get(concolor::Stream::Stderr).ansi_color();

    git_stack::log::init_logging(args.verbose.clone(), colored_stderr);

    if let Some(output_path) = args.dump_config.as_deref() {
        config::dump_config(&args, output_path)?;
    } else if let Some(ignore) = args.protect.as_deref() {
        config::protect(&args, ignore)?;
    } else if args.protected {
        config::protected(&args)?;
    } else {
        stack::stack(&args, colored_stdout, colored_stderr)?;
    }

    Ok(())
}
