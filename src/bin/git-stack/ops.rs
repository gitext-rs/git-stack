use bstr::ByteSlice;
use eyre::WrapErr;

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

pub fn resolve_base_from_onto(repo: &git_stack::git::GitRepo, onto: &AnnotatedOid) -> AnnotatedOid {
    // HACK: Assuming the local branch is the current base for all the commits
    onto.branch
        .as_ref()
        .filter(|b| b.remote.is_some())
        .and_then(|b| repo.find_local_branch(&b.name))
        .map(AnnotatedOid::with_branch)
        .unwrap_or_else(|| onto.clone())
}

pub fn git_prune_development(
    repo: &mut git_stack::git::GitRepo,
    branches: &[&str],
    dry_run: bool,
) -> eyre::Result<()> {
    if branches.is_empty() {
        return Ok(());
    }

    let remote = repo.push_remote();
    let output = std::process::Command::new("git")
        .arg("ls-remote")
        .arg("--heads")
        .arg(remote)
        .args(branches)
        .stdout(std::process::Stdio::piped())
        .spawn()
        .wrap_err("Could not run `git fetch`")?
        .wait_with_output()?;
    if !output.status.success() {
        eyre::bail!("Could not run `git fetch`");
    }
    let stdout = String::from_utf8(output.stdout).wrap_err("Could not run `git fetch`")?;
    #[allow(clippy::needless_collect)]
    let remote_branches: Vec<_> = stdout
        .lines()
        .filter_map(|l| l.split_once('\t').map(|s| s.1))
        .filter_map(|l| l.strip_prefix("refs/heads/"))
        .collect();

    for branch in branches {
        if !remote_branches.contains(branch) {
            let remote_branch = format!("{}/{}", remote, branch);
            log::info!("Pruning {}", remote_branch);
            if !dry_run {
                let mut branch = repo
                    .raw()
                    .find_branch(&remote_branch, git2::BranchType::Remote)?;
                branch.delete()?;
            }
        }
    }

    Ok(())
}

pub fn git_fetch_upstream(remote: &str, branch_name: &str) -> eyre::Result<()> {
    log::debug!("git fetch {} {}", remote, branch_name);
    // A little uncertain about some of the weirder authentication needs, just deferring to `git`
    // instead of using `libgit2`
    let status = std::process::Command::new("git")
        .arg("fetch")
        .arg(remote)
        .arg(branch_name)
        .status()
        .wrap_err("Could not run `git fetch`")?;
    if !status.success() {
        eyre::bail!("`git fetch {} {}` failed", remote, branch_name,);
    }

    Ok(())
}

/// Switch to the best-guess branch
///
/// # Panic
///
/// Panics if `current_id` is not present
pub fn switch(
    repo: &mut git_stack::git::GitRepo,
    branches: &git_stack::graph::BranchSet,
    current_id: git2::Oid,
    stderr_palette: Palette,
    dry_run: bool,
) -> Result<(), git2::Error> {
    use std::io::Write;

    let current_commit = repo
        .find_commit(current_id)
        .expect("children/head are always present");
    if let Some(current) = branches.get(current_id) {
        let mut current = current.to_owned();
        current.sort_by_key(|b| b.kind());
        let current_branch = current.first().expect("always at least one");
        let _ = writeln!(
            std::io::stderr(),
            "{} to {}: {}",
            stderr_palette.good.paint("Switching"),
            stderr_palette
                .highlight
                .paint(current_branch.display_name()),
            stderr_palette.hint.paint(&current_commit.summary)
        );
        if !dry_run {
            repo.switch_branch(
                current_branch
                    .local_name()
                    .expect("only local branches present"),
            )?;
        }
    } else {
        let abbrev_id = repo
            .raw()
            .find_object(current_id, None)
            .unwrap_or_else(|e| panic!("Unexpected git2 error: {}", e))
            .short_id()
            .unwrap_or_else(|e| panic!("Unexpected git2 error: {}", e));
        let _ = writeln!(
            std::io::stderr(),
            "{} to {}: {}",
            stderr_palette.good.paint("Switching"),
            stderr_palette.highlight.paint(abbrev_id.as_str().unwrap()),
            stderr_palette.hint.paint(&current_commit.summary)
        );
        if !dry_run {
            repo.switch_commit(current_id)?;
        }
    }

    Ok(())
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
