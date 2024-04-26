use bstr::ByteSlice;

pub(crate) fn populate_repo(repo: &mut git_stack::git::InMemoryRepo, fixture: git_fixture::TodoList) {
    if fixture.init {
        repo.clear();
    }

    let mut last_oid = None;
    let mut labels: std::collections::HashMap<git_fixture::Label, git2::Oid> = Default::default();
    for command in fixture.commands {
        match command {
            git_fixture::Command::Label(label) => {
                let current_oid = last_oid.unwrap();
                labels.insert(label.clone(), current_oid);
            }
            git_fixture::Command::Reset(label) => {
                let current_oid = *labels.get(label.as_str()).unwrap();
                last_oid = Some(current_oid);
            }
            git_fixture::Command::Tree(tree) => {
                let parent_id = last_oid;
                let commit_id = repo.gen_id();
                let message = bstr::BString::from(tree.message.as_deref().unwrap_or("Automated"));
                let summary = message.lines().next().unwrap().to_owned();
                let commit = git_stack::git::Commit {
                    id: commit_id,
                    tree_id: commit_id,
                    summary: bstr::BString::from(summary),
                    time: std::time::SystemTime::now(),
                    author: Some(std::rc::Rc::from(
                        tree.author.as_deref().unwrap_or("fixture"),
                    )),
                    committer: Some(std::rc::Rc::from(
                        tree.author.as_deref().unwrap_or("fixture"),
                    )),
                };
                repo.push_commit(parent_id, commit);
                last_oid = Some(commit_id);
            }
            git_fixture::Command::Merge(_) => {
                unimplemented!("merges aren't handled atm");
            }
            git_fixture::Command::Branch(branch) => {
                let current_oid = last_oid.unwrap();
                let branch = git_stack::git::Branch {
                    remote: None,
                    name: branch.as_str().to_owned(),
                    id: current_oid,
                };
                repo.mark_branch(branch);
            }
            git_fixture::Command::Tag(_) => {
                unimplemented!("tags aren't handled atm");
            }
            git_fixture::Command::Head => {
                let current_oid = last_oid.unwrap();
                repo.set_head(current_oid);
            }
        }
    }
}
