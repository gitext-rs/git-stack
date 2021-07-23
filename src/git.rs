use itertools::Itertools;

pub static NO_BRANCH: &str = "<>";

pub fn default_branch<'c>(config: &'c git2::Config) -> &'c str {
    config.get_str("init.defaultBranch").ok().unwrap_or("main")
}

pub fn commits_from<'r>(
    repo: &'r git2::Repository,
    head_oid: git2::Oid,
) -> Result<impl Iterator<Item = git2::Commit<'r>>, git2::Error> {
    let mut revwalk = repo.revwalk()?;
    revwalk.push(head_oid)?;

    let commits = revwalk
        .filter_map(Result::ok)
        .filter_map(move |oid| repo.find_commit(oid).ok());
    Ok(commits)
}

pub fn resolve_branch<'r>(
    repo: &'r git2::Repository,
    name: &str,
) -> Result<git2::Branch<'r>, git2::Error> {
    repo.find_branch(name, git2::BranchType::Local)
}

pub fn resolve_head_branch<'r>(
    repo: &'r git2::Repository,
) -> Result<git2::Branch<'r>, git2::Error> {
    let reference = repo.head()?;
    let name = reference.shorthand().ok_or_else(|| {
        git2::Error::new(
            git2::ErrorCode::NotFound,
            git2::ErrorClass::Reference,
            "HEAD must point to a branch",
        )
    })?;
    let branch = resolve_branch(repo, name)?;
    Ok(branch)
}

pub fn resolve_name(repo: &git2::Repository, name: &str) -> Result<git2::Oid, git2::Error> {
    let oid = repo.revparse_single(name)?.id();
    Ok(oid)
}

pub fn head_oid(repo: &git2::Repository) -> Result<git2::Oid, git2::Error> {
    let oid = repo.head()?.resolve()?.target().ok_or_else(|| {
        git2::Error::new(
            git2::ErrorCode::NotFound,
            git2::ErrorClass::Reference,
            "could not find HEAD",
        )
    })?;
    Ok(oid)
}

pub fn is_dirty(repo: &git2::Repository) -> Result<bool, git2::Error> {
    if repo.state() != git2::RepositoryState::Clean {
        return Ok(false);
    }

    let status = repo.statuses(Some(git2::StatusOptions::new().include_ignored(false)))?;
    if status.is_empty() {
        Ok(false)
    } else {
        log::trace!(
            "Repository is dirty: {}",
            status
                .iter()
                .flat_map(|s| s.path().map(|s| s.to_owned()))
                .join(", ")
        );
        Ok(true)
    }
}

pub fn reorder_fixup<'c, 'r>(commits: &'c [git2::Commit<'r>]) -> Vec<&'c git2::Commit<'r>> {
    let summaries: std::collections::HashMap<_, _> = commits
        .into_iter()
        .map(|c| (c.summary().unwrap_or(""), c.id()))
        .collect();

    let mut bases = Vec::new();
    let mut fixes = std::collections::HashMap::new();
    let mut unowned = Vec::new();
    for commit in commits.into_iter().rev() {
        if let Some(target_summary) = get_fixup_target_summary(commit.summary().unwrap_or("")) {
            if let Some(target_oid) = summaries.get(target_summary) {
                fixes
                    .entry(*target_oid)
                    .or_insert_with(|| Vec::new())
                    .push(commit);
            } else {
                unowned.push(commit);
            }
        } else {
            bases.push(commit.id());
            fixes
                .entry(commit.id())
                .or_insert_with(|| Vec::new())
                .push(commit);
        }
    }

    unowned
        .into_iter()
        .chain(
            bases
                .into_iter()
                .flat_map(|oid| fixes.get(&oid).unwrap().into_iter().map(|c| *c)),
        )
        .collect()
}

pub fn get_fixup_target_summary(summary: &str) -> Option<&str> {
    summary.strip_prefix("fixup! ")
}
