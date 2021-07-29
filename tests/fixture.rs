use bstr::ByteSlice;

pub fn populate_repo(repo: &mut git_stack::git::InMemoryRepo, fixture: git_fixture::Dag) {
    if fixture.init {
        repo.clear();
    }

    let import_root = fixture.import_root;
    let mut marks: std::collections::HashMap<String, git2::Oid> = Default::default();
    for event in fixture.events.into_iter() {
        populate_event(repo, event, &import_root, &mut marks);
    }
}

fn populate_event(
    repo: &mut git_stack::git::InMemoryRepo,
    event: git_fixture::Event,
    import_root: &std::path::Path,
    marks: &mut std::collections::HashMap<String, git2::Oid>,
) {
    match event {
        git_fixture::Event::Import(path) => {
            let path = import_root.join(path);
            let mut child_dag = git_fixture::Dag::load(&path).unwrap();
            child_dag.init = false;
            populate_repo(repo, child_dag);
        }
        git_fixture::Event::Tree(tree) => {
            if tree.state.is_committed() {
                let parent_id = repo.head_id();
                let commit_id = repo.gen_id();
                let message = bstr::BString::from(tree.message.as_deref().unwrap_or("Automated"));
                let summary = message.lines().next().unwrap().to_owned();
                let commit = git_stack::git::Commit {
                    id: commit_id,
                    summary: bstr::BString::from(summary),
                    is_merge: false,
                };
                repo.push_commit(parent_id, commit);

                if let Some(branch) = tree.branch.as_ref() {
                    let branch = git_stack::git::Branch {
                        name: branch.as_str().to_owned(),
                        id: commit_id,
                    };
                    repo.mark_branch(branch);
                }

                if let Some(mark) = tree.mark.as_ref() {
                    marks.insert(mark.as_str().to_owned(), commit_id);
                }
            }
        }
        git_fixture::Event::Children(mut events) => {
            let start_commit = repo.head_id().unwrap();
            let last_run = events.pop();
            for run in events {
                for event in run {
                    populate_event(repo, event, import_root, marks);
                }
                repo.set_head(start_commit);
            }
            if let Some(last_run) = last_run {
                for event in last_run {
                    populate_event(repo, event, import_root, marks);
                }
            }
        }
        git_fixture::Event::Head(reference) => {
            let id = match reference {
                git_fixture::Reference::Mark(mark) => *marks.get(mark.as_str()).unwrap(),
                git_fixture::Reference::Branch(name) => {
                    repo.find_local_branch(name.as_str()).unwrap().id
                }
            };
            repo.set_head(id);
        }
    }
}
