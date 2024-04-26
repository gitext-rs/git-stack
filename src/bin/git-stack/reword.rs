use std::io::Write;

use itertools::Itertools;
use proc_exit::prelude::*;

use git_stack::git::Repo;

/// Rewrite the commit message
///
/// When you reword a commit that has descendants, those descendants are rebased on top of the
/// reworded version of the commit.
#[derive(clap::Args)]
pub(crate) struct RewordArgs {
    /// Commit to rewrite
    #[arg(default_value = "HEAD")]
    rev: String,

    /// Commit message
    #[arg(short, long)]
    message: Option<String>,

    /// Don't actually switch
    #[arg(short = 'n', long)]
    dry_run: bool,
}

impl RewordArgs {
    pub(crate) const fn alias() -> crate::alias::Alias {
        let alias = "reword";
        let action = "stack reword";
        crate::alias::Alias {
            alias,
            action,
            action_base: action,
        }
    }

    pub(crate) fn exec(&self) -> proc_exit::ExitResult {
        let stderr_palette = crate::ops::Palette::colored();

        let cwd = std::env::current_dir().with_code(proc_exit::sysexits::USAGE_ERR)?;
        let repo = git2::Repository::discover(&cwd).with_code(proc_exit::sysexits::USAGE_ERR)?;
        let mut repo = git_stack::git::GitRepo::new(repo);

        let repo_config = git_stack::config::RepoConfig::from_all(repo.raw())
            .with_code(proc_exit::sysexits::CONFIG_ERR)?;
        repo.set_push_remote(repo_config.push_remote());
        repo.set_pull_remote(repo_config.pull_remote());
        let config = repo
            .raw()
            .config()
            .with_code(proc_exit::sysexits::CONFIG_ERR)?;
        repo.set_sign(
            config
                .get_bool("stack.gpgSign")
                .or_else(|_| config.get_bool("commit.gpgSign"))
                .unwrap_or_default(),
        )
        .with_code(proc_exit::Code::FAILURE)?;

        let protected = git_stack::git::ProtectedBranches::new(
            repo_config.protected_branches().iter().map(|s| s.as_str()),
        )
        .with_code(proc_exit::sysexits::CONFIG_ERR)?;
        let branches = git_stack::graph::BranchSet::from_repo(&repo, &protected)
            .with_code(proc_exit::Code::FAILURE)?;

        let head_ann_id = crate::ops::resolve_explicit_base(&repo, &self.rev)
            .with_code(proc_exit::Code::FAILURE)?;
        let head_id = head_ann_id.id;
        let head = repo.find_commit(head_id).expect("resolve found a commit");
        let head_branch = head_ann_id.branch.as_ref();
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
        let mut graph = git_stack::graph::Graph::from_branches(&repo, stack_branches)
            .with_code(proc_exit::Code::FAILURE)?;
        git_stack::graph::protect_branches(&mut graph);
        git_stack::graph::mark_fixup(&mut graph, &repo);
        git_stack::graph::mark_wip(&mut graph, &repo);

        if repo.raw().state() != git2::RepositoryState::Clean {
            let message = format!("cannot walk commits, {:?} in progress", repo.raw().state());
            if self.dry_run {
                let _ = writeln!(
                    anstream::stderr(),
                    "{}: {}",
                    stderr_palette.error("error"),
                    message
                );
            } else {
                return Err(proc_exit::sysexits::USAGE_ERR.with_message(message));
            }
        }
        let action = graph
            .commit_get::<git_stack::graph::Action>(head_id)
            .copied()
            .unwrap_or_default();
        match action {
            git_stack::graph::Action::Pick => {}
            git_stack::graph::Action::Fixup => {
                return Err(proc_exit::Code::FAILURE.with_message("cannot reword fixup commits"));
            }
            git_stack::graph::Action::Protected => {
                return Err(
                    proc_exit::Code::FAILURE.with_message("cannot reword protected commits")
                );
            }
        }

        let new_message = if let Some(message) = self.message.as_deref() {
            message.trim().to_owned()
        } else {
            use std::fmt::Write;

            let raw_commit = repo
                .raw()
                .find_commit(head.id)
                .expect("head_commit is always valid");
            let existing = String::from_utf8_lossy(raw_commit.message_bytes());
            let mut template = String::new();
            writeln!(&mut template, "{existing}").unwrap();
            writeln!(&mut template).unwrap();
            writeln!(
                &mut template,
                "# Please enter the commit message for your changes. Lines starting"
            )
            .unwrap();
            writeln!(
                &mut template,
                "# with '#' will be ignored, and an empty message aborts the commit."
            )
            .unwrap();
            if let Some(head_branch) = &head_branch {
                writeln!(&mut template, "#").unwrap();
                writeln!(&mut template, "# On branch {head_branch}").unwrap();
            }
            let message = crate::ops::edit_commit(
                repo.path()
                    .ok_or_else(|| eyre::format_err!("no `.git` path found"))
                    .with_code(proc_exit::Code::FAILURE)?,
                repo_config.editor(),
                &template,
            )
            .with_code(proc_exit::Code::FAILURE)?;
            let message = match message {
                Some(message) => message,
                None => {
                    return Err(proc_exit::Code::SUCCESS.with_message("Nothing to do."));
                }
            };
            message
        };

        git_stack::graph::reword_commit(&mut graph, &repo, head_id, new_message)
            .with_code(proc_exit::Code::FAILURE)?;

        let mut stash_id = None;
        if !self.dry_run {
            stash_id = git_stack::git::stash_push(&mut repo, "reword");
        }

        let mut backed_up = false;
        {
            let stash_repo =
                git2::Repository::discover(&cwd).with_code(proc_exit::sysexits::USAGE_ERR)?;
            let stash_repo = git_branch_stash::GitRepo::new(stash_repo);
            let mut snapshots =
                git_branch_stash::Stack::new(crate::ops::STASH_STACK_NAME, &stash_repo);
            let snapshot_capacity = repo_config.capacity();
            snapshots.capacity(snapshot_capacity);
            let snapshot = git_branch_stash::Snapshot::from_repo(&stash_repo)
                .with_code(proc_exit::Code::FAILURE)?;
            if !self.dry_run {
                snapshots.push(snapshot).to_sysexits()?;
                backed_up = true;
            }
        }

        let mut success = true;
        let scripts = git_stack::graph::to_scripts(&graph, vec![]);
        let mut executor = git_stack::rewrite::Executor::new(self.dry_run);
        for script in scripts {
            let results = executor.run(&mut repo, &script);
            for (err, name, dependents) in results.iter() {
                success = false;
                log::error!("Failed to re-stack branch `{}`: {}", name, err);
                if !dependents.is_empty() {
                    log::error!("  Blocked dependents: {}", dependents.iter().join(", "));
                }
            }
        }
        executor
            .close(&mut repo, head_branch.as_ref().and_then(|b| b.local_name()))
            .with_code(proc_exit::Code::FAILURE)?;

        git_stack::git::stash_pop(&mut repo, stash_id);
        if backed_up {
            anstream::eprintln!(
                "{}: to undo, run {}",
                stderr_palette.info("note"),
                stderr_palette.highlight(format_args!(
                    "`git branch-stash pop {}`",
                    crate::ops::STASH_STACK_NAME
                ))
            );
        }

        if success {
            Ok(())
        } else {
            Err(proc_exit::Code::FAILURE.as_exit())
        }
    }
}
