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
