#[derive(Clone, Debug)]
pub struct ProtectedBranches {
    ignores: ignore::gitignore::Gitignore,
}

impl ProtectedBranches {
    pub fn new<'p>(patterns: impl IntoIterator<Item = &'p str>) -> eyre::Result<Self> {
        let mut ignores = ignore::gitignore::GitignoreBuilder::new("");
        for pattern in patterns {
            ignores.add_line(None, pattern)?;
        }
        let ignores = ignores.build()?;
        Ok(Self { ignores })
    }

    pub fn is_protected(&self, name: &str) -> bool {
        let name_match = self.ignores.matched_path_or_any_parents(name, false);
        match name_match {
            ignore::Match::None => false,
            ignore::Match::Ignore(glob) => {
                log::trace!("`{}` is ignored by {:?}", name, glob.original());
                true
            }
            ignore::Match::Whitelist(glob) => {
                log::trace!("`{}` is allowed by {:?}", name, glob.original());
                false
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn empty_allows_all() {
        let protect = ProtectedBranches::new(None).unwrap();
        assert!(!protect.is_protected("main"));
    }

    #[test]
    fn protect_branch() {
        let protect = ProtectedBranches::new(Some("main")).unwrap();
        assert!(protect.is_protected("main"));
        assert!(!protect.is_protected("feature"));
    }

    #[test]
    fn negation_patterns() {
        let protect = ProtectedBranches::new(vec!["v*", "!very"]).unwrap();
        assert!(protect.is_protected("v1.0.0"));
        assert!(!protect.is_protected("very"));
        assert!(!protect.is_protected("feature"));
    }

    #[test]
    fn folders() {
        let protect = ProtectedBranches::new(vec!["release/"]).unwrap();
        assert!(!protect.is_protected("release"));
        assert!(protect.is_protected("release/v1.0.0"));
        assert!(!protect.is_protected("feature"));
    }
}
