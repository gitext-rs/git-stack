use std::io::Write;

use proc_exit::prelude::*;

/// Run across commands in the current stack
#[derive(clap::Args)]
pub struct RunArgs {
    #[arg(value_names = ["COMMAND", "ARG"], trailing_var_arg = true, required=true)]
    command: Vec<std::ffi::OsString>,

    /// Don't actually switch
    #[arg(short = 'n', long)]
    dry_run: bool,
}

impl RunArgs {
    pub const fn alias() -> crate::alias::Alias {
        let alias = "run";
        let action = "stack run";
        crate::alias::Alias {
            alias,
            action,
            action_base: action,
        }
    }

    pub fn exec(&self, _colored_stdout: bool, colored_stderr: bool) -> proc_exit::ExitResult {
        let stderr_palette = if colored_stderr {
            crate::ops::Palette::colored()
        } else {
            crate::ops::Palette::plain()
        };

        let cwd = std::env::current_dir().with_code(proc_exit::sysexits::USAGE_ERR)?;
        let repo = git2::Repository::discover(cwd).with_code(proc_exit::sysexits::USAGE_ERR)?;
        let mut repo = git_stack::git::GitRepo::new(repo);

        let repo_config = git_stack::config::RepoConfig::from_all(repo.raw())
            .with_code(proc_exit::sysexits::CONFIG_ERR)?;
        repo.set_push_remote(repo_config.push_remote());
        repo.set_pull_remote(repo_config.pull_remote());

        let protected = git_stack::git::ProtectedBranches::new(
            repo_config.protected_branches().iter().map(|s| s.as_str()),
        )
        .with_code(proc_exit::sysexits::CONFIG_ERR)?;
        let branches = git_stack::graph::BranchSet::from_repo(&repo, &protected)
            .with_code(proc_exit::Code::FAILURE)?;

        let mut stash_id = None;
        if !self.dry_run {
            stash_id = git_stack::git::stash_push(&mut repo, "run");
        }
        if repo.is_dirty() {
            let message = "Working tree is dirty, aborting";
            if self.dry_run {
                let _ = writeln!(
                    std::io::stderr(),
                    "{}: {}",
                    stderr_palette.error.paint("error"),
                    message
                );
            } else {
                return Err(proc_exit::sysexits::USAGE_ERR.with_message(message));
            }
        }

        let head_branch = repo.head_branch();
        let head_id = repo.head_commit().id;
        let base = crate::ops::resolve_implicit_base(
            &repo,
            head_id,
            &branches,
            repo_config.auto_base_commit_count(),
        );
        let merge_base_oid = repo
            .merge_base(base.id, head_id)
            .ok_or_else(|| {
                git2::Error::new(
                    git2::ErrorCode::NotFound,
                    git2::ErrorClass::Reference,
                    format!("could not find base between {} and HEAD", base),
                )
            })
            .with_code(proc_exit::sysexits::USAGE_ERR)?;
        let stack_branches = branches.dependents(&repo, merge_base_oid, head_id);
        let graph = git_stack::graph::Graph::from_branches(&repo, stack_branches)
            .with_code(proc_exit::Code::FAILURE)?;

        let mut success = true;
        for current_id in graph.descendants_of(merge_base_oid) {
            let current_commit = repo
                .find_commit(current_id)
                .expect("children/head are always present");
            let _ = writeln!(
                std::io::stderr(),
                "{} to {}: {}",
                stderr_palette.good.paint("Switching"),
                stderr_palette
                    .highlight
                    .paint(crate::ops::render_id(&repo, &branches, current_id)),
                stderr_palette.hint.paint(&current_commit.summary)
            );
            if !self.dry_run {
                repo.switch_commit(current_id)
                    .with_code(proc_exit::Code::FAILURE)?;
            }
            let status = std::process::Command::new(&self.command[0])
                .args(&self.command[1..])
                .status();
            match status {
                Ok(status) if status.success() => {
                    let _ = writeln!(
                        std::io::stderr(),
                        "{}",
                        stderr_palette.good.paint("Success"),
                    );
                }
                Ok(status) => match status.code() {
                    Some(code) => {
                        let _ = writeln!(
                            std::io::stderr(),
                            "{}: exit code {}",
                            stderr_palette.error.paint("Failed"),
                            code,
                        );
                        success = false;
                    }
                    None => {
                        let _ = writeln!(
                            std::io::stderr(),
                            "{}: signal caught",
                            stderr_palette.error.paint("Failed"),
                        );
                        success = false;
                    }
                },
                Err(err) => {
                    let _ = writeln!(
                        std::io::stderr(),
                        "{}: {}",
                        stderr_palette.error.paint("Failed"),
                        err
                    );
                    success = false;
                }
            }
        }

        if let Some(branch) = head_branch {
            if !self.dry_run {
                repo.switch_branch(branch.local_name().expect("HEAD is always local"))
                    .with_code(proc_exit::Code::FAILURE)?;
            }
        } else {
            if !self.dry_run {
                repo.switch_commit(head_id)
                    .with_code(proc_exit::Code::FAILURE)?;
            }
        }

        git_stack::git::stash_pop(&mut repo, stash_id);

        if success {
            Ok(())
        } else {
            Err(proc_exit::Code::FAILURE.as_exit())
        }
    }
}
