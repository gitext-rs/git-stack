pub use crate::graph::Node;

pub fn protect_branches(
    root: &mut Node,
    repo: &dyn crate::git::Repo,
    protected_branches: &crate::git::Branches,
) -> Result<(), git2::Error> {
    // Assuming the root is the base.  The base is not guaranteed to be a protected branch but
    // might be an ancestor of one.
    for protected_oid in protected_branches.oids() {
        if let Some(merge_base_oid) = repo.merge_base(root.local_commit.id, protected_oid) {
            if merge_base_oid == root.local_commit.id {
                root.action = crate::graph::Action::Protected;
                break;
            }
        }
    }

    for children in root.children.iter_mut() {
        protect_branches_internal(children, repo, protected_branches)?;
    }

    Ok(())
}

fn protect_branches_internal(
    nodes: &mut Vec<Node>,
    repo: &dyn crate::git::Repo,
    protected_branches: &crate::git::Branches,
) -> Result<bool, git2::Error> {
    let mut descendant_protected = false;
    for node in nodes.iter_mut().rev() {
        let mut children_protected = false;
        for children in node.children.iter_mut() {
            let child_protected = protect_branches_internal(children, repo, protected_branches)?;
            children_protected |= child_protected;
        }
        let self_protected = protected_branches.contains_oid(node.local_commit.id);
        if descendant_protected || children_protected || self_protected {
            node.action = crate::graph::Action::Protected;
            descendant_protected = true;
        }
    }

    Ok(descendant_protected)
}

pub fn rebase_branches(node: &mut Node, new_base: git2::Oid) -> Result<(), git2::Error> {
    rebase_branches_internal(node, new_base)?;

    Ok(())
}

/// Mark a new base commit for the last protected commit on each branch.
fn rebase_branches_internal(node: &mut Node, new_base: git2::Oid) -> Result<bool, git2::Error> {
    let mut all_children_rebased = true;
    for child in node.children.iter_mut() {
        let mut child_rebased = false;
        for node in child.iter_mut().rev() {
            let node_rebase = rebase_branches_internal(node, new_base)?;
            if node_rebase {
                child_rebased = true;
                break;
            }
        }
        if !child_rebased {
            all_children_rebased = false;
        }
    }

    if all_children_rebased {
        return Ok(true);
    }

    if node.action == crate::graph::Action::Protected {
        node.action = crate::graph::Action::Rebase(new_base);
        Ok(true)
    } else {
        Ok(false)
    }
}

pub fn delinearize(node: &mut Node) {
    for child in node.children.iter_mut() {
        delinearize_internal(child);
    }
}

fn delinearize_internal(nodes: &mut Vec<Node>) {
    for node in nodes.iter_mut() {
        for child in node.children.iter_mut() {
            delinearize_internal(child);
        }
    }

    let splits: Vec<_> = nodes
        .iter()
        .enumerate()
        .filter(|(_, n)| !n.branches.is_empty())
        .map(|(i, _)| i + 1)
        .rev()
        .collect();
    for split in splits {
        if split == nodes.len() {
            continue;
        }
        let child = nodes.split_off(split);
        assert!(!child.is_empty());
        nodes.last_mut().unwrap().children.push(child);
    }
}

pub fn to_script(node: &Node) -> crate::git::Script {
    let mut script = crate::git::Script::new();

    match node.action {
        crate::graph::Action::Pick => {
            // The base should be immutable, so nothing to cherry-pick
            let child_mark = node.local_commit.id;
            script
                .commands
                .push(crate::git::Command::SwitchCommit(child_mark));
            script
                .commands
                .push(crate::git::Command::RegisterMark(child_mark));
            for child in node.children.iter() {
                script
                    .dependents
                    .extend(to_script_internal(child, node.local_commit.id));
            }
        }
        crate::graph::Action::Protected => {
            let child_mark = node.local_commit.id;
            script
                .commands
                .push(crate::git::Command::SwitchCommit(child_mark));
            script
                .commands
                .push(crate::git::Command::RegisterMark(child_mark));
            for child in node.children.iter() {
                script
                    .dependents
                    .extend(to_script_internal(child, node.local_commit.id));
            }
        }
        crate::graph::Action::Rebase(new_base) => {
            script
                .commands
                .push(crate::git::Command::SwitchCommit(new_base));
            script
                .commands
                .push(crate::git::Command::RegisterMark(new_base));
            for child in node.children.iter() {
                script
                    .dependents
                    .extend(to_script_internal(child, new_base));
            }
        }
    }

    script
}

fn to_script_internal(nodes: &[Node], base_mark: git2::Oid) -> Option<crate::git::Script> {
    let mut script = crate::git::Script::new();
    for node in nodes {
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

                if !node.children.is_empty() {
                    let child_mark = node.local_commit.id;
                    script
                        .commands
                        .push(crate::git::Command::RegisterMark(child_mark));
                    for child in node.children.iter() {
                        script
                            .dependents
                            .extend(to_script_internal(child, child_mark));
                    }
                }
            }
            crate::graph::Action::Protected => {
                for child in node.children.iter() {
                    script
                        .dependents
                        .extend(to_script_internal(child, node.local_commit.id));
                }
            }
            crate::graph::Action::Rebase(new_base) => {
                script
                    .commands
                    .push(crate::git::Command::SwitchCommit(new_base));
                script
                    .commands
                    .push(crate::git::Command::RegisterMark(new_base));
                for child in node.children.iter() {
                    script
                        .dependents
                        .extend(to_script_internal(child, new_base));
                }
            }
        }
    }

    if !script.commands.is_empty() {
        script
            .commands
            .insert(0, crate::git::Command::SwitchMark(base_mark));
    }
    if script.commands.is_empty() && script.dependents.is_empty() {
        None
    } else {
        Some(script)
    }
}
