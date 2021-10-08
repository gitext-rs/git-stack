use std::io::Write;

use proc_exit::WithCodeResultExt;
use structopt::StructOpt;

#[derive(StructOpt)]
#[structopt(group = structopt::clap::ArgGroup::with_name("mode").multiple(false))]
struct Args {
    #[structopt(short, long, group = "mode")]
    input: Option<std::path::PathBuf>,
    #[structopt(short, long)]
    output: Option<std::path::PathBuf>,
    /// Sleep between commits
    #[structopt(long, parse(try_from_str))]
    sleep: Option<humantime::Duration>,

    #[structopt(short, long, group = "mode")]
    schema: Option<std::path::PathBuf>,
}

fn main() {
    let result = run();
    proc_exit::exit(result);
}

fn run() -> proc_exit::ExitResult {
    let args = Args::from_args();
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
