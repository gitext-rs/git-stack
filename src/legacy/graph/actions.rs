#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Action {
    Pick,
    Fixup,
    Protected,
    Delete,
}

impl Action {
    pub fn is_pick(&self) -> bool {
        matches!(self, Self::Pick)
    }

    pub fn is_fixup(&self) -> bool {
        matches!(self, Self::Fixup)
    }

    pub fn is_protected(&self) -> bool {
        matches!(self, Self::Protected)
    }

    pub fn is_delete(&self) -> bool {
        matches!(self, Self::Delete)
    }
}
