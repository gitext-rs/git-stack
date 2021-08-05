use std::str::FromStr;

#[derive(Default, Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct RepoConfig {
    pub protected_branches: Option<Vec<String>>,
    pub stack: Option<Stack>,
    pub push_remote: Option<String>,
    pub pull_remote: Option<String>,
    pub show_format: Option<Format>,
    pub show_stacked: Option<bool>,
}

static PROTECTED_STACK_FIELD: &str = "stack.protected-branch";
static DEFAULT_PROTECTED_BRANCHES: [&str; 4] = ["main", "master", "dev", "stable"];
static STACK_FIELD: &str = "stack.stack";
static PUSH_REMOTE_FIELD: &str = "stack.push-remote";
static PULL_REMOTE_FIELD: &str = "stack.pull-remote";
static FORMAT_FIELD: &str = "stack.show-format";
static STACKED_FIELD: &str = "stack.show-stacked";

impl RepoConfig {
    pub fn from_all(repo: &git2::Repository) -> eyre::Result<Self> {
        log::trace!("Loading gitconfig");
        let default_config = match git2::Config::open_default() {
            Ok(config) => Some(config),
            Err(err) => {
                log::debug!("Failed to load git config: {}", err);
                None
            }
        };
        let config = Self::from_defaults_internal(default_config.as_ref());
        let config = if let Some(default_config) = default_config.as_ref() {
            config.update(Self::from_gitconfig(default_config))
        } else {
            config
        };
        let config = config.update(Self::from_workdir(repo)?);
        let config = config.update(Self::from_repo(repo)?);
        let config = config.update(Self::from_env());
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

    pub fn from_env() -> Self {
        let mut config = Self::default();

        let params = git_config_env::ConfigParameters::new();
        config = config.update(Self::from_env_iter(params.iter()));

        let params = git_config_env::ConfigEnv::new();
        config = config.update(Self::from_env_iter(
            params.iter().map(|(k, v)| (k, Some(v))),
        ));

        config
    }

    fn from_env_iter<'s>(
        iter: impl Iterator<Item = (std::borrow::Cow<'s, str>, Option<std::borrow::Cow<'s, str>>)>,
    ) -> Self {
        let mut config = Self::default();

        for (key, value) in iter {
            log::trace!("Env config: {}={:?}", key, value);
            if key == PROTECTED_STACK_FIELD {
                if let Some(value) = value {
                    config
                        .protected_branches
                        .get_or_insert_with(Vec::new)
                        .push(value.into_owned());
                }
            } else if key == STACK_FIELD {
                if let Some(value) = value.as_ref().and_then(|v| FromStr::from_str(v).ok()) {
                    config.stack = Some(value);
                }
            } else if key == PUSH_REMOTE_FIELD {
                if let Some(value) = value {
                    config.push_remote = Some(value.into_owned());
                }
            } else if key == PULL_REMOTE_FIELD {
                if let Some(value) = value {
                    config.pull_remote = Some(value.into_owned());
                }
            } else if key == FORMAT_FIELD {
                if let Some(value) = value.as_ref().and_then(|v| FromStr::from_str(v).ok()) {
                    config.show_format = Some(value);
                }
            } else if key == STACKED_FIELD {
                config.show_stacked = Some(value.as_ref().map(|v| v == "true").unwrap_or(true));
            } else {
                log::warn!(
                    "Unsupported config: {}={}",
                    key,
                    value.as_deref().unwrap_or("")
                );
            }
        }

        config
    }

    pub fn from_defaults() -> Self {
        log::trace!("Loading gitconfig");
        let config = match git2::Config::open_default() {
            Ok(config) => Some(config),
            Err(err) => {
                log::debug!("Failed to load git config: {}", err);
                None
            }
        };
        Self::from_defaults_internal(config.as_ref())
    }

    fn from_defaults_internal(config: Option<&git2::Config>) -> Self {
        let mut conf = Self::default();
        conf.stack = Some(conf.stack());
        conf.push_remote = Some(conf.push_remote().to_owned());
        conf.pull_remote = Some(conf.pull_remote().to_owned());
        conf.show_format = Some(conf.show_format());
        conf.show_stacked = Some(conf.show_stacked());

        let mut protected_branches: Vec<String> = Vec::new();

        if let Some(config) = config {
            let default_branch = default_branch(config);
            let default_branch_ignore = default_branch.to_owned();
            protected_branches.push(default_branch_ignore);
        }
        // Don't bother with removing duplicates if `default_branch` is the same as one of our
        // default protected branches
        protected_branches.extend(DEFAULT_PROTECTED_BRANCHES.iter().map(|s| (*s).to_owned()));
        conf.protected_branches = Some(protected_branches);

        conf
    }

    pub fn from_gitconfig(config: &git2::Config) -> Self {
        let protected_branches = config
            .multivar(PROTECTED_STACK_FIELD, None)
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

        let push_remote = config.get_string(PUSH_REMOTE_FIELD).ok();
        let pull_remote = config.get_string(PULL_REMOTE_FIELD).ok();

        let stack = config
            .get_str(STACK_FIELD)
            .ok()
            .and_then(|s| FromStr::from_str(s).ok());

        let show_format = config
            .get_str(FORMAT_FIELD)
            .ok()
            .and_then(|s| FromStr::from_str(s).ok());

        let show_stacked = config.get_bool(STACKED_FIELD).ok();

        Self {
            protected_branches,
            push_remote,
            pull_remote,
            stack,
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
            let _ = config.remove_multivar(PROTECTED_STACK_FIELD, ".*");
            for branch in protected_branches {
                config.set_multivar(PROTECTED_STACK_FIELD, "^$", branch)?;
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

        self.push_remote = other.push_remote.or(self.push_remote);
        self.pull_remote = other.pull_remote.or(self.pull_remote);
        self.stack = other.stack.or(self.stack);
        self.show_format = other.show_format.or(self.show_format);
        self.show_stacked = other.show_stacked.or(self.show_stacked);

        self
    }

    pub fn protected_branches(&self) -> &[String] {
        self.protected_branches.as_deref().unwrap_or(&[])
    }

    pub fn push_remote(&self) -> &str {
        self.push_remote.as_deref().unwrap_or("origin")
    }

    pub fn pull_remote(&self) -> &str {
        self.pull_remote
            .as_deref()
            .unwrap_or_else(|| self.push_remote())
    }

    pub fn stack(&self) -> Stack {
        self.stack.unwrap_or_else(Default::default)
    }

    pub fn show_format(&self) -> Format {
        self.show_format.unwrap_or_else(Default::default)
    }

    pub fn show_stacked(&self) -> bool {
        self.show_stacked.unwrap_or(true)
    }
}

fn default_branch(config: &git2::Config) -> &str {
    config.get_str("init.defaultStack").ok().unwrap_or("main")
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
    pub enum Stack {
        Current,
        Dependents,
        Descendants,
        All,
    }
}

impl Default for Stack {
    fn default() -> Self {
        Stack::All
    }
}
