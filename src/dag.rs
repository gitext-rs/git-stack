use itertools::Itertools;

pub fn graph(
    repo: &dyn crate::repo::Repo,
    mut base_oid: git2::Oid,
    head_oid: git2::Oid,
    protected_branches: &crate::branches::Branches,
    mut graph_branches: crate::branches::Branches,
) -> Result<Node, git2::Error> {
    let mut root = Node::populate(repo, base_oid, head_oid, &mut graph_branches)?;

    if !graph_branches.is_empty() {
        let branch_oids: Vec<_> = graph_branches.oids().collect();
        for branch_oid in branch_oids {
            if protected_branches.contains_oid(branch_oid) {
                continue;
            }

            let branches = if let Some(branches) = graph_branches.get(branch_oid) {
                branches
            } else {
                continue;
            };
            let branch_name = branches.first().unwrap().name.clone();
            let merge_base_oid = repo.merge_base(base_oid, branch_oid).ok_or_else(|| {
                git2::Error::new(
                    git2::ErrorCode::NotFound,
                    git2::ErrorClass::Reference,
                    "Could not find merge base",
                )
            })?;
            if merge_base_oid != base_oid {
                match Node::populate(repo, merge_base_oid, base_oid, &mut graph_branches) {
                    Ok(mut prefix) => {
                        prefix.push(root);
                        root = prefix;
                    }
                    Err(err) => {
                        log::error!("Could not generate prefix for {}: {}", branch_name, err);
                    }
                }
                base_oid = merge_base_oid;
            }
            match Node::populate(repo, base_oid, branch_oid, &mut graph_branches) {
                Ok(branch_root) => {
                    root.merge(branch_root);
                }
                Err(err) => {
                    log::error!("Unhandled branch {}: {}", branch_name, err);
                }
            }
        }
    }

    if !graph_branches.is_empty() {
        let unused_branches = graph_branches
            .iter()
            .flat_map(|(_, branches)| branches)
            .map(|branch| branch.name.as_str())
            .join(", ");
        log::error!("Unhandled branches: {}", unused_branches);
    }

    Ok(root)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Node {
    pub local_commit: std::rc::Rc<crate::repo::Commit>,
    pub branches: Vec<crate::repo::Branch>,
    pub children: Vec<Vec<Node>>,
    pub action: crate::actions::Action,
}

impl Node {
    pub fn display(&self) -> DisplayTree<'_> {
        DisplayTree::new(self)
    }

    fn populate(
        repo: &dyn crate::repo::Repo,
        base_oid: git2::Oid,
        head_oid: git2::Oid,
        branches: &mut crate::branches::Branches,
    ) -> Result<Self, git2::Error> {
        if let Some(head_branches) = branches.get(head_oid) {
            let head_name = head_branches.first().unwrap().name.as_str();
            log::trace!("Populating data for {}", head_name);
        } else {
            log::trace!("Populating data for {}", head_oid);
        }
        let merge_base_oid = repo.merge_base(base_oid, head_oid).ok_or_else(|| {
            git2::Error::new(
                git2::ErrorCode::NotFound,
                git2::ErrorClass::Reference,
                "Could not find merge base",
            )
        })?;
        if merge_base_oid != base_oid {
            return Err(git2::Error::new(
                git2::ErrorCode::NotFound,
                git2::ErrorClass::Reference,
                "HEAD must be a descendant of base",
            ));
        }
        let base_commit = repo.find_commit(base_oid).unwrap();

        let mut root = Node::from_commit(base_commit).with_branches(branches);

        let mut children: Vec<_> = repo
            .commits_from(head_oid)
            .take_while(|commit| commit.id != merge_base_oid)
            .map(|commit| Node::from_commit(commit).with_branches(branches))
            .collect();
        children.reverse();
        if !children.is_empty() {
            root.children.push(children);
        }

        Ok(root)
    }

    fn from_commit(local_commit: std::rc::Rc<crate::repo::Commit>) -> Self {
        let branches = Vec::new();
        let children = Vec::new();
        Self {
            local_commit,
            branches,
            children,
            action: crate::actions::Action::Pick,
        }
    }

    fn with_branches(mut self, possible_branches: &mut crate::branches::Branches) -> Self {
        if let Some(branches) = possible_branches.remove(self.local_commit.id) {
            self.branches = branches;
        }
        self
    }

    fn push(&mut self, other: Self) {
        let other_oid = other.local_commit.id;
        if self.local_commit.id == other_oid {
            self.merge(other);
        } else if self.children.len() == 1 {
            let child = &mut self.children[0];
            for node in child.iter_mut() {
                if node.local_commit.id == other_oid {
                    node.merge(other);
                    return;
                }
            }
            unimplemented!("This case isn't needed yet");
        } else {
            unimplemented!("This case isn't needed yet");
        }
    }

    fn merge(&mut self, mut other: Self) {
        assert_eq!(self.local_commit.id, other.local_commit.id);

        let mut branches = Vec::new();
        std::mem::swap(&mut other.branches, &mut branches);
        self.branches.extend(branches);

        let other_children = other.children;
        for mut other_children_branch in other_children {
            assert!(!other_children_branch.is_empty());
            for mut self_children_branch in self.children.iter_mut() {
                merge_nodes(&mut self_children_branch, &mut other_children_branch);
                if other_children_branch.is_empty() {
                    break;
                }
            }
            if !other_children_branch.is_empty() {
                self.children.push(other_children_branch);
            }
        }
    }
}

