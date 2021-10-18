pub use crate::graph::Node;

pub fn protect_branches(
    root: &mut Node,
    repo: &dyn crate::git::Repo,
    protected_branches: &crate::git::Branches,
) {
    let mut protected_commits = std::collections::HashSet::new();
    for protected_oid in protected_branches.oids() {
        if let Some(merge_base_oid) = repo.merge_base(root.commit.id, protected_oid) {
            if merge_base_oid == root.commit.id {
                for commit in repo.commits_from(protected_oid) {
                    protected_commits.insert(commit.id);
                    if commit.id == root.commit.id {
                        break;
                    }
                }
            }
        }
    }

    protect_branches_node(root, &protected_commits);
}

fn protect_branches_node(
    node: &mut Node,
    protected_commits: &std::collections::HashSet<git2::Oid>,
) {
    if protected_commits.contains(&node.commit.id) {
        node.action = crate::graph::Action::Protected;
        for child in node.children.values_mut() {
            protect_branches_node(child, protected_commits);
        }
    }
}

/// Pre-requisites:
/// - Running protect_branches
///
/// # Panics
///
/// - If `new_base_id` doesn't exist
/// - If `new_base_id` isn't protected
pub fn rebase_branches(node: &mut Node, new_base_id: git2::Oid) {
    debug_assert_eq!(
        node.find_commit_mut(new_base_id).unwrap().action,
        crate::graph::Action::Protected
    );
    let mut rebaseable = Vec::new();
    pop_rebaseable_stacks(node, &mut rebaseable);

    let new_base = node.find_commit_mut(new_base_id).unwrap();
    new_base
        .children
        .extend(rebaseable.into_iter().map(|n| (n.commit.id, n)));
}

fn pop_rebaseable_stacks(node: &mut Node, rebaseable: &mut Vec<Node>) {
    if !node.action.is_protected() {
        // The parent is responsible for popping this node
        return;
    }

    let mut base_ids = Vec::new();
    for (child_id, child) in node.children.iter_mut() {
        if child.action.is_protected() {
            pop_rebaseable_stacks(child, rebaseable);
        } else {
            base_ids.push(*child_id);
        }
    }
    for base_id in base_ids {
        let child = node.children.remove(&base_id).unwrap();
        rebaseable.push(child);
    }
}

pub fn pushable(node: &mut Node) {
    if node.action.is_protected() {
        for child in node.children.values_mut() {
            pushable_node(child, None);
        }
    } else {
        // No idea if a parent commit invalidates our results
    }
}

fn pushable_node(node: &mut Node, mut cause: Option<&str>) {
    if node.action.is_protected() {
        assert_eq!(cause, None);
        for child in node.children.values_mut() {
            pushable_node(child, cause);
        }
        return;
    }

    if node.commit.wip_summary().is_some() {
        cause = Some("contains WIP commit");
    }

    if !node.branches.is_empty() {
        let branch = &node.branches[0];
        if let Some(cause) = cause {
            log::debug!("{} isn't pushable, {}", branch.name, cause);
        } else if node.branches.iter().all(|b| Some(b.id) == b.push_id) {
            log::debug!("{} is already pushed", branch.name);
        } else {
            log::debug!("{} is pushable", branch.name);
            node.pushable = true;
        }
        // Bail out, only the first branch of a stack is up for consideration
        return;
    }

    for stack in node.children.values_mut() {
        pushable_node(stack, cause);
    }
}

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
pub fn drop_by_tree_id(node: &mut Node) {
    if node.action.is_protected() {
        track_protected_tree_id(node, std::collections::HashSet::new());
    }
}

fn track_protected_tree_id(
    node: &mut Node,
    mut protected_tree_ids: std::collections::HashSet<git2::Oid>,
) {
    assert!(node.action.is_protected());
    protected_tree_ids.insert(node.commit.tree_id);

    match node.children.len() {
        0 => (),
        1 => {
            let child = node.children.values_mut().next().unwrap();
            if child.action.is_protected() {
                track_protected_tree_id(child, protected_tree_ids);
            } else {
                drop_first_branch_by_tree_id(child, protected_tree_ids);
            }
        }
        _ => {
            for child in node.children.values_mut() {
                if child.action.is_protected() {
                    track_protected_tree_id(child, protected_tree_ids.clone());
                } else {
                    drop_first_branch_by_tree_id(child, protected_tree_ids.clone());
                }
            }
        }
    }
}

