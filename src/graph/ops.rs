pub use crate::graph::Node;

pub fn protect_branches(
    root: &mut Node,
    repo: &dyn crate::git::Repo,
    protected_branches: &crate::git::Branches,
) {
    // Assuming the root is the base.  The base is not guaranteed to be a protected branch but
    // might be an ancestor of one that was not included in the graph.
    //
    // We can't use `descendant_protected` because a protect branch might not be in the
    // descendants, depending on what Graph the user selected.
    for protected_oid in protected_branches.oids() {
        if let Some(merge_base_oid) = repo.merge_base(root.local_commit.id, protected_oid) {
            if merge_base_oid == root.local_commit.id {
                root.action = crate::graph::Action::Protected;
                break;
            }
        }
    }

    for node in root.children.values_mut() {
        protect_branches_node(node, repo, protected_branches);
    }
}

fn protect_branches_node(
    node: &mut Node,
    repo: &dyn crate::git::Repo,
    protected_branches: &crate::git::Branches,
) -> bool {
    // Can't short-circuit since we need to ensure all nodes are marked.
    let mut is_protected = false;
    for child in node.children.values_mut() {
        is_protected |= protect_branches_node(child, repo, protected_branches);
    }

    is_protected |= protected_branches.contains_oid(node.local_commit.id);

    if is_protected {
        node.action = crate::graph::Action::Protected;
    }

    is_protected
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
        .extend(rebaseable.into_iter().map(|n| (n.local_commit.id, n)));
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

    if node.local_commit.wip_summary().is_some() {
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

pub fn to_script(node: &Node) -> crate::git::Script {
    let mut script = crate::git::Script::new();

    match node.action {
        // The base should be immutable, so nothing to cherry-pick
        crate::graph::Action::Pick | crate::graph::Action::Protected => {
            let stack_mark = node.local_commit.id;
            for child in node.children.values() {
                script.dependents.extend(node_to_script(child));
            }
            if !script.dependents.is_empty() {
                script
                    .commands
                    .push(crate::git::Command::SwitchCommit(stack_mark));
            }
        }
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
                .push(crate::git::Command::CherryPick(node.local_commit.id));
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
                let stack_mark = node.local_commit.id;
                script
                    .commands
                    .push(crate::git::Command::SwitchCommit(stack_mark));

                // No transactions needed for protected commits
                let transaction = false;
                extend_dependents(node, &mut script, node_dependents, transaction);
            }
        }
        crate::graph::Action::Delete => {
            assert!(node.children.is_empty());
            for branch in node.branches.iter() {
                script
                    .commands
                    .push(crate::git::Command::DeleteBranch(branch.name.clone()));
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
        let stack_mark = node.local_commit.id;
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
