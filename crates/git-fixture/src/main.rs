use proc_exit::WithCodeResultExt;
use structopt::StructOpt;

#[derive(StructOpt)]
struct Args {
    #[structopt(short, long)]
    cwd: std::path::PathBuf,

    input: std::path::PathBuf,
}

fn main() {
    let result = run();
    proc_exit::exit(result);
}

fn run() -> proc_exit::ExitResult {
    let args = Args::from_args();
    std::fs::create_dir_all(&args.cwd)?;
    let dag = git_fixture::Dag::load(&args.input).with_code(proc_exit::Code::CONFIG_ERR)?;
    dag.run(&args.cwd).with_code(proc_exit::Code::FAILURE)?;
    Ok(())
}
