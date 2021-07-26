use std::str::FromStr;

#[derive(Default, Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct RepoConfig {
    pub protected_branches: Option<Vec<String>>,
    pub branch: Option<Branch>,
    pub show_format: Option<Format>,
    pub show_stacked: Option<bool>,
}

static PROTECTED_BRANCH_FIELD: &str = "stack.protected-branch";
static DEFAULT_PROTECTED_BRANCHES: [&str; 4] = ["main", "master", "dev", "stable"];
static BRANCH: &str = "stack.branch";
static FORMAT_FIELD: &str = "stack.show-format";
static STACKED_FIELD: &str = "stack.show-stacked";

impl RepoConfig {
    pub fn from_all(repo: &git2::Repository) -> eyre::Result<Self> {
        let config = Self::from_defaults();
        let config = config.update(Self::from_workdir(repo)?);
        let config = config.update(Self::from_repo(repo)?);
        Ok(config)
    }

    pub fn from_repo(repo: &git2::Repository) -> eyre::Result<Self> {
        let workdir = repo
            .workdir()
            .ok_or_else(|| eyre::eyre!("Cannot read config in bare repository."))?;
        let config_path = workdir.join(".git/config");
        log::trace!("Loading {}", config_path.display());
        if config_path.exists() {
            match git2::Config::open(&config_path) {
                Ok(config) => Ok(Self::from_gitconfig(&config)),
                Err(err) => {
                    log::debug!("Failed to load git config: {}", err);
                    Ok(Default::default())
                }
            }
        } else {
            Ok(Default::default())
        }
    }

    pub fn from_workdir(repo: &git2::Repository) -> eyre::Result<Self> {
        let workdir = repo
            .workdir()
            .ok_or_else(|| eyre::eyre!("Cannot read config in bare repository."))?;
        let config_path = workdir.join(".gitconfig");
        log::trace!("Loading {}", config_path.display());
        if config_path.exists() {
            match git2::Config::open(&config_path) {
                Ok(config) => Ok(Self::from_gitconfig(&config)),
                Err(err) => {
                    log::debug!("Failed to load git config: {}", err);
                    Ok(Default::default())
                }
            }
        } else {
            Ok(Default::default())
        }
    }

    pub fn from_defaults() -> Self {
        let mut protected_branches: Vec<String> = Vec::new();

        log::trace!("Loading gitconfig");
        match git2::Config::open_default() {
            Ok(config) => {
                let default_branch = default_branch(&config);
                let default_branch_ignore = default_branch.to_owned();
                protected_branches.push(default_branch_ignore);
            }
            Err(err) => {
                log::debug!("Failed to load git config: {}", err);
            }
        }
        // Don't bother with removing duplicates if `default_branch` is the same as one of our
        // default protected branches
        protected_branches.extend(DEFAULT_PROTECTED_BRANCHES.iter().map(|s| (*s).to_owned()));

        Self {
            protected_branches: Some(protected_branches),
            branch: Some(Default::default()),
            show_format: Some(Default::default()),
            show_stacked: Some(true),
        }
    }

    pub fn from_gitconfig(config: &git2::Config) -> Self {
        let protected_branches = config
            .multivar(PROTECTED_BRANCH_FIELD, None)
            .map(|entries| {
                let entries_ref = &entries;
                let protected_branches: Vec<_> = entries_ref
                    .flat_map(|e| e.into_iter())
                    .filter_map(|e| e.value().map(|v| v.to_owned()))
                    .collect();
                if protected_branches.is_empty() {
                    None
                } else {
                    Some(protected_branches)
                }
            })
            .unwrap_or(None);

        let branch = config
            .get_str(BRANCH)
            .ok()
            .and_then(|s| FromStr::from_str(s).ok());

        let show_format = config
            .get_str(FORMAT_FIELD)
            .ok()
            .and_then(|s| FromStr::from_str(s).ok());

        let show_stacked = config.get_bool(STACKED_FIELD).ok();

        Self {
            protected_branches,
            branch,
            show_format,
            show_stacked,
        }
    }

    pub fn write_repo(&self, repo: &git2::Repository) -> eyre::Result<()> {
        let workdir = repo
            .workdir()
            .ok_or_else(|| eyre::eyre!("Cannot read config in bare repository."))?;
        let config_path = workdir.join(".git/config");
        log::trace!("Loading {}", config_path.display());
        let mut config = git2::Config::open(&config_path)?;
        log::info!("Writing {}", config_path.display());
        self.to_gitconfig(&mut config)?;
        Ok(())
    }

    pub fn to_gitconfig(&self, config: &mut git2::Config) -> eyre::Result<()> {
        if let Some(protected_branches) = self.protected_branches.as_ref() {
            // Ignore errors if there aren't keys to remove
            let _ = config.remove_multivar(PROTECTED_BRANCH_FIELD, ".*");
            for branch in protected_branches {
                config.set_multivar(PROTECTED_BRANCH_FIELD, "^$", branch)?;
            }
        }
        Ok(())
    }

    pub fn update(mut self, other: Self) -> Self {
        match (&mut self.protected_branches, other.protected_branches) {
            (Some(lhs), Some(rhs)) => lhs.extend(rhs),
            (None, Some(rhs)) => self.protected_branches = Some(rhs),
            (_, _) => (),
        }

        self.branch = other.branch.or(self.branch);
        self.show_format = other.show_format.or(self.show_format);
        self.show_stacked = other.show_stacked.or(self.show_stacked);

        self
    }
}

fn default_branch<'c>(config: &'c git2::Config) -> &'c str {
    config.get_str("init.defaultBranch").ok().unwrap_or("main")
}

arg_enum! {
    #[derive(Debug, Copy, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
    #[serde(rename_all = "kebab-case")]
    pub enum Format {
        Silent,
        Brief,
        Full,
    }
}

impl Default for Format {
    fn default() -> Self {
        Format::Brief
    }
}

arg_enum! {
    #[derive(Debug, Copy, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
    #[serde(rename_all = "kebab-case")]
    pub enum Branch {
        Current,
        Dependents,
        All,
    }
}

impl Default for Branch {
    fn default() -> Self {
        Branch::Current
    }
}
