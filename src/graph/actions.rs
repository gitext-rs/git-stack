#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Action {
    Pick,
    Squash,
    Protected,
    Delete,
}

impl Action {
    pub fn is_pick(&self) -> bool {
        matches!(self, Action::Pick)
    }

    pub fn is_squash(&self) -> bool {
        matches!(self, Action::Squash)
    }

    pub fn is_protected(&self) -> bool {
        matches!(self, Action::Protected)
    }

    pub fn is_delete(&self) -> bool {
        matches!(self, Action::Delete)
    }
}
