#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Backup {
    pub branches: Vec<Branch>,
    #[serde(default)]
    #[serde(skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub metadata: std::collections::BTreeMap<String, serde_json::Value>,
}

impl Backup {
    pub fn load(path: &std::path::Path) -> Result<Self, std::io::Error> {
        let file = std::fs::File::open(path)?;
        let reader = std::io::BufReader::new(file);
        let b = serde_json::from_reader(reader)?;
        Ok(b)
    }

    pub fn save(&self, path: &std::path::Path) -> Result<(), std::io::Error> {
        let s = serde_json::to_string_pretty(self)?;
        std::fs::write(path, &s)?;
        Ok(())
    }

    pub fn from_repo(repo: &dyn crate::git::Repo) -> Result<Self, git2::Error> {
        let mut branches: Vec<_> = repo
            .local_branches()
            .map(|b| {
                let commit = repo.find_commit(b.id).unwrap();
                Branch {
                    name: b.name,
                    id: b.id,
                    metadata: maplit::btreemap! {
                        "summary".to_owned() => serde_json::Value::String(
                            String::from_utf8_lossy(commit.summary.as_slice()).into_owned()
                        ),
                    },
                }
            })
            .collect();
        branches.sort_unstable();
        let metadata = Default::default();
        Ok(Self { branches, metadata })
    }

    pub fn apply(&self, repo: &mut dyn crate::git::Repo) -> Result<(), git2::Error> {
        for branch in self.branches.iter() {
            let existing = repo.find_local_branch(&branch.name);
            if existing.map(|b| b.id) == Some(branch.id) {
                log::trace!("No change for {}", branch.name);
            } else {
                log::debug!("Restoring {}", branch.name);
                repo.branch(&branch.name, branch.id)?;
            }
        }
        Ok(())
    }

    pub fn insert_message(&mut self, message: &str) {
        self.metadata.insert(
            "message".to_owned(),
            serde_json::Value::String(message.to_owned()),
        );
    }

    pub fn insert_parent(
        &mut self,
        repo: &dyn crate::git::Repo,
        branches: &crate::git::Branches,
        protected_branches: &crate::git::Branches,
    ) {
        for branch in self.branches.iter_mut() {
            if let Some(parent) = crate::git::find_base(repo, branches, branch.id)
                .or_else(|| crate::git::find_protected_base(repo, protected_branches, branch.id))
            {
                branch.metadata.insert(
                    "parent".to_owned(),
                    serde_json::Value::String(parent.name.clone()),
                );
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Branch {
    pub name: String,
    #[serde(serialize_with = "serialize_oid")]
    #[serde(deserialize_with = "deserialize_oid")]
    pub id: git2::Oid,
    #[serde(default)]
    #[serde(skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub metadata: std::collections::BTreeMap<String, serde_json::Value>,
}

fn serialize_oid<S>(id: &git2::Oid, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    let id = id.to_string();
    serializer.serialize_str(&id)
}

fn deserialize_oid<'de, D>(deserializer: D) -> Result<git2::Oid, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize;
    let s = String::deserialize(deserializer)?;
    git2::Oid::from_str(&s).map_err(serde::de::Error::custom)
}

impl PartialOrd for Branch {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some((&self.name, self.id).cmp(&(&other.name, other.id)))
    }
}

impl Ord for Branch {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (&self.name, self.id).cmp(&(&other.name, other.id))
    }
}
