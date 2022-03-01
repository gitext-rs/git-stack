pub use super::Snapshot;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stack {
    pub name: String,
    root: std::path::PathBuf,
    capacity: Option<usize>,
}

impl Stack {
    pub const DEFAULT_STACK: &'static str = "recent";
    const EXT: &'static str = "bak";

    pub fn new(name: &str, repo: &crate::git::GitRepo) -> Self {
        let root = stack_root(repo.raw().path(), name);
        let name = name.to_owned();
        Self {
            name,
            root,
            capacity: None,
        }
    }

    pub fn all(repo: &crate::git::GitRepo) -> impl Iterator<Item = Self> {
        let root = stacks_root(repo.raw().path());
        let mut stacks: Vec<_> = std::fs::read_dir(root)
            .into_iter()
            .flatten()
            .filter_map(|e| {
                let e = e.ok()?;
                let e = e.file_type().ok()?.is_dir().then(|| e)?;
                let p = e.path();
                let stack_name = p.file_name()?.to_str()?.to_owned();
                let stack_root = stack_root(repo.raw().path(), &stack_name);
                Some(Self {
                    name: stack_name,
                    root: stack_root,
                    capacity: None,
                })
            })
            .collect();
        if !stacks.iter().any(|v| v.name == Self::DEFAULT_STACK) {
            stacks.insert(0, Self::new(Self::DEFAULT_STACK, repo));
        }
        stacks.into_iter()
    }

    pub fn capacity(&mut self, capacity: Option<usize>) {
        self.capacity = capacity;
    }

    pub fn iter(&self) -> impl DoubleEndedIterator<Item = std::path::PathBuf> {
        let mut elements: Vec<(usize, std::path::PathBuf)> = std::fs::read_dir(&self.root)
            .into_iter()
            .flatten()
            .filter_map(|e| {
                let e = e.ok()?;
                let e = e.file_type().ok()?.is_file().then(|| e)?;
                let p = e.path();
                let p = (p.extension()? == Self::EXT).then(|| p)?;
                let index = p.file_stem()?.to_str()?.parse::<usize>().ok()?;
                Some((index, p))
            })
            .collect();
        elements.sort_unstable();
        elements.into_iter().map(|(_, p)| p)
    }

    pub fn push(&mut self, snapshot: Snapshot) -> Result<std::path::PathBuf, std::io::Error> {
        let elems: Vec<_> = self.iter().collect();
        let last_path = elems.iter().last();
        let next_index = match last_path {
            Some(last_path) => {
                let current_index = last_path
                    .file_stem()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .parse::<usize>()
                    .unwrap();
                current_index + 1
            }
            None => 0,
        };
        let last = last_path.and_then(|p| Snapshot::load(p).ok());
        if last.as_ref() == Some(&snapshot) {
            let last_path = last_path.unwrap().to_owned();
            log::trace!("Reusing snapshot {}", last_path.display());
            return Ok(last_path);
        }

        std::fs::create_dir_all(&self.root)?;
        let new_path = self.root.join(format!("{}.{}", next_index, Self::EXT));
        snapshot.save(&new_path)?;
        log::trace!("Backed up as {}", new_path.display());

        if let Some(capacity) = self.capacity {
            let len = elems.len();
            if capacity < len {
                let remove = len - capacity;
                log::warn!("Too many snapshots, clearing {} oldest", remove);
                for snapshot_path in &elems[0..remove] {
                    if let Err(err) = std::fs::remove_file(&snapshot_path) {
                        log::trace!("Failed to remove {}: {}", snapshot_path.display(), err);
                    } else {
                        log::trace!("Removed {}", snapshot_path.display());
                    }
                }
            }
        }

        Ok(new_path)
    }

    pub fn clear(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }

    pub fn pop(&mut self) -> Option<std::path::PathBuf> {
        let mut elems: Vec<_> = self.iter().collect();
        let last = elems.pop()?;
        std::fs::remove_file(&last).ok()?;
        Some(last)
    }

    pub fn peek(&mut self) -> Option<std::path::PathBuf> {
        self.iter().last()
    }
}

fn stacks_root(repo: &std::path::Path) -> std::path::PathBuf {
    repo.join("branch-stash")
}

fn stack_root(repo: &std::path::Path, stack: &str) -> std::path::PathBuf {
    repo.join("branch-stash").join(stack)
}
