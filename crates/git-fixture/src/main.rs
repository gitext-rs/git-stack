use std::io::Write;

use clap::Parser;
use proc_exit::WithCodeResultExt;

#[derive(Parser)]
#[clap(about, author, version)]
#[clap(group = clap::ArgGroup::new("mode").multiple(false))]
struct Args {
    #[clap(short, long, parse(from_os_str), group = "mode")]
    input: Option<std::path::PathBuf>,
    #[clap(short, long, parse(from_os_str))]
    output: Option<std::path::PathBuf>,
    /// Sleep between commits
    #[clap(long, parse(try_from_str))]
    sleep: Option<humantime::Duration>,

    #[clap(short, long, parse(from_os_str), group = "mode")]
    schema: Option<std::path::PathBuf>,
}

fn main() {
    let result = run();
    proc_exit::exit(result);
}

fn run() -> proc_exit::ExitResult {
    let args = Args::parse();
    let output = args
        .output
        .clone()
        .unwrap_or_else(|| std::env::current_dir().unwrap());

    if let Some(input) = args.input.as_deref() {
        std::fs::create_dir_all(&output)?;
        let mut dag = git_fixture::Dag::load(input).with_code(proc_exit::Code::CONFIG_ERR)?;
        dag.sleep = dag.sleep.or_else(|| args.sleep.map(|s| s.into()));
        dag.run(&output).with_code(proc_exit::Code::FAILURE)?;
    } else if let Some(schema_path) = args.schema.as_deref() {
        let schema = schemars::schema_for!(git_fixture::Dag);
        let schema = serde_json::to_string_pretty(&schema).unwrap();
        if schema_path == std::path::Path::new("-") {
            std::io::stdout().write_all(schema.as_bytes())?;
        } else {
            std::fs::write(&schema_path, &schema).with_code(proc_exit::Code::FAILURE)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn verify_app() {
        use clap::IntoApp;
        Args::into_app().debug_assert()
    }
}
