use bstr::ByteSlice;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct AnnotatedOid {
    pub id: git2::Oid,
    pub branch: Option<git_stack::git::Branch>,
}

impl AnnotatedOid {
    pub fn new(id: git2::Oid) -> Self {
        Self { id, branch: None }
    }

    pub fn with_branch(branch: git_stack::git::Branch) -> Self {
        Self {
            id: branch.id,
            branch: Some(branch),
        }
    }
}

impl std::fmt::Display for AnnotatedOid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(branch) = &self.branch {
            branch.fmt(f)
        } else {
            self.id.fmt(f)
        }
    }
}

pub fn resolve_implicit_base(
    repo: &dyn git_stack::git::Repo,
    head_oid: git2::Oid,
    branches: &git_stack::graph::BranchSet,
    auto_base_commit_count: Option<usize>,
) -> AnnotatedOid {
    match git_stack::graph::find_protected_base(repo, branches, head_oid) {
        Some(branch) => {
            let merge_base_id = repo
                .merge_base(branch.id(), head_oid)
                .expect("to be a base, there must be a merge base");
            if let Some(max_commit_count) = auto_base_commit_count {
                let ahead_count = repo
                    .commit_count(merge_base_id, head_oid)
                    .expect("merge_base should ensure a count exists ");
                let behind_count = repo
                    .commit_count(merge_base_id, branch.id())
                    .expect("merge_base should ensure a count exists ");
                if max_commit_count <= ahead_count + behind_count {
                    let assumed_base_oid =
                        git_stack::graph::infer_base(repo, head_oid).unwrap_or(head_oid);
                    log::warn!(
                        "`{}` is {} ahead and {} behind `{}`, using `{}` as `--base` instead",
                        branches
                            .get(head_oid)
                            .map(|b| b[0].name())
                            .or_else(|| {
                                repo.find_commit(head_oid)?
                                    .summary
                                    .to_str()
                                    .ok()
                                    .map(ToOwned::to_owned)
                            })
                            .unwrap_or_else(|| "target".to_owned()),
                        ahead_count,
                        behind_count,
                        branch.display_name(),
                        assumed_base_oid
                    );
                    return AnnotatedOid::new(assumed_base_oid);
                }
            }

            log::debug!(
                "Chose branch `{}` as the base for `{}`",
                branch.display_name(),
                branches
                    .get(head_oid)
                    .map(|b| b[0].name())
                    .or_else(|| {
                        repo.find_commit(head_oid)?
                            .summary
                            .to_str()
                            .ok()
                            .map(ToOwned::to_owned)
                    })
                    .unwrap_or_else(|| "target".to_owned())
            );
            AnnotatedOid::with_branch(branch.git().to_owned())
        }
        None => {
            let assumed_base_oid = git_stack::graph::infer_base(repo, head_oid).unwrap_or(head_oid);
            log::warn!(
                "Could not find protected branch for {}, assuming {}",
                head_oid,
                assumed_base_oid
            );
            AnnotatedOid::new(assumed_base_oid)
        }
    }
}

pub fn render_id(
    repo: &git_stack::git::GitRepo,
    branches: &git_stack::graph::BranchSet,
    id: git2::Oid,
) -> String {
    if let Some(current) = branches.get(id) {
        let mut current = current.to_owned();
        current.sort_by_key(|b| b.kind());
        let current_branch = current.first().expect("always at least one");
        let name = current_branch.display_name().to_string();
        name
    } else {
        repo.raw()
            .find_object(id, None)
            .unwrap_or_else(|e| panic!("Unexpected git2 error: {}", e))
            .short_id()
            .unwrap_or_else(|e| panic!("Unexpected git2 error: {}", e))
            .as_str()
            .unwrap()
            .to_owned()
    }
}
