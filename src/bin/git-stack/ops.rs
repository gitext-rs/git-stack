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

pub fn sanitize_message(message: &str) -> String {
    let mut lines = LinesWithTerminator::new(message).collect::<Vec<_>>();
    lines.retain(|l| !l.starts_with('#'));
    while !lines.is_empty() {
        if lines.first().unwrap().trim().is_empty() {
            lines.remove(0);
        } else {
            break;
        }
    }
    while !lines.is_empty() {
        if lines.last().unwrap().trim().is_empty() {
            lines.pop();
        } else {
            break;
        }
    }
    let message = lines.join("");
    message.trim_end().to_owned()
}

#[derive(Clone, Debug)]
pub(crate) struct LinesWithTerminator<'a> {
    data: &'a str,
}

impl<'a> LinesWithTerminator<'a> {
    pub(crate) fn new(data: &'a str) -> LinesWithTerminator<'a> {
        LinesWithTerminator { data }
    }
}

impl<'a> Iterator for LinesWithTerminator<'a> {
    type Item = &'a str;

    #[inline]
    fn next(&mut self) -> Option<&'a str> {
        match self.data.find('\n') {
            None if self.data.is_empty() => None,
            None => {
                let line = self.data;
                self.data = "";
                Some(line)
            }
            Some(end) => {
                let line = &self.data[..end + 1];
                self.data = &self.data[end + 1..];
                Some(line)
            }
        }
    }
}

#[derive(Copy, Clone, Debug)]
#[non_exhaustive]
pub struct Palette {
    pub error: yansi::Style,
    pub warn: yansi::Style,
    pub info: yansi::Style,
    pub good: yansi::Style,
    pub highlight: yansi::Style,
    pub hint: yansi::Style,
}

impl Palette {
    pub fn colored() -> Self {
        Self {
            error: yansi::Style::new(yansi::Color::Red).bold(),
            warn: yansi::Style::new(yansi::Color::Yellow).bold(),
            info: yansi::Style::new(yansi::Color::Blue).bold(),
            good: yansi::Style::new(yansi::Color::Cyan).bold(),
            highlight: yansi::Style::new(yansi::Color::Green).bold(),
            hint: yansi::Style::new(yansi::Color::Unset).dimmed(),
        }
    }

    pub fn plain() -> Self {
        Self {
            error: yansi::Style::default(),
            warn: yansi::Style::default(),
            info: yansi::Style::default(),
            good: yansi::Style::default(),
            highlight: yansi::Style::default(),
            hint: yansi::Style::default(),
        }
    }
}

pub const STASH_STACK_NAME: &str = "git-stack";
