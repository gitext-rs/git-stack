use itertools::Itertools;
use proc_exit::prelude::*;

/// Rebase local branches on top of pull remotes
#[derive(clap::Args)]
pub struct SyncArgs {
    /// Don't actually switch
    #[arg(short = 'n', long)]
    dry_run: bool,
}

impl SyncArgs {
    pub const fn alias() -> crate::alias::Alias {
        let alias = "sync";
        let action = "stack sync";
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

        let head = repo.head_commit();
        let head_id = head.id;
        let mut head_branch = repo.head_branch();
        let mut onto = crate::ops::resolve_implicit_base(
            &repo,
            head_id,
            &branches,
            repo_config.auto_base_commit_count(),
        );
        let mut base = crate::ops::resolve_base_from_onto(&repo, &onto);
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
        let mut branches = branches.descendants(&repo, merge_base_oid);

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

        // Update status of remote unprotected branches
        let mut update_branches = false;
        let mut push_branches: Vec<_> = branches
            .iter()
            .flat_map(|(_, b)| b.iter())
            .filter(|b| match b.kind() {
                git_stack::graph::BranchKind::Mutable => true,
                git_stack::graph::BranchKind::Deleted
                | git_stack::graph::BranchKind::Protected
                | git_stack::graph::BranchKind::Mixed => false,
            })
            .filter_map(|b| b.push_id().and_then(|_| b.local_name()))
            .collect();
        push_branches.sort_unstable();
        if !push_branches.is_empty() {
            match crate::ops::git_prune_development(&mut repo, &push_branches, self.dry_run) {
                Ok(_) => update_branches = true,
                Err(err) => {
                    log::warn!("Skipping fetch of `{}`, {}", repo.push_remote(), err);
                }
            }
        }
        if let Some(branch) = &onto.branch {
            if let Some(remote) = &branch.remote {
                match crate::ops::git_fetch_upstream(remote, branch.name.as_str()) {
                    Ok(_) => update_branches = true,
                    Err(err) => {
                        log::warn!("Skipping pull of `{}`, {}", branch, err);
                    }
                }
            }
        }
        if update_branches {
            branches.update(&repo).with_code(proc_exit::Code::FAILURE)?;
            base.update(&repo).with_code(proc_exit::Code::FAILURE)?;
            onto.update(&repo).with_code(proc_exit::Code::FAILURE)?;
        }

        let protect_commit_count = repo_config.protect_commit_count();
        let protect_commit_age = repo_config.protect_commit_age();
        let protect_commit_time = std::time::SystemTime::now() - protect_commit_age;
        let scripts = plan_changes(
            &repo,
            &base,
            &onto,
            &branches,
            protect_commit_count,
            protect_commit_time,
        )
        .with_code(proc_exit::Code::FAILURE)?;
        let head_local_branch = head_branch.clone();
        if let Some(head_local_branch) = head_local_branch.as_ref().and_then(|b| b.local_name()) {
            for script in &scripts {
                if script.is_branch_deleted(head_local_branch) {
                    // Current branch is deleted, fallback to the local version of the onto branch,
                    // if possible.
                    if let Some(local_branch) = base
                        .branch
                        .as_ref()
                        .map(|b| b.name.as_str())
                        .and_then(|n| repo.find_local_branch(n))
                    {
                        head_branch = Some(local_branch);
                    }
                }
            }
        }

        let mut success = true;
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

fn plan_changes(
    repo: &dyn git_stack::git::Repo,
    base: &crate::ops::AnnotatedOid,
    onto: &crate::ops::AnnotatedOid,
    branches: &git_stack::graph::BranchSet,
    protect_commit_count: Option<usize>,
    protect_commit_time: std::time::SystemTime,
) -> eyre::Result<Vec<git_stack::rewrite::Script>> {
    log::trace!("Planning stack changes with base={}, onto={}", base, onto);
    let graphed_branches = branches.clone();
    let mut graph = git_stack::graph::Graph::from_branches(repo, graphed_branches)?;
    git_stack::graph::protect_branches(&mut graph);
    if let Some(protect_commit_count) = protect_commit_count {
        git_stack::graph::protect_large_branches(&mut graph, protect_commit_count);
    }
    let head_id = repo.head_commit().id;
    git_stack::graph::protect_stale_branches(&mut graph, repo, protect_commit_time, &[head_id]);
    if let Some(user) = repo.user() {
        git_stack::graph::protect_foreign_branches(&mut graph, repo, &user, &[]);
    }

    let mut dropped_branches = Vec::new();

    let onto_id = onto.id;
    let pull_start_id = base.id;
    let pull_start_id = repo.merge_base(pull_start_id, onto_id).unwrap_or(onto_id);
    git_stack::graph::rebase_development_branches(&mut graph, onto_id);
    git_stack::graph::rebase_pulled_branches(&mut graph, pull_start_id, onto_id);

    let pull_range: Vec<_> = git_stack::git::commit_range(repo, onto_id..pull_start_id)?
        .into_iter()
        .map(|id| repo.find_commit(id).unwrap())
        .collect();
    dropped_branches.extend(git_stack::graph::delete_squashed_branches_by_tree_id(
        &mut graph,
        repo,
        pull_start_id,
        pull_range.iter().map(|c| c.tree_id),
    ));
    dropped_branches.extend(git_stack::graph::delete_merged_branches(
        &mut graph,
        pull_range.iter().map(|c| c.id),
    ));

    log::trace!("Generating script");
    let scripts = git_stack::graph::to_scripts(&graph, dropped_branches);
    Ok(scripts)
}
