#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Action {
    Pick,
    Protected,
    Rebase(git2::Oid),
}
