#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum Action {
    #[default]
    Pick,
    Fixup,
    Protected,
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
}

impl crate::any::ResourceTag for Action {}
