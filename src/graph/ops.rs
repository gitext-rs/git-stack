use std::collections::BTreeMap;
use std::collections::HashSet;

use crate::graph::Graph;
use crate::graph::Resource;

pub fn protect_branches(graph: &mut Graph) {
    let protected_oids: Vec<_> = graph
        .branches
        .iter()
        .filter_map(|(oid, branches)| {
            branches
                .iter()
                .find(|b| b.kind() == crate::graph::BranchKind::Protected)
                .map(|_| oid)
        })
        .flat_map(|protected_oid| graph.ancestors_of(protected_oid))
        .collect();
    for protected_oid in protected_oids {
        graph.commit_set(protected_oid, crate::graph::Action::Protected);
    }
}

pub fn protect_large_branches(graph: &mut Graph, max: usize) -> Vec<String> {
    let mut large_branches = Vec::new();

    'branch: for branch_id in graph.branches.oids().collect::<Vec<_>>() {
        let mut ancestors = graph.ancestors_of(branch_id).into_cursor();
        let mut count = 0;
        while let Some(ancestor_id) = ancestors.next(graph) {
            count += 1;
            for branch in graph.branches.get(ancestor_id).into_iter().flatten() {
                match branch.kind() {
                    crate::graph::BranchKind::Deleted => {
                        // Pretend it doesn't exist
                    }
                    crate::graph::BranchKind::Mutable | crate::graph::BranchKind::Mixed => {
                        // Let the parent branch take care of things
                        continue 'branch;
                    }
                    crate::graph::BranchKind::Protected => {
                        ancestors.stop();
                        continue;
                    }
                }
            }

            let action = graph
                .commit_get::<crate::graph::Action>(ancestor_id)
                .copied()
                .unwrap_or_default();
            if action.is_protected() {
                ancestors.stop();
                continue;
            }
        }
        if max <= count {
            mark_branch_protected(graph, branch_id);
            large_branches.extend(
                graph
                    .branches
                    .get(branch_id)
                    .unwrap_or(&[])
                    .iter()
                    .filter_map(|branch| branch.kind().has_user_commits().then(|| branch.name())),
            );
        }
    }

    large_branches
}

fn mark_branch_protected(graph: &mut Graph, commit_id: git2::Oid) {
    let protected_oids: Vec<_> = graph.ancestors_of(commit_id).collect();
    for protected_oid in protected_oids {
        graph.commit_set(protected_oid, crate::graph::Action::Protected);
    }
}

pub fn tag_commits_while(
    graph: &mut Graph,
    tag: impl Fn(&Graph, git2::Oid) -> Option<crate::any::BoxedEntry>,
) {
    let mut cursor = graph.descendants().into_cursor();
    while let Some(descendant_id) = cursor.next(graph) {
        if let Some(resource) = tag(graph, descendant_id) {
            graph.commit_set(descendant_id, resource);
        } else {
            cursor.stop();
        }
    }
}

pub fn trim_tagged_branch<R: Resource + Eq>(
    graph: &mut Graph,
    template: R,
) -> Vec<crate::graph::Branch> {
    let mut trimmed = Vec::new();

    let mut branches = graph
        .branches
        .iter()
        .map(|(id, _branches)| id)
        .collect::<Vec<_>>();
    let mut made_progress = true;
    while made_progress {
        made_progress = false;
        branches.retain(|id| {
            if graph.commit_get::<R>(*id) != Some(&template) {
                // Not relevant, no more processing needed
                return false;
            }

            if graph.children_of(*id).count() != 0 {
                // Children might get removed in another pass
                return true;
            }

            let mut to_remove = Vec::new();
            let mut cursor = graph.ancestors_of(*id).into_cursor();
            while let Some(candidate_id) = cursor.next(graph) {
                if candidate_id == graph.root_id() {
                    // Always must have at least one commit, don't remove
                    cursor.stop();
                } else if 1 < graph.children_of(*id).count() {
                    // Shared commit, don't remove
                    cursor.stop();
                } else if graph
                    .commit_get::<crate::graph::Action>(candidate_id)
                    .copied()
                    .unwrap_or_default()
                    .is_protected()
                {
                    // Protected commit, don't remove
                    cursor.stop();
                } else if candidate_id != *id && graph.branches.contains_oid(candidate_id) {
                    // Hit another branch which needs its own evaluation for whether we should
                    // remove it
                    cursor.stop();
                } else {
                    trimmed.extend(graph.branches.remove(candidate_id).unwrap_or_default());
                    to_remove.push(candidate_id);
                }
            }
            for id in to_remove {
                graph.remove(id);
            }
            made_progress = true;

            false
        });
    }

    trimmed
}

