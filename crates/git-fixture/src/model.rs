#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
#[serde(deny_unknown_fields)]
pub struct Dag {
    pub init: bool,
    pub events: Vec<Event>,
    #[serde(skip)]
    pub import_root: std::path::PathBuf,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, derive_more::IsVariant)]
#[serde(rename_all = "snake_case")]
#[serde(deny_unknown_fields)]
pub enum Event {
    Import(std::path::PathBuf),
    Tree(Tree),
    Children(Vec<Vec<Event>>),
    Head(Reference),
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
#[serde(deny_unknown_fields)]
pub struct Tree {
    #[serde(default)]
    pub tracked: std::collections::HashMap<std::path::PathBuf, bstr::BString>,
    #[serde(default)]
    pub state: TreeState,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub branch: Option<Branch>,
    #[serde(default)]
    pub mark: Option<Mark>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
#[serde(deny_unknown_fields)]
pub struct Merge {
    pub base: Vec<Reference>,
    #[serde(default)]
    pub branch: Option<Branch>,
    #[serde(default)]
    pub mark: Option<Mark>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, derive_more::IsVariant)]
#[serde(rename_all = "snake_case")]
#[serde(deny_unknown_fields)]
pub enum TreeState {
    Committed,
    Staged,
    Tracked,
}

impl Default for TreeState {
    fn default() -> Self {
        Self::Committed
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, derive_more::IsVariant)]
#[serde(rename_all = "snake_case")]
#[serde(deny_unknown_fields)]
pub enum Reference {
    Branch(Branch),
    Mark(Mark),
}

#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
#[serde(transparent)]
pub struct Mark(String);

impl Mark {
    pub fn new(name: &str) -> Self {
        Self(name.to_owned())
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl From<String> for Mark {
    fn from(other: String) -> Self {
        Self(other)
    }
}

impl<'s> From<&'s str> for Mark {
    fn from(other: &'s str) -> Self {
        Self(other.to_owned())
    }
}

impl std::ops::Deref for Mark {
    type Target = str;

    #[inline]
    fn deref(&self) -> &str {
        self.as_str()
    }
}

impl std::borrow::Borrow<str> for Mark {
    #[inline]
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
#[serde(transparent)]
pub struct Branch(String);

impl Branch {
    pub fn new(name: &str) -> Self {
        Self(name.to_owned())
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl From<String> for Branch {
    fn from(other: String) -> Self {
        Self(other)
    }
}

impl<'s> From<&'s str> for Branch {
    fn from(other: &'s str) -> Self {
        Self(other.to_owned())
    }
}

impl std::ops::Deref for Branch {
    type Target = str;

    #[inline]
    fn deref(&self) -> &str {
        self.as_str()
    }
}

impl std::borrow::Borrow<str> for Branch {
    #[inline]
    fn borrow(&self) -> &str {
        self.as_str()
    }
}
