use std::io::Write;

use proc_exit::prelude::*;

/// Switch to an ancestor commit
#[derive(clap::Args)]
pub struct PrevArgs {
    /// Jump back the specified number of commits or branches
    #[arg(default_value = "1")]
    num_commits: usize,

    /// Jump directly to the previous branch
    #[arg(short, long)]
    branch: bool,

    /// Stash prior to switch
    #[arg(long)]
    stash: bool,

    /// On ambiguity, select the oldest commit
    #[arg(long)]
    oldest: bool,

    /// Traverse across protected commits
    #[arg(long)]
    protected: bool,

    /// Don't actually switch
    #[arg(short = 'n', long)]
    dry_run: bool,
}

impl PrevArgs {
    pub const fn alias() -> crate::alias::Alias {
        let alias = "prev";
        let action = "stack previous";
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

        if repo.raw().state() != git2::RepositoryState::Clean {
            let message = format!(
                "cannot move to previous, {:?} in progress",
                repo.raw().state()
            );
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

        if self.stash && !self.dry_run {
            git_stack::git::stash_push(&mut repo, "branch-stash");
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

        let head_id = repo.head_commit().id;
        let mut current_id = head_id;
        let mut progress = 0;
        while progress < self.num_commits {
            let is_current_protected = matches!(
                branches
                    .get(current_id)
                    .and_then(|b| b.iter().map(|b| b.kind()).max()),
                // HACK: We should be checking whether the commits are protected, rather than
                // to allow user commits on Mixed branches
                Some(git_stack::graph::BranchKind::Protected)
                    | Some(git_stack::graph::BranchKind::Mixed)
            );
            if is_current_protected && !self.protected {
                if progress == 0 {
                    let _ = writeln!(
                        std::io::stderr(),
                        "{}: no unprotected parent commit; to traverse protected commits, pass `--protected`",
                        stderr_palette.info.paint("note"),
                    );
                } else {
                    let _ = writeln!(
                        std::io::stderr(),
                        "{}: not enough unprotected parent {}, only able to go back {}; to traverse protected commits, pass `--protected`",
                        stderr_palette.info.paint("note"),
                        if self.branch { "branches" } else { "commits" },
                        self.num_commits
                    );
                }
                break;
            }

            let mut next_ids = repo
                .parent_ids(current_id)
                .with_code(proc_exit::Code::FAILURE)?;
            if next_ids.is_empty() {
                if progress == 0 {
                    let _ = writeln!(
                        std::io::stderr(),
                        "{}: no parent commit",
                        stderr_palette.info.paint("note"),
                    );
                } else {
                    let _ = writeln!(
                        std::io::stderr(),
                        "{}: not enough parent {}, only able to go forward {}",
                        stderr_palette.info.paint("note"),
                        if self.branch { "branches" } else { "commits" },
                        self.num_commits
                    );
                }
                break;
            }

            if self.oldest {
                next_ids.sort_by_key(|id| {
                    let branch_kind = branches
                        .get(*id)
                        .and_then(|b| b.iter().map(|b| b.kind()).max());
                    let commit_time = repo.find_commit(*id).map(|c| c.time);
                    // Prefer user branches
                    (branch_kind, commit_time)
                });
            } else {
                next_ids.sort_by_key(|id| {
                    let branch_kind = branches
                        .get(*id)
                        .and_then(|b| b.iter().map(|b| b.kind()).max());
                    let commit_time = repo.find_commit(*id).map(|c| c.time);
                    // Prefer user branches
                    (branch_kind, std::cmp::Reverse(commit_time))
                });
            }
            current_id = *next_ids.first().expect("next_ids.is_empty checked");
            if 1 < next_ids.len() {
                log::debug!(
                    "selected {} over {}",
                    crate::ops::render_id(&repo, &branches, current_id),
                    next_ids
                        .iter()
                        .skip(1)
                        .map(|id| crate::ops::render_id(&repo, &branches, *id))
                        .collect::<Vec<_>>()
                        .join(", ")
                );
            }
            if self.branch {
                if let Some(current) = branches.get(current_id) {
                    log::debug!(
                        "Traversing {}",
                        current
                            .iter()
                            .map(|b| b.display_name().to_string())
                            .collect::<Vec<_>>()
                            .join(", ")
                    );
                    progress += 1;
                }
            } else {
                progress += 1;
            }
        }

        if current_id != head_id {
            crate::ops::switch(
                &mut repo,
                &branches,
                current_id,
                stderr_palette,
                self.dry_run,
            )
            .with_code(proc_exit::Code::FAILURE)?;
        }

        Ok(())
    }
}