/// If a merge occurs, `rhs_nodes` will be empty
fn merge_nodes(lhs_nodes: &mut Vec<Node>, rhs_nodes: &mut Vec<Node>) {
    assert!(
        !lhs_nodes.is_empty(),
        "to exist, there has to be at least one node"
    );
    assert!(
        !rhs_nodes.is_empty(),
        "to exist, there has to be at least one node"
    );

    for (lhs, rhs) in lhs_nodes.iter_mut().zip(rhs_nodes.iter_mut()) {
        if lhs.local_commit.id != rhs.local_commit.id {
            break;
        }
        let mut branches = Vec::new();
        std::mem::swap(&mut rhs.branches, &mut branches);
        lhs.branches.extend(branches);
    }

    let index = lhs_nodes
        .iter()
        .zip_longest(rhs_nodes.iter())
        .enumerate()
        .find(|(_, zipped)| match zipped {
            itertools::EitherOrBoth::Both(lhs, rhs) => lhs.local_commit.id != rhs.local_commit.id,
            _ => true,
        })
        .map(|(index, zipped)| {
            let zipped = zipped.map_any(|_| (), |_| ());
            (index, zipped)
        });

    match index {
        Some((index, itertools::EitherOrBoth::Both(_, _)))
        | Some((index, itertools::EitherOrBoth::Right(_))) => {
            if index == 0 {
                // Not a good merge candidate, find another
            } else {
                let remaining = rhs_nodes.split_off(index);
                lhs_nodes[index - 1].children.push(remaining);
                rhs_nodes.clear();
            }
        }
        Some((_, itertools::EitherOrBoth::Left(_))) | None => {
            // lhs is a superset, so consider us done.
            rhs_nodes.clear();
        }
    }
}

pub struct DisplayTree<'n> {
    root: &'n Node,
    palette: Palette,
    all: bool,
}

impl<'n> DisplayTree<'n> {
    pub fn new(root: &'n Node) -> Self {
        Self {
            root,
            palette: Palette::plain(),
            all: false,
        }
    }

    pub fn colored(mut self, yes: bool) -> Self {
        if yes {
            self.palette = Palette::colored()
        } else {
            self.palette = Palette::plain()
        }
        self
    }

    pub fn all(mut self, yes: bool) -> Self {
        self.all = yes;
        self
    }
}

impl<'n> std::fmt::Display for DisplayTree<'n> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        let mut tree = treeline::Tree::root(RenderNode {
            node: Some(self.root),
            palette: &self.palette,
        });
        to_tree(
            self.root.children.as_slice(),
            &mut tree,
            &self.palette,
            self.all,
        );
        tree.fmt(f)
    }
}

