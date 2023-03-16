use std::io::Write;

use itertools::Itertools;
use proc_exit::prelude::*;

use git_stack::git::Repo;

/// Meld changes into the current commit
///
/// By default, your existing commit message will be reused.  To change the commit message, use
/// `--edit` or `--message`.
///
/// When you amend a commit that has descendants, those descendants are rebased on top of the
/// amended version of the commit, unless doing so would result in merge conflicts.
#[derive(clap::Args)]
pub struct AmendArgs {
    /// Commit to rewrite
    #[arg(default_value = "HEAD")]
    rev: String,

    /// Commit all changed files
    #[arg(short, long)]
    all: bool,

    /// Interactively add changes
    #[arg(
        short,
        long,
        short_alias = 'p',
        alias = "patch",
        conflicts_with = "all"
    )]
    interactive: bool,

    /// Force edit of commit message
    #[arg(short, long)]
    edit: bool,

    /// Commit message
    #[arg(short, long)]
    message: Option<String>,

    /// Don't actually switch
    #[arg(short = 'n', long)]
    dry_run: bool,
}

impl AmendArgs {
    pub const fn alias() -> crate::alias::Alias {
        let alias = "amend";
        let action = "stack amend";
        crate::alias::Alias {
            alias,
            action,
            action_base: action,
        }
    }

    pub fn exec(&self) -> proc_exit::ExitResult {
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

        let head_id = crate::ops::resolve_explicit_base(&repo, &self.rev)
            .with_code(proc_exit::Code::FAILURE)?
            .id;
        let head = repo.find_commit(head_id).expect("explicit bases exist");
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
            git_stack::graph::Action::Fixup => {}
            git_stack::graph::Action::Protected => {
                return Err(proc_exit::Code::FAILURE.with_message("cannot amend protected commits"));
            }
        }

        let index_tree = stage_fixup(
            &repo,
            self.all,
            self.interactive,
            stderr_palette,
            self.dry_run,
        )
        .with_code(proc_exit::Code::FAILURE)?;
        let fixup_id = commit_fixup(
            &mut repo,
            &graph.branches,
            head_id,
            index_tree,
            self.dry_run,
        )
        .with_code(proc_exit::Code::FAILURE)?;
        if let Some(fixup_id) = fixup_id {
            if let Some(parent_id) = repo.parent_ids(fixup_id).expect("commit exists").first() {
                graph.insert(git_stack::graph::Node::new(fixup_id), *parent_id);
            }
            graph.commit_set(fixup_id, git_stack::graph::Fixup);
        }
        graph
            .branches
            .update(&repo)
            .with_code(proc_exit::Code::FAILURE)?;

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

        let new_message = if let Some(message) = self.message.as_deref() {
            Some(message.trim().to_owned())
        } else if self.edit {
            use std::fmt::Write;

            let raw_commit = repo
                .raw()
                .find_commit(head_id)
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
            let head_branch = repo.head_branch();
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
            message
        } else {
            None
        };

        if fixup_id.is_none() && new_message.is_none() {
            let abbrev_id = repo
                .raw()
                .find_object(head_id, None)
                .unwrap_or_else(|e| panic!("Unexpected git2 error: {e}"))
                .short_id()
                .unwrap_or_else(|e| panic!("Unexpected git2 error: {e}"));
            let _ = writeln!(
                anstream::stderr(),
                "{} nothing to amend to {}: {}",
                stderr_palette.error("error:"),
                stderr_palette.highlight(abbrev_id.as_str().unwrap()),
                stderr_palette.hint(&head.summary)
            );
            return Err(proc_exit::Code::FAILURE.as_exit());
        }

        let mut stash_id = None;
        if !self.dry_run {
            stash_id = git_stack::git::stash_push(&mut repo, "amend");
        }

        git_stack::graph::fixup(&mut graph, &repo, git_stack::config::Fixup::Squash);
        if let Some(new_message) = new_message {
            git_stack::graph::reword_commit(&mut graph, &repo, head_id, new_message)
                .with_code(proc_exit::Code::FAILURE)?;
        }