pub fn protect_tagged_branch<R: Resource + Eq>(graph: &mut Graph, template: R) {
    let branches = graph
        .branches
        .iter()
        .map(|(id, _branches)| id)
        .filter(|id| graph.commit_get::<R>(*id) == Some(&template))
        .collect::<Vec<_>>();
    for branch_id in branches {
        mark_branch_protected(graph, branch_id);
    }
}

pub fn tag_stale_commits(
    graph: &mut Graph,
    repo: &dyn crate::git::Repo,
    earlier_than: std::time::SystemTime,
    ignore: &[git2::Oid],
) {
    tag_commits_while(graph, |_graph, id| {
        if ignore.contains(&id) {
            return None;
        }
        let commit = repo.find_commit(id)?;
        (commit.time < earlier_than).then(|| StaleCommit.into())
    })
}

pub fn trim_stale_branches(
    graph: &mut Graph,
    repo: &dyn crate::git::Repo,
    earlier_than: std::time::SystemTime,
    ignore: &[git2::Oid],
) -> Vec<crate::graph::Branch> {
    tag_stale_commits(graph, repo, earlier_than, ignore);
    trim_tagged_branch(graph, StaleCommit)
}

pub fn protect_stale_branches(
    graph: &mut Graph,
    repo: &dyn crate::git::Repo,
    earlier_than: std::time::SystemTime,
    ignore: &[git2::Oid],
) {
    tag_stale_commits(graph, repo, earlier_than, ignore);
    protect_tagged_branch(graph, StaleCommit);
}

#[derive(Copy, Clone, Default, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct StaleCommit;

impl crate::any::ResourceTag for StaleCommit {}

pub fn tag_foreign_commits(
    graph: &mut Graph,
    repo: &dyn crate::git::Repo,
    user: &str,
    ignore: &[git2::Oid],
) {
    tag_commits_while(graph, |_graph, id| {
        if ignore.contains(&id) {
            return None;
        }
        let commit = repo.find_commit(id)?;
        (commit.committer.as_deref() != Some(user) && commit.author.as_deref() != Some(user))
            .then(|| ForeignCommit.into())
    })
}

pub fn trim_foreign_branches(
    graph: &mut Graph,
    repo: &dyn crate::git::Repo,
    user: &str,
    ignore: &[git2::Oid],
) -> Vec<crate::graph::Branch> {
    tag_foreign_commits(graph, repo, user, ignore);
    trim_tagged_branch(graph, ForeignCommit)
}

pub fn protect_foreign_branches(
    graph: &mut Graph,
    repo: &dyn crate::git::Repo,
    user: &str,
    ignore: &[git2::Oid],
) {
    tag_foreign_commits(graph, repo, user, ignore);
    protect_tagged_branch(graph, ForeignCommit);
}

#[derive(Copy, Clone, Default, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ForeignCommit;

impl crate::any::ResourceTag for ForeignCommit {}

/// Pre-requisites:
/// - Running protect_branches
///
/// # Panics
///
/// - If `new_base_id` doesn't exist
pub fn rebase_development_branches(graph: &mut Graph, onto_id: git2::Oid) {
    let mut descendants = graph.descendants().into_cursor();
    while let Some(descendant_id) = descendants.next(graph) {
        let action = graph
            .commit_get::<crate::graph::Action>(descendant_id)
            .copied()
            .unwrap_or_default();
        if action.is_protected() {
            continue;
        }

        let bases: Vec<_> = graph
            .parents_of(descendant_id)
            .filter(|id| *id != onto_id)
            .filter(|id| {
                let action = graph
                    .commit_get::<crate::graph::Action>(*id)
                    .copied()
                    .unwrap_or_default();
                action.is_protected()
            })
            .collect();
        for base in bases {
            graph.rebase(descendant_id, base, onto_id);
        }

        descendants.stop();
    }
}

