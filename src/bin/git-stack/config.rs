use std::io::Write;

use proc_exit::prelude::*;

pub fn dump_config(
    args: &crate::args::Args,
    output_path: &std::path::Path,
) -> proc_exit::ExitResult {
    log::trace!("Initializing");
    let cwd = std::env::current_dir().with_code(proc_exit::sysexits::USAGE_ERR)?;
    let repo = git2::Repository::discover(&cwd).with_code(proc_exit::sysexits::USAGE_ERR)?;

    let repo_config = git_stack::legacy::config::RepoConfig::from_all(&repo)
        .with_code(proc_exit::sysexits::CONFIG_ERR)?
        .update(args.to_config());

    let output = repo_config.to_string();

    if output_path == std::path::Path::new("-") {
        std::io::stdout()
            .write_all(output.as_bytes())
            .to_sysexits()?;
    } else {
        std::fs::write(output_path, &output).to_sysexits()?;
    }

    Ok(())
}

pub fn protect(args: &crate::args::Args, ignore: &str) -> proc_exit::ExitResult {
    log::trace!("Initializing");
    let cwd = std::env::current_dir().with_code(proc_exit::sysexits::USAGE_ERR)?;
    let repo = git2::Repository::discover(&cwd).with_code(proc_exit::sysexits::USAGE_ERR)?;

    let mut repo_config = git_stack::legacy::config::RepoConfig::from_repo(&repo)
        .with_code(proc_exit::sysexits::CONFIG_ERR)?
        .update(args.to_config());
    repo_config
        .protected_branches
        .get_or_insert_with(Vec::new)
        .push(ignore.to_owned());

    repo_config
        .write_repo(&repo)
        .with_code(proc_exit::Code::FAILURE)?;

    Ok(())
}

pub fn protected(args: &crate::args::Args) -> proc_exit::ExitResult {
    log::trace!("Initializing");
    let cwd = std::env::current_dir().with_code(proc_exit::sysexits::USAGE_ERR)?;
    let repo = git2::Repository::discover(&cwd).with_code(proc_exit::sysexits::USAGE_ERR)?;

    let repo_config = git_stack::legacy::config::RepoConfig::from_all(&repo)
        .with_code(proc_exit::sysexits::CONFIG_ERR)?
        .update(args.to_config());
    let protected = git_stack::legacy::git::ProtectedBranches::new(
        repo_config.protected_branches().iter().map(|s| s.as_str()),
    )
    .with_code(proc_exit::sysexits::CONFIG_ERR)?;

    let repo = git_stack::legacy::git::GitRepo::new(repo);
    let mut branches = git_stack::legacy::git::Branches::new([]);
    let mut protected_branches = git_stack::legacy::git::Branches::new([]);
    for branch in repo.local_branches() {
        if protected.is_protected(&branch.name) {
            log::trace!("Branch {} is protected", branch);
            protected_branches.insert(branch.clone());
            if let Some(remote) = repo.find_remote_branch(repo.pull_remote(), &branch.name) {
                protected_branches.insert(remote.clone());
                branches.insert(remote);
            }
        }
        branches.insert(branch);
    }

    for (branch_id, branches) in branches.iter() {
        if protected_branches.contains_oid(branch_id) {
            for branch in branches {
                writeln!(std::io::stdout(), "{}", branch).to_sysexits()?;
            }
        } else {
            for branch in branches {
                log::debug!("Unprotected: {}", branch);
            }
        }
    }

    Ok(())
}