        let mut success = true;
        let scripts = git_stack::graph::to_scripts(&graph, vec![]);
        let head_branch = repo.head_branch();
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

        if success {
            let abbrev_id = repo
                .raw()
                .find_object(head_id, None)
                .unwrap_or_else(|e| panic!("Unexpected git2 error: {e}"))
                .short_id()
                .unwrap_or_else(|e| panic!("Unexpected git2 error: {e}"));
            let _ = writeln!(
                anstream::stderr(),
                "{} to {}: {}",
                stderr_palette.good("Amended"),
                stderr_palette.highlight(abbrev_id.as_str().unwrap()),
                stderr_palette.hint(&head.summary)
            );
        }

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

fn stage_fixup(
    repo: &git_stack::git::GitRepo,
    all: bool,
    interactive: bool,
    stderr_palette: crate::ops::Palette,
    dry_run: bool,
) -> Result<git2::Oid, eyre::Error> {
    let mut index = repo.raw().index()?;
    if all {
        index.update_all(
            ["*"].iter(),
            Some(&mut |path, _| {
                let _ = writeln!(
                    anstream::stderr(),
                    "{} {}",
                    stderr_palette.good("Adding"),
                    path.display()
                );
                if dry_run {
                    // skip
                    1
                } else {
                    // confirm
                    0
                }
            }),
        )?;
    } else if interactive {
        // See
        // - https://github.com/arxanas/git-branchless/blob/master/git-branchless-record/src/lib.rs#L196
        // - https://github.com/arxanas/git-branchless/tree/master/git-record
        todo!("interactive support")
    }
    let tree_id = index.write_tree()?;
    Ok(tree_id)
}

fn commit_fixup(
    repo: &mut git_stack::git::GitRepo,
    branches: &git_stack::graph::BranchSet,
    target_id: git2::Oid,
    tree_id: git2::Oid,
    dry_run: bool,
) -> Result<Option<git2::Oid>, eyre::Error> {
    let parent_id = repo.head_commit().id;

    let id = {
        let parent_raw_commit = repo
            .raw()
            .find_commit(parent_id)
            .expect("head_commit is always valid");
        if parent_raw_commit.tree_id() == tree_id {
            return Ok(None);
        }

        let target_commit = repo.find_commit(target_id).unwrap();

        let tree = repo.raw().find_tree(tree_id)?;
        let message = format!(
            "fixup! {}",
            target_commit
                .fixup_summary()
                .unwrap_or_else(|| target_commit.summary.as_ref())
        );
        let id = git2_ext::ops::commit(
            repo.raw(),
            &parent_raw_commit.author(),
            &parent_raw_commit.committer(),
            &message,
            &tree,
            &[&parent_raw_commit],
            None,
        )?;
        log::debug!("committed {} {}", id, message);
        id
    };
    if !dry_run {
        let mut stash_id = None;
        if repo.is_dirty() {
            stash_id = repo.stash_push(None).ok();
        }

        let head_branch = repo.head_branch();
        if head_branch.is_some() {
            repo.detach()?;
        }
        for branch in branches.get(parent_id).into_iter().flatten() {
            if let Some(name) = branch.local_name() {
                repo.branch(name, id)?;
            }
        }
        if let Some(head_branch) = head_branch {
            log::debug!("switching to {} {}", head_branch, id);
            repo.switch_branch(
                head_branch
                    .local_name()
                    .expect("HEAD branch is always local"),
            )?;
        } else {
            log::debug!("switching to {}", id);
            repo.switch_commit(id)?;
        }

        if let Some(stash_id) = stash_id {
            match repo.stash_pop(stash_id) {
                Ok(()) => {
                    log::debug!("Dropped refs/stash {}", stash_id);
                }
                Err(err) => {
                    log::error!("Failed to pop {} from stash: {}", stash_id, err);
                }
            }
        }
    }
    Ok(Some(id))
}