/// Update branches from `pull_start` to `pull_end`
///
/// A normal `rebase_development_branches` only looks at development commits.  If `main` is pristine or if the
/// user has branches on the same commit as `main`, we should also update these to what we pulled.
pub fn rebase_pulled_branches(graph: &mut Graph, pull_start: git2::Oid, pull_end: git2::Oid) {
    if pull_start == pull_end {
        return;
    }

    let branches = if let Some(branches) = graph.branches.remove(pull_start) {
        branches
    } else {
        return;
    };
    let (mut end_branches, start_branches): (Vec<_>, Vec<_>) = branches
        .into_iter()
        .partition(|b| b.kind().has_user_commits());
    graph.branches.extend(start_branches);

    for end_branch in &mut end_branches {
        end_branch.set_id(pull_end);
    }
    graph.branches.extend(end_branches);
}

pub fn mark_wip(graph: &mut Graph, repo: &dyn crate::git::Repo) {
    let mut cursor = graph.descendants().into_cursor();
    while let Some(current_id) = cursor.next(graph) {
        if graph
            .commit_get::<crate::graph::Action>(current_id)
            .copied()
            .unwrap_or_default()
            .is_protected()
        {
            continue;
        }

        let commit = repo
            .find_commit(current_id)
            .expect("all commits in graph present in git");
        if commit.wip_summary().is_some() {
            graph.commit_set(current_id, Wip);
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Wip;

impl crate::any::ResourceTag for Wip {}

pub fn mark_fixup(graph: &mut Graph, repo: &dyn crate::git::Repo) {
    let mut cursor = graph.descendants().into_cursor();
    while let Some(current_id) = cursor.next(graph) {
        if graph
            .commit_get::<crate::graph::Action>(current_id)
            .copied()
            .unwrap_or_default()
            .is_protected()
        {
            continue;
        }

        let commit = repo
            .find_commit(current_id)
            .expect("all commits in graph present in git");
        if commit.fixup_summary().is_some() {
            graph.commit_set(current_id, Fixup);
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Fixup;

impl crate::any::ResourceTag for Fixup {}

pub fn pushable(graph: &mut Graph) {
    let branches = graph
        .branches
        .iter()
        .map(|(id, _branches)| id)
        .collect::<Vec<_>>();
    for branch_id in branches {
        mark_push_status(graph, branch_id);
    }
}

fn mark_push_status(graph: &mut Graph, branch_id: git2::Oid) -> Option<PushStatus> {
    if let Some(status) = graph.commit_get::<PushStatus>(branch_id) {
        return Some(*status);
    }

    let mut status = Some(PushStatus::Pushable);

    if graph
        .commit_get::<crate::graph::Action>(branch_id)
        .copied()
        .unwrap_or_default()
        .is_protected()
    {
        log::debug!(
            "Branches at {} aren't pushable, the commit is protected",
            branch_id
        );
        status = None;
    } else {
        let mut ancestors = graph.ancestors_of(branch_id).into_cursor();
        while let Some(parent_id) = ancestors.next(graph) {
            if graph
                .commit_get::<crate::graph::Action>(parent_id)
                .copied()
                .unwrap_or_default()
                .is_protected()
            {
                ancestors.stop();
            } else if graph.commit_get::<Wip>(parent_id).is_some() {
                log::debug!(
                    "Branches at {} aren't pushable, commit {} is WIP",
                    branch_id,
                    parent_id,
                );
                status = Some(PushStatus::Blocked("wip"));
                break;
            } else if branch_id != parent_id && graph.branches.contains_oid(parent_id) {
                let parent_status = mark_push_status(graph, parent_id);
                match parent_status {
                    Some(PushStatus::Blocked(reason)) => {
                        log::debug!(
                            "Branches at {} aren't pushable, parent commit {} is blocked for {}",
                            branch_id,
                            parent_id,
                            reason
                        );
                        status = Some(PushStatus::Blocked("parent branch"));
                        break;
                    }
                    Some(PushStatus::Pushed) | Some(PushStatus::Pushable) => {
                        log::debug!("Branches at {} aren't pushable, parent branch at {} should be pushed first", branch_id, parent_id);
                        status = Some(PushStatus::Blocked("parent branch"));
                        break;
                    }
                    None => {
                        // Must be a protected branch, safe for us to push
                    }
                }
                ancestors.stop();
            }
        }
    }

    if graph
        .branches
        .get(branch_id)
        .into_iter()
        .flatten()
        .any(|b| Some(b.id()) == b.push_id())
    {
        // User pushed, so trust them.  Consider all other branches as empty branches
        log::debug!("A branch at {} is already pushed", branch_id);
        status = Some(PushStatus::Pushed);
    }

    if let Some(status) = status {
        graph.commit_set(branch_id, status);
    }
    status
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum PushStatus {
    Blocked(&'static str),
    Pushed,
    Pushable,
}

impl Default for PushStatus {
    fn default() -> Self {
        Self::Blocked("protected")
    }
}

impl crate::any::ResourceTag for PushStatus {}

pub fn hide_protected_branch_commits(graph: &mut Graph, visible: &[git2::Oid]) {
    let hidden_oids: Vec<_> = graph
        .branches
        .iter()
        .filter_map(|(oid, branches)| {
            branches
                .iter()
                .find(|b| b.kind() == crate::graph::BranchKind::Protected)
                .map(|_| oid)
        })
        .flat_map(|branch_oid| graph.ancestors_of(branch_oid))
        .filter(|oid| {
            let is_branch = graph.branches.get(*oid).is_some();
            let is_forced = visible.contains(oid);
            let is_root = *oid == graph.root_id();
            let is_leaf = graph.children_of(*oid).count() == 0;
            !(is_branch || is_forced || is_root || is_leaf)
        })
        .collect();
    for hidden_oid in hidden_oids {
        graph.commit_set(hidden_oid, Hidden);
    }
}

pub fn hide_branch_commits(graph: &mut Graph, visible: &[git2::Oid]) {
    let hidden_oids: Vec<_> = graph
        .branches
        .oids()
        .flat_map(|branch_oid| graph.ancestors_of(branch_oid))
        .filter(|oid| {
            let is_branch = graph.branches.get(*oid).is_some();
            let is_forced = visible.contains(oid);
            let is_root = *oid == graph.root_id();
            let is_leaf = graph.children_of(*oid).count() == 0;
            !(is_branch || is_forced || is_root || is_leaf)
        })
        .collect();
    for hidden_oid in hidden_oids {
        graph.commit_set(hidden_oid, Hidden);
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Hidden;

impl crate::any::ResourceTag for Hidden {}

/// Quick pass for what is droppable
///
/// We get into this state when a branch is squashed.  The id would be different due to metadata
/// but the tree_id, associated with the repo, is the same if your branch is up-to-date.
///
/// The big risk is if a commit was reverted.  To protect against this, we only look at the final
/// state of the branch and then check if it looks like a revert.
///
/// To avoid walking too much of the tree, we are going to assume only the first branch in a stack
/// could have been squash-merged.
///
/// This assumes that the Node was rebased onto all of the new potentially squash-merged Nodes and
/// we extract the potential tree_id's from those protected commits.
pub fn delete_squashed_branches_by_tree_id(
    graph: &mut Graph,
    repo: &dyn crate::git::Repo,
    pull_start_id: git2::Oid,
    pulled_tree_ids: impl Iterator<Item = git2::Oid>,
) -> Vec<crate::graph::Branch> {
    let mut removed = Vec::new();

    let pulled_tree_ids: HashSet<_> = pulled_tree_ids.collect();
    let mut descendants = graph.descendants_of(pull_start_id).into_cursor();
    while let Some(descendant_id) = descendants.next(graph) {
        if pulled_tree_ids.contains(&descendant_id) {
            descendants.stop();
            continue;
        }

        let branches = if let Some(branches) = graph.branches.get_mut(descendant_id) {
            branches
        } else {
            continue;
        };
        let mut stop = false;
        for branch in branches {
            match branch.kind() {
                crate::graph::BranchKind::Deleted => {
                    // Pretend it doesn't exist
                }
                crate::graph::BranchKind::Mutable => {
                    stop = true;

                    let commit = repo
                        .find_commit(descendant_id)
                        .expect("graph entries are valid");
                    if pulled_tree_ids.contains(&commit.tree_id) {
                        branch.set_kind(crate::graph::BranchKind::Deleted);
                        removed.push(branch.clone());
                    }
                }
                crate::graph::BranchKind::Mixed | crate::graph::BranchKind::Protected => {
                    stop = true;
                }
            }
        }
        if stop {
            descendants.stop();
        }
    }

    removed
}

/// Drop branches merged among the pulled IDs
///
/// Marking it deleted in `graph` is purely superficial since nothing can act on it.  The returned
/// branches are the important part.
pub fn delete_merged_branches(
    graph: &mut Graph,
    pulled_ids: impl Iterator<Item = git2::Oid>,
) -> Vec<crate::graph::Branch> {
    let mut removed = Vec::new();
    for pulled_id in pulled_ids {
        let branches = if let Some(branches) = graph.branches.get_mut(pulled_id) {
            branches
        } else {
            continue;
        };
        for branch in branches {
            if branch.kind() == crate::graph::BranchKind::Mutable {
                branch.set_kind(crate::graph::BranchKind::Deleted);
                removed.push(branch.clone());
            }
        }
    }
    removed
}

pub fn fixup(graph: &mut Graph, repo: &dyn crate::git::Repo, effect: crate::config::Fixup) {
    if effect == crate::config::Fixup::Ignore {
        return;
    }

    let mut fixups = Vec::new();

    let mut descendants = graph.descendants().into_cursor();
    while let Some(descendant_id) = descendants.next(graph) {
        let action = graph
            .commit_get::<crate::graph::Action>(descendant_id)
            .copied()
            .unwrap_or_default();
        if action.is_protected() {
            continue;
        }

        let commit = repo
            .find_commit(descendant_id)
            .expect("all commits in graph present in git");
        if let Some(summary) = commit.fixup_summary() {
            fixups.push((descendant_id, summary.to_owned()));
        }
    }

    for (fixup_id, summary) in fixups {
        let mut ancestors = graph.ancestors_of(fixup_id).into_cursor();
        let _self = ancestors.next(graph);
        assert_eq!(_self, Some(fixup_id));
        let mut fixed = false;
        while let Some(ancestor_id) = ancestors.next(graph) {
            let action = graph
                .commit_get::<crate::graph::Action>(ancestor_id)
                .copied()
                .unwrap_or_default();
            if action.is_protected() {
                ancestors.stop();
                continue;
            }

            let anc_commit = repo
                .find_commit(ancestor_id)
                .expect("all commits in graph present in git");
            if let Some(anc_summary) = anc_commit.fixup_summary() {
                if anc_summary == summary {
                    fixup_commit(graph, fixup_id, ancestor_id, effect);
                    fixed = true;
                    break;
                }
            }
            if anc_commit.summary == summary {
                fixup_commit(graph, fixup_id, ancestor_id, effect);
                fixed = true;
                break;
            }
        }
        if !fixed {
            log::trace!(
                "Could not find base commit for fixup {} ({})",
                fixup_id,
                summary
            );
        }
    }
}

fn fixup_commit(
    graph: &mut Graph,
    fixup_id: git2::Oid,
    target_id: git2::Oid,
    effect: crate::config::Fixup,
) {
    debug_assert_ne!(fixup_id, target_id);
    // Re-target all branches from the fixup commit to the next most-recent commit
    let branches = graph.branches.remove(fixup_id);
    let fixup_parent_id = graph
        .primary_parent_of(fixup_id)
        .expect("if there is a target, there is a parent");
    for mut branch in branches.into_iter().flatten() {
        branch.set_id(fixup_parent_id);
        graph.branches.insert(branch);
    }

    // Move the fixup commit
    let node = graph.remove(fixup_id).expect("fixup is always valid");
    graph.insert(node, target_id);

    // Re-parent all commits to the fixup commit
    for target_child_id in graph.children_of(target_id).collect::<Vec<_>>() {
        if target_child_id != fixup_id {
            graph.rebase(target_child_id, target_id, fixup_id);
        }
    }

    // Re-target all branches from the target to the fixup
    let branches = graph.branches.remove(target_id);
    for mut branch in branches.into_iter().flatten() {
        if branch.kind().has_user_commits() {
            branch.set_id(fixup_id);
        }
        graph.branches.insert(branch);
    }

    match effect {
        crate::config::Fixup::Ignore => unreachable!(),
        crate::config::Fixup::Squash => {
            graph.commit_set(fixup_id, crate::graph::Action::Fixup);
        }
        crate::config::Fixup::Move => {
            // Happened above
        }
    }
}

/// When a branch has extra commits, update dependent branches to the latest
pub fn realign_stacks(graph: &mut Graph, repo: &dyn crate::git::Repo) {
    let mut descendants = graph.descendants().into_cursor();
    while let Some(descendant_id) = descendants.next(graph) {
        let action = graph
            .commit_get::<crate::graph::Action>(descendant_id)
            .copied()
            .unwrap_or_default();
        if action.is_protected() {
            continue;
        }

        realign_stack(graph, repo, descendant_id);
        descendants.stop();
    }
}

fn realign_stack(graph: &mut Graph, repo: &dyn crate::git::Repo, base_id: git2::Oid) {
    let mut old_edges = Vec::new();

    let mut current_id = base_id;
    loop {
        if graph
            .branches
            .get(current_id)
            .unwrap_or_default()
            .is_empty()
        {
            let mut children = graph.children_of(current_id).collect::<Vec<_>>();
            match children.len() {
                0 => {
                    // Can't continue
                    break;
                }
                1 => {
                    current_id = children[0];
                }
                _ => {
                    // Assuming the more recent work is a continuation of the existing stack and
                    // aligning the other stacks to be on top of it
                    //
                    // This should be safe in light of our rebases since we don't preserve the time
                    children.sort_unstable_by_key(|id| {
                        repo.find_commit(*id)
                            .expect("all commits in graph present in git")
                            .time
                    });
                    let newest = children.pop().unwrap();
                    current_id = newest;
                    old_edges.extend(children.into_iter().map(|child_id| (current_id, child_id)));
                }
            }
        } else {
            // Alignment point found
            break;
        }
    }

    for (parent_id, child_id) in old_edges {
        graph.rebase(child_id, parent_id, current_id);
    }
    let children = graph.children_of(current_id).collect::<Vec<_>>();
    for child_id in children {
        realign_stack(graph, repo, child_id);
    }
}

/// When a rebase has split stack, re-combine them
pub fn merge_stacks_by_tree_id(graph: &mut Graph, repo: &dyn crate::git::Repo) {
    let mut descendants = graph.descendants().into_cursor();
    while let Some(descendant_id) = descendants.next(graph) {
        let mut unprotected_children = CommitTimesByTreeId::new();
        for child_id in graph.children_of(descendant_id) {
            let action = graph
                .commit_get::<crate::graph::Action>(descendant_id)
                .copied()
                .unwrap_or_default();
            if action.is_protected() {
                continue;
            }

            let commit = repo
                .find_commit(child_id)
                .expect("all commits in graph present in git");
            unprotected_children
                .entry(commit.tree_id)
                .or_insert_with(Vec::new)
                .push((commit.time, child_id));
        }
        for mut commits in unprotected_children.into_values() {
            if commits.len() < 2 {
                continue;
            }

            commits.sort_unstable();
            let (_, winner_id) = commits.pop().expect("checked len earlier");

            for (_, from_id) in commits {
                let rebased_grandchildren = graph.children_of(from_id).collect::<Vec<_>>();
                for grandchild_id in rebased_grandchildren {
                    graph.rebase(grandchild_id, from_id, winner_id);
                }
                for mut branch in graph.branches.remove(from_id).into_iter().flatten() {
                    branch.set_id(winner_id);
                    graph.branches.insert(branch);
                }
                graph.remove(from_id);
            }
        }
    }
}

type CommitTimesByTreeId = BTreeMap<git2::Oid, Vec<(std::time::SystemTime, git2::Oid)>>;

pub fn reword_commit(
    graph: &mut Graph,
    repo: &dyn crate::git::Repo,
    id: git2::Oid,
    message: String,
) -> Result<(), eyre::Error> {
    eyre::ensure!(
        graph.contains_id(id),
        "cannot rewrite commit {}, not present",
        id
    );

    let commit = repo
        .find_commit(id)
        .expect("graph.contains_id ensures commit exists");
    for descendant_id in graph.descendants_of(id) {
        let descendant_commit = repo
            .find_commit(descendant_id)
            .expect("graph.descendants_of ensures commit exists");
        if Some(commit.summary.as_ref()) == descendant_commit.fixup_summary() {
            eyre::bail!("cannot reword; first squash dependent fixups")
        }
    }

    graph.commit_set(id, Reword(message));

    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Reword(String);

impl crate::any::ResourceTag for Reword {}

pub fn to_scripts(
    graph: &Graph,
    dropped_branches: Vec<super::Branch>,
) -> Vec<crate::rewrite::Script> {
    let mut scripts = Vec::new();
    let mut dropped_branches = dropped_branches
        .into_iter()
        .map(|b| {
            (
                b.id(),
                b.local_name()
                    .expect("only local branches passed in")
                    .to_owned(),
            )
        })
        .collect::<std::collections::HashMap<_, _>>();

    let mut descendants = graph.descendants().into_cursor();
    let mut seen = std::collections::HashSet::new();
    while let Some(descendant_id) = descendants.next(graph) {
        for child_id in graph.children_of(descendant_id) {
            let action = graph
                .commit_get::<crate::graph::Action>(child_id)
                .copied()
                .unwrap_or_default();
            if !seen.insert(child_id) {
            } else if action.is_protected() {
                let mut batch = crate::rewrite::Batch::new(child_id);
                if let Some(dropped) = dropped_branches.remove(&descendant_id) {
                    batch.push(
                        descendant_id,
                        crate::rewrite::Command::DeleteBranch(dropped),
                    );
                }
                for branch in graph.branches.get(child_id).into_iter().flatten() {
                    if branch.kind().has_user_commits() {
                        if let Some(local_name) = branch.local_name() {
                            batch.push(
                                child_id,
                                crate::rewrite::Command::CreateBranch(local_name.to_owned()),
                            );
                        }
                    }
                }
                if !batch.is_empty() {
                    scripts.push(vec![batch].into());
                }
            } else {
                descendants.stop();
                let mut script = Vec::new();
                gather_script(
                    graph,
                    descendant_id,
                    child_id,
                    &mut dropped_branches,
                    &mut script,
                );
                scripts.push(script.into());
            }
        }
    }

    if !dropped_branches.is_empty() {
        let mut batch = crate::rewrite::Batch::new(graph.root_id());
        for (id, branch) in dropped_branches {
            batch.push(id, crate::rewrite::Command::DeleteBranch(branch));
        }
        scripts.insert(0, vec![batch].into());
    }

    scripts
}

fn gather_script(
    graph: &Graph,
    onto_id: git2::Oid,
    start_id: git2::Oid,
    dropped_branches: &mut std::collections::HashMap<git2::Oid, String>,
    script: &mut Vec<crate::rewrite::Batch>,
) {
    let mut batch = crate::rewrite::Batch::new(onto_id);

    let mut current_id = Some(start_id);
    while let Some(id) = current_id {
        if let Some(dropped) = dropped_branches.remove(&id) {
            batch.push(id, crate::rewrite::Command::DeleteBranch(dropped));
        }
        let action = graph
            .commit_get::<crate::graph::Action>(id)
            .copied()
            .unwrap_or_default();
        match action {
            crate::graph::Action::Pick => {
                batch.push(id, crate::rewrite::Command::CherryPick(id));
                if let Some(Reword(message)) = graph.commit_get::<Reword>(id) {
                    batch.push(id, crate::rewrite::Command::Reword(message.clone()));
                }
                for branch in graph.branches.get(id).into_iter().flatten() {
                    if branch.kind().has_user_commits() {
                        if let Some(local_name) = branch.local_name() {
                            batch.push(
                                id,
                                crate::rewrite::Command::CreateBranch(local_name.to_owned()),
                            );
                        }
                    }
                }
            }
            crate::graph::Action::Fixup => {
                batch.push(id, crate::rewrite::Command::Fixup(id));
                for branch in graph.branches.get(id).into_iter().flatten() {
                    if branch.kind().has_user_commits() {
                        if let Some(local_name) = branch.local_name() {
                            batch.push(
                                id,
                                crate::rewrite::Command::CreateBranch(local_name.to_owned()),
                            );
                        }
                    }
                }
            }
            crate::graph::Action::Protected => {
                unreachable!(
                    "Rebasing {} (via {}) onto {} should not have protected commits",
                    id, start_id, onto_id
                );
            }
        }

        current_id = None;
        for (i, child_id) in graph.children_of(id).enumerate() {
            match i {
                0 if 1 < graph.parents_of(child_id).count() => {
                    current_id = None;
                    gather_script(graph, id, child_id, dropped_branches, script);
                }
                0 => {
                    current_id = Some(child_id);
                }
                _ => {
                    gather_script(graph, id, child_id, dropped_branches, script);
                }
            }
        }
    }

    script.push(batch);
}
