#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Action {
    Pick,
    Protected,
    Rebase(git2::Oid),
}

impl Action {
    pub fn is_pick(&self) -> bool {
        matches!(self, Action::Pick)
    }

    pub fn is_protected(&self) -> bool {
        matches!(self, Action::Protected)
    }

    pub fn is_rebase(&self) -> bool {
        matches!(self, Action::Rebase(_))
    }
}
