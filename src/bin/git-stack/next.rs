use std::io::Write;

use proc_exit::prelude::*;

/// Switch to a descendant commit
#[derive(clap::Args)]
pub struct NextArgs {
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

    /// Don't actually switch
    #[arg(short = 'n', long)]
    dry_run: bool,
}

impl NextArgs {
    pub const fn alias() -> crate::alias::Alias {
        let alias = "next";
        let action = "stack next";
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
            let message = format!("cannot move to next, {:?} in progress", repo.raw().state());
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
                    format!("could not find base between {base} and HEAD"),
                )
            })
            .with_code(proc_exit::sysexits::USAGE_ERR)?;
        let stack_branches = branches.descendants(&repo, merge_base_oid);
        let graph = git_stack::graph::Graph::from_branches(&repo, stack_branches)
            .with_code(proc_exit::Code::FAILURE)?;

        let mut current_id = head_id;
        let mut progress = 0;
        while progress < self.num_commits {
            let mut next_ids = graph.children_of(current_id).collect::<Vec<_>>();
            if next_ids.is_empty() {
                if progress == 0 {
                    let _ = writeln!(
                        std::io::stderr(),
                        "{}: no child commit",
                        stderr_palette.info.paint("note"),
                    );
                } else {
                    let _ = writeln!(
                        std::io::stderr(),
                        "{}: not enough child {}, only able to go forward {}",
                        stderr_palette.info.paint("note"),
                        if self.branch { "branches" } else { "commits" },
                        self.num_commits
                    );
                }
                break;
            }

            next_ids.sort_by_key(|id| repo.find_commit(*id).map(|c| c.time));
            if !self.oldest {
                next_ids.reverse();
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
                        "traversing {}",
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