fn drop_first_branch_by_tree_id(
    node: &mut Node,
    protected_tree_ids: std::collections::HashSet<git2::Oid>,
) -> bool {
    #![allow(clippy::if_same_then_else)]

    assert!(!node.action.is_protected());
    if node.branches.is_empty() {
        match node.children.len() {
            0 => false,
            1 => {
                let child = node.children.values_mut().next().unwrap();
                let all_dropped = drop_first_branch_by_tree_id(child, protected_tree_ids);
                if all_dropped {
                    node.action = crate::graph::Action::Delete;
                }
                all_dropped
            }
            _ => {
                let mut all_dropped = true;
                for child in node.children.values_mut() {
                    all_dropped &= drop_first_branch_by_tree_id(child, protected_tree_ids.clone());
                }
                if all_dropped {
                    node.action = crate::graph::Action::Delete;
                }
                all_dropped
            }
        }
    } else if !protected_tree_ids.contains(&node.commit.tree_id) {
        false
    } else if node.commit.revert_summary().is_some() {
        // Might not *actually* be a revert or something more complicated might be going on.  Let's
        // just be cautious.
        false
    } else {
        node.action = crate::graph::Action::Delete;
        true
    }
}

pub fn fixup(node: &mut Node, effect: crate::config::Fixup) {
    if effect == crate::config::Fixup::Ignore {
        return;
    }

    let mut outstanding = std::collections::BTreeMap::new();
    fixup_nodes(node, effect, &mut outstanding);
    if !outstanding.is_empty() {
        assert!(!node.action.is_protected());
        for nodes in outstanding.into_values() {
            for mut other in nodes.into_iter() {
                std::mem::swap(node, &mut other);
                node.children.insert(other.commit.id, other);
            }
        }
    }
}

fn fixup_nodes(
    node: &mut Node,
    effect: crate::config::Fixup,
    outstanding: &mut std::collections::BTreeMap<bstr::BString, Vec<Node>>,
) {
    let mut fixups = Vec::new();
    for (id, child) in node.children.iter_mut() {
        fixup_nodes(child, effect, outstanding);

        if child.action.is_protected() || child.action.is_delete() {
            continue;
        }
        if let Some(summary) = node.commit.fixup_summary() {
            fixups.push((*id, summary.to_owned()));
        }
    }

    for (id, summary) in fixups {
        let mut child = node.children.remove(&id).unwrap();

        let mut new_children = Default::default();
        std::mem::swap(&mut child.children, &mut new_children);
        node.children.extend(new_children);

        let mut new_branches = Default::default();
        std::mem::swap(&mut child.branches, &mut new_branches);
        node.branches.extend(new_branches);

        outstanding
            .entry(summary)
            .or_insert_with(Default::default)
            .push(child);
    }

    if let Some(mut fixups) = outstanding.remove(&node.commit.summary) {
        if effect == crate::config::Fixup::Squash {
            for fixup in fixups.iter_mut() {
                assert!(fixup.action == crate::graph::Action::Pick);
                fixup.action = crate::graph::Action::Squash;
            }
        }
        splice_after(node, fixups);
    } else if (node.action.is_protected() || node.action.is_delete()) && !outstanding.is_empty() {
        let mut local = Default::default();
        std::mem::swap(&mut local, outstanding);

        let mut outstanding = local.into_values();
        let mut fixups = outstanding.next().unwrap();
        fixups.extend(outstanding.flatten());
        splice_after(node, fixups);
    }
}

fn splice_after(node: &mut Node, fixups: Vec<Node>) -> &mut Node {
    let mut new_children = Default::default();
    std::mem::swap(&mut node.children, &mut new_children);

    let mut new_branches = Default::default();
    std::mem::swap(&mut node.branches, &mut new_branches);

    let mut current = node;
    for fixup in fixups.into_iter().rev() {
        current = current.children.entry(fixup.commit.id).or_insert(fixup);
    }

    std::mem::swap(&mut current.children, &mut new_children);
    assert!(new_children.is_empty());

    std::mem::swap(&mut current.branches, &mut new_branches);
    assert!(new_branches.is_empty());

    current
}