fn to_tree<'n, 'p>(
    nodes: &'n [Vec<Node>],
    tree: &mut treeline::Tree<RenderNode<'n, 'p>>,
    palette: &'p Palette,
    show_all: bool,
) {
    for branch in nodes {
        let mut branch_root = treeline::Tree::root(RenderNode {
            node: None,
            palette,
        });
        for node in branch {
            if node.branches.is_empty() && node.children.is_empty() && !show_all {
                log::trace!("Skipping commit {}", node.local_commit.id);
                continue;
            }
            let mut child_tree = treeline::Tree::root(RenderNode {
                node: Some(node),
                palette,
            });
            to_tree(node.children.as_slice(), &mut child_tree, palette, show_all);
            branch_root.push(child_tree);
        }
        tree.push(branch_root);
    }
}

struct RenderNode<'n, 'p> {
    node: Option<&'n Node>,
    palette: &'p Palette,
}

impl<'n, 'p> std::fmt::Display for RenderNode<'n, 'p> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        if let Some(node) = self.node.as_ref() {
            if node.branches.is_empty() {
                write!(f, "{} ", self.palette.info.paint(node.local_commit.id))?;
            } else if node.action == crate::actions::Action::Protected {
                write!(
                    f,
                    "{} ",
                    self.palette
                        .info
                        .paint(node.branches.iter().map(|b| b.name.as_str()).join(", ")),
                )?;
            } else {
                write!(
                    f,
                    "{} ",
                    self.palette
                        .branch
                        .paint(node.branches.iter().map(|b| b.name.as_str()).join(", ")),
                )?;
            }

            let summary = String::from_utf8_lossy(&node.local_commit.summary);
            if node.action == crate::actions::Action::Protected {
                write!(f, "{}", self.palette.hint.paint(summary))?;
            } else if node.local_commit.is_merge {
                write!(f, "{}", self.palette.error.paint("merge commit"))?;
            } else if node.branches.is_empty() && !node.children.is_empty() {
                // Branches should be off of other branches
                write!(f, "{}", self.palette.warn.paint(summary))?;
            } else if node.local_commit.fixup_summary().is_some() {
                // Needs to be squashed
                write!(f, "{}", self.palette.warn.paint(summary))?;
            } else if node.local_commit.wip_summary().is_some() {
                // Not for pushing implicitly
                write!(f, "{}", self.palette.error.paint(summary))?;
            } else {
                write!(f, "{}", self.palette.hint.paint(summary))?;
            }
        } else {
            write!(f, "o")?;
        }
        Ok(())
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Palette {
    error: yansi::Style,
    warn: yansi::Style,
    info: yansi::Style,
    branch: yansi::Style,
    hint: yansi::Style,
}

impl Palette {
    pub fn colored() -> Self {
        Self {
            error: yansi::Style::new(yansi::Color::Red),
            warn: yansi::Style::new(yansi::Color::Yellow),
            info: yansi::Style::new(yansi::Color::Blue),
            branch: yansi::Style::new(yansi::Color::Green),
            hint: yansi::Style::new(yansi::Color::Blue).dimmed(),
        }
    }

    pub fn plain() -> Self {
        Self {
            error: yansi::Style::default(),
            warn: yansi::Style::default(),
            info: yansi::Style::default(),
            branch: yansi::Style::default(),
            hint: yansi::Style::default(),
        }
    }
}

pub fn protect_branches(
    root: &mut Node,
    repo: &dyn crate::repo::Repo,
    protected_branches: &crate::branches::Branches,
) -> Result<(), git2::Error> {
    // Assuming the root is the base.  The base is not guaranteed to be a protected branch but
    // might be an ancestor of one.
    for protected_oid in protected_branches.oids() {
        if let Some(merge_base_oid) = repo.merge_base(root.local_commit.id, protected_oid) {
            if merge_base_oid == root.local_commit.id {
                root.action = crate::actions::Action::Protected;
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
    repo: &dyn crate::repo::Repo,
    protected_branches: &crate::branches::Branches,
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
            node.action = crate::actions::Action::Protected;
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

    if node.action == crate::actions::Action::Protected {
        node.action = crate::actions::Action::Rebase(new_base);
        Ok(true)
    } else {
        Ok(false)
    }
}
