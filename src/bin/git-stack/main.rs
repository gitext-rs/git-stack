#![allow(clippy::single_match_else)] // avoid large clean up

use clap::Parser;
use proc_exit::WithCodeResultExt;

mod alias;
mod amend;
mod args;
mod config;
mod logger;
mod next;
mod ops;
mod prev;
mod reword;
mod run;
mod stack;
mod sync;

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
            return proc_exit::sysexits::USAGE_ERR.ok();
        }
        Err(e) => {
            let _ = e.print();
            return proc_exit::Code::SUCCESS.ok();
        }
    };

    args.color.write_global();
    let colored_stderr = !matches!(
        anstream::AutoStream::choice(&std::io::stderr()),
        anstream::ColorChoice::Never
    );

    logger::init_logging(args.verbose.clone(), colored_stderr);

    if let Some(current_dir) = args.current_dir.as_deref() {
        let current_dir = current_dir
            .iter()
            .fold(std::path::PathBuf::new(), |current, next| {
                current.join(next)
            });
        log::trace!("CWD={}", current_dir.display());
        std::env::set_current_dir(current_dir).with_code(proc_exit::sysexits::USAGE_ERR)?;
    }

    args.exec()
}
