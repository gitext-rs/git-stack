#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Action {
    Pick,
    Fixup,
    Protected,
}

impl Action {
    pub fn is_pick(&self) -> bool {
        matches!(self, Action::Pick)
    }

    pub fn is_fixup(&self) -> bool {
        matches!(self, Action::Fixup)
    }

    pub fn is_protected(&self) -> bool {
        matches!(self, Action::Protected)
    }
}

impl Default for Action {
    fn default() -> Self {
        Self::Pick
    }
}

impl crate::any::ResourceTag for Action {}