pub fn to_script(node: &Node) -> crate::git::Script {
    let mut script = crate::git::Script::new();

    match node.action {
        // The base should be immutable, so nothing to cherry-pick
        crate::graph::Action::Pick | crate::graph::Action::Protected => {
            let node_dependents: Vec<_> = node
                .children
                .values()
                .filter_map(|child| node_to_script(child))
                .collect();
            if !node_dependents.is_empty() {
                let stack_mark = node.commit.id;
                script
                    .commands
                    .push(crate::git::Command::SwitchCommit(stack_mark));

                let transaction = false;
                extend_dependents(node, &mut script, node_dependents, transaction);
            }
        }
        crate::graph::Action::Squash => unreachable!("base should be immutable"),
        crate::graph::Action::Delete => unreachable!("base should be immutable"),
    }

    script
}

fn node_to_script(node: &Node) -> Option<crate::git::Script> {
    let mut script = crate::git::Script::new();

    match node.action {
        crate::graph::Action::Pick => {
            script
                .commands
                .push(crate::git::Command::CherryPick(node.commit.id));
            for branch in node.branches.iter() {
                script
                    .commands
                    .push(crate::git::Command::CreateBranch(branch.name.clone()));
            }

            let node_dependents: Vec<_> = node
                .children
                .values()
                .filter_map(|child| node_to_script(child))
                .collect();
            if !node_dependents.is_empty() {
                // End the transaction on branch boundaries
                let transaction = !node.branches.is_empty();
                extend_dependents(node, &mut script, node_dependents, transaction);
            }
        }
        crate::graph::Action::Squash => {
            script
                .commands
                .push(crate::git::Command::Squash(node.commit.id));
            // We can't re-target the branches of the commit we are squashing into, so the ops that
            // creates a `Squash` option has to handle that.
            for branch in node.branches.iter() {
                script
                    .commands
                    .push(crate::git::Command::CreateBranch(branch.name.clone()));
            }

            let node_dependents: Vec<_> = node
                .children
                .values()
                .filter_map(|child| node_to_script(child))
                .collect();
            if !node_dependents.is_empty() {
                // End the transaction on branch boundaries
                let transaction = !node.branches.is_empty();
                extend_dependents(node, &mut script, node_dependents, transaction);
            }
        }
        crate::graph::Action::Protected => {
            let node_dependents: Vec<_> = node
                .children
                .values()
                .filter_map(|child| node_to_script(child))
                .collect();
            if !node_dependents.is_empty() {
                let stack_mark = node.commit.id;
                script
                    .commands
                    .push(crate::git::Command::SwitchCommit(stack_mark));

                // No transactions needed for protected commits
                let transaction = false;
                extend_dependents(node, &mut script, node_dependents, transaction);
            }
        }
        crate::graph::Action::Delete => {
            for branch in node.branches.iter() {
                script
                    .commands
                    .push(crate::git::Command::DeleteBranch(branch.name.clone()));
            }

            let node_dependents: Vec<_> = node
                .children
                .values()
                .filter_map(|child| node_to_script(child))
                .collect();
            if !node_dependents.is_empty() {
                // End the transaction on branch boundaries
                let transaction = !node.branches.is_empty();
                extend_dependents(node, &mut script, node_dependents, transaction);
            }
        }
    }

    if script.is_empty() {
        None
    } else {
        Some(script)
    }
}

fn extend_dependents(
    node: &Node,
    script: &mut crate::git::Script,
    mut dependents: Vec<crate::git::Script>,
    transaction: bool,
) {
    // Create transactions at the branch boundaries
    if !transaction && dependents.len() == 1 {
        let dependent = dependents.remove(0);
        script.commands.extend(dependent.commands);
        script.dependents.extend(dependent.dependents);
    } else {
        // Ensure each dependent can pick up where needed
        let stack_mark = node.commit.id;
        script
            .commands
            .push(crate::git::Command::RegisterMark(stack_mark));
        for dependent in dependents.iter_mut() {
            dependent
                .commands
                .insert(0, crate::git::Command::SwitchMark(stack_mark));
        }
        script.dependents.extend(dependents);
    }
}
