use std::str::FromStr;

#[derive(Default, Clone, Debug)]
pub struct RepoConfig {
    pub editor: Option<String>,

    pub protected_branches: Option<Vec<String>>,
    pub protect_commit_count: Option<usize>,
    pub protect_commit_age: Option<std::time::Duration>,
    pub auto_base_commit_count: Option<usize>,
    pub stack: Option<Stack>,
    pub push_remote: Option<String>,
    pub pull_remote: Option<String>,
    pub show_format: Option<Format>,
    pub show_commits: Option<ShowCommits>,
    pub show_stacked: Option<bool>,
    pub auto_fixup: Option<Fixup>,
    pub auto_repair: Option<bool>,

    pub capacity: Option<usize>,
}

static CORE_EDITOR: &str = "core.editor";
static PROTECTED_STACK_FIELD: &str = "stack.protected-branch";
static PROTECT_COMMIT_COUNT: &str = "stack.protect-commit-count";
static PROTECT_COMMIT_AGE: &str = "stack.protect-commit-age";
static AUTO_BASE_COMMIT_COUNT: &str = "stack.auto-base-commit-count";
static STACK_FIELD: &str = "stack.stack";
static PUSH_REMOTE_FIELD: &str = "stack.push-remote";
static PULL_REMOTE_FIELD: &str = "stack.pull-remote";
static FORMAT_FIELD: &str = "stack.show-format";
static SHOW_COMMITS_FIELD: &str = "stack.show-commits";
static STACKED_FIELD: &str = "stack.show-stacked";
static AUTO_FIXUP_FIELD: &str = "stack.auto-fixup";
static AUTO_REPAIR_FIELD: &str = "stack.auto-repair";
static BACKUP_CAPACITY_FIELD: &str = "branch-stash.capacity";

#[cfg(windows)]
static DEFAULT_CORE_EDITOR: &str = "notepad.exe";
#[cfg(not(windows))]
static DEFAULT_CORE_EDITOR: &str = "vi";
static DEFAULT_PROTECTED_BRANCHES: [&str; 4] = ["main", "master", "dev", "stable"];
static DEFAULT_PROTECT_COMMIT_COUNT: usize = 50;
static DEFAULT_PROTECT_COMMIT_AGE: std::time::Duration =
    std::time::Duration::from_secs(60 * 60 * 24 * 14);
static DEFAULT_AUTO_BASE_COMMIT_COUNT: usize = 500;
const DEFAULT_CAPACITY: usize = 30;

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
        let config_path = git_dir_config(repo);
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

        config.editor = std::env::var("GIT_EDITOR").ok();

        config
    }

    fn from_env_iter<'s>(
        iter: impl Iterator<Item = (std::borrow::Cow<'s, str>, Option<std::borrow::Cow<'s, str>>)>,
    ) -> Self {
        let mut config = Self::default();

        for (key, value) in iter {
            log::trace!("Env config: {}={:?}", key, value);
            if key == CORE_EDITOR {
                if let Some(value) = value {
                    config.editor = Some(value.into_owned());
                }
            } else if key == PROTECTED_STACK_FIELD {
                if let Some(value) = value {
                    config
                        .protected_branches
                        .get_or_insert_with(Vec::new)
                        .push(value.into_owned());
                }
            } else if key == PROTECT_COMMIT_COUNT {
                if let Some(value) = value.as_ref().and_then(|v| FromStr::from_str(v).ok()) {
                    config.protect_commit_count = Some(value);
                }
            } else if key == PROTECT_COMMIT_AGE {
                if let Some(value) = value
                    .as_ref()
                    .and_then(|v| humantime::parse_duration(v).ok())
                {
                    config.protect_commit_age = Some(value);
                }
            } else if key == AUTO_BASE_COMMIT_COUNT {
                if let Some(value) = value.as_ref().and_then(|v| FromStr::from_str(v).ok()) {
                    config.auto_base_commit_count = Some(value);
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
            } else if key == SHOW_COMMITS_FIELD {
                if let Some(value) = value.as_ref().and_then(|v| FromStr::from_str(v).ok()) {
                    config.show_commits = Some(value);
                }
            } else if key == STACKED_FIELD {
                config.show_stacked = Some(value.as_ref().map(|v| v == "true").unwrap_or(true));
            } else if key == AUTO_FIXUP_FIELD {
                if let Some(value) = value.as_ref().and_then(|v| FromStr::from_str(v).ok()) {
                    config.auto_fixup = Some(value);
                }
            } else if key == AUTO_REPAIR_FIELD {
                config.auto_repair = Some(value.as_ref().map(|v| v == "true").unwrap_or(true));
            } else if key == BACKUP_CAPACITY_FIELD {
                config.capacity = value.as_deref().and_then(|s| s.parse::<usize>().ok());
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
        conf.editor = std::env::var("VISUAL")
            .or_else(|_err| std::env::var("EDITOR"))
            .ok();
        conf.protect_commit_count = Some(conf.protect_commit_count().unwrap_or(0));
        conf.protect_commit_age = Some(conf.protect_commit_age());
        conf.auto_base_commit_count = Some(conf.auto_base_commit_count().unwrap_or(0));
        conf.stack = Some(conf.stack());
        conf.push_remote = Some(conf.push_remote().to_owned());
        conf.pull_remote = Some(conf.pull_remote().to_owned());
        conf.show_format = Some(conf.show_format());
        conf.show_commits = Some(conf.show_commits());
        conf.show_stacked = Some(conf.show_stacked());
        conf.auto_fixup = Some(conf.auto_fixup());
        conf.capacity = Some(DEFAULT_CAPACITY);

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
        let editor = config.get_string(CORE_EDITOR).ok();

        let protected_branches = config
            .multivar(PROTECTED_STACK_FIELD, None)
            .map(|entries| {
                let mut protected_branches = Vec::new();
                entries
                    .for_each(|entry| {
                        if let Some(value) = entry.value() {
                            protected_branches.push(value.to_owned());
                        }
                    })
                    .unwrap();
                if protected_branches.is_empty() {
                    None
                } else {
                    Some(protected_branches)
                }
            })
            .unwrap_or(None);

        let protect_commit_count = config
            .get_i64(PROTECT_COMMIT_COUNT)
            .ok()
            .map(|i| i.max(0) as usize);
        let protect_commit_age = config
            .get_string(PROTECT_COMMIT_AGE)
            .ok()
            .and_then(|s| humantime::parse_duration(&s).ok());

        let auto_base_commit_count = config
            .get_i64(AUTO_BASE_COMMIT_COUNT)
            .ok()
            .map(|i| i.max(0) as usize);

        let push_remote = config
            .get_string(PUSH_REMOTE_FIELD)
            .ok()
            .or_else(|| config.get_string("remote.pushDefault").ok());
        let pull_remote = config.get_string(PULL_REMOTE_FIELD).ok();

        let stack = config
            .get_string(STACK_FIELD)
            .ok()
            .and_then(|s| FromStr::from_str(&s).ok());

        let show_format = config
            .get_string(FORMAT_FIELD)
            .ok()
            .and_then(|s| FromStr::from_str(&s).ok());

        let show_commits = config
            .get_string(SHOW_COMMITS_FIELD)
            .ok()
            .and_then(|s| FromStr::from_str(&s).ok());

        let show_stacked = config.get_bool(STACKED_FIELD).ok();

        let auto_fixup = config
            .get_string(AUTO_FIXUP_FIELD)
            .ok()
            .and_then(|s| FromStr::from_str(&s).ok());

        let auto_repair = config.get_bool(AUTO_REPAIR_FIELD).ok();

        let capacity = config
            .get_i64(BACKUP_CAPACITY_FIELD)
            .map(|i| i as usize)
            .ok();

        Self {
            editor,
            protected_branches,
            protect_commit_count,
            protect_commit_age,
            auto_base_commit_count,
            stack,
            push_remote,
            pull_remote,
            show_format,
            show_commits,
            show_stacked,
            auto_fixup,
            auto_repair,
            capacity,
        }
    }

    pub fn write_repo(&self, repo: &git2::Repository) -> eyre::Result<()> {
        let config_path = git_dir_config(repo);
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
        self.editor = other.editor.or(self.editor);
        match (&mut self.protected_branches, other.protected_branches) {
            (Some(lhs), Some(rhs)) => lhs.extend(rhs),
            (None, Some(rhs)) => self.protected_branches = Some(rhs),
            (_, _) => (),
        }
        self.protect_commit_count = other.protect_commit_count.or(self.protect_commit_count);
        self.protect_commit_age = other.protect_commit_age.or(self.protect_commit_age);
        self.auto_base_commit_count = other.auto_base_commit_count.or(self.auto_base_commit_count);
        self.push_remote = other.push_remote.or(self.push_remote);
        self.pull_remote = other.pull_remote.or(self.pull_remote);
        self.stack = other.stack.or(self.stack);
        self.show_format = other.show_format.or(self.show_format);
        self.show_commits = other.show_commits.or(self.show_commits);
        self.show_stacked = other.show_stacked.or(self.show_stacked);
        self.auto_fixup = other.auto_fixup.or(self.auto_fixup);
        self.auto_repair = other.auto_repair.or(self.auto_repair);
        self.capacity = other.capacity.or(self.capacity);

        self
    }

    pub fn editor(&self) -> &str {
        self.editor.as_deref().unwrap_or(DEFAULT_CORE_EDITOR)
    }

    pub fn protected_branches(&self) -> &[String] {
        self.protected_branches.as_deref().unwrap_or(&[])
    }

    pub fn protect_commit_count(&self) -> Option<usize> {
        let protect_commit_count = self
            .protect_commit_count
            .unwrap_or(DEFAULT_PROTECT_COMMIT_COUNT);
        (protect_commit_count != 0).then_some(protect_commit_count)
    }

    pub fn protect_commit_age(&self) -> std::time::Duration {
        self.protect_commit_age
            .unwrap_or(DEFAULT_PROTECT_COMMIT_AGE)
    }

    pub fn auto_base_commit_count(&self) -> Option<usize> {
        let auto_base_commit_count = self
            .auto_base_commit_count
            .unwrap_or(DEFAULT_AUTO_BASE_COMMIT_COUNT);
        (auto_base_commit_count != 0).then_some(auto_base_commit_count)
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
        self.stack.unwrap_or_default()
    }

    pub fn show_format(&self) -> Format {
        self.show_format.unwrap_or_default()
    }

    pub fn show_commits(&self) -> ShowCommits {
        self.show_commits.unwrap_or_default()
    }

    pub fn show_stacked(&self) -> bool {
        self.show_stacked.unwrap_or(true)
    }

    pub fn auto_fixup(&self) -> Fixup {
        self.auto_fixup.unwrap_or_default()
    }

    pub fn auto_repair(&self) -> bool {
        self.auto_repair.unwrap_or(true)
    }

    pub fn capacity(&self) -> Option<usize> {
        let capacity = self.capacity.unwrap_or(DEFAULT_CAPACITY);
        (capacity != 0).then_some(capacity)
    }
}

impl std::fmt::Display for RepoConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "[{}]", CORE_EDITOR.split_once('.').unwrap().0)?;
        writeln!(
            f,
            "\t{}={}",
            CORE_EDITOR.split_once('.').unwrap().1,
            self.editor()
        )?;
        writeln!(f, "[{}]", STACK_FIELD.split_once('.').unwrap().0)?;
        for branch in self.protected_branches() {
            writeln!(
                f,
                "\t{}={}",
                PROTECTED_STACK_FIELD.split_once('.').unwrap().1,
                branch
            )?;
        }
        writeln!(
            f,
            "\t{}={}",
            PROTECT_COMMIT_COUNT.split_once('.').unwrap().1,
            self.protect_commit_count().unwrap_or(0)
        )?;
        writeln!(
            f,
            "\t{}={}",
            PROTECT_COMMIT_AGE.split_once('.').unwrap().1,
            humantime::format_duration(self.protect_commit_age())
        )?;
        writeln!(
            f,
            "\t{}={}",
            AUTO_BASE_COMMIT_COUNT.split_once('.').unwrap().1,
            self.auto_base_commit_count().unwrap_or(0)
        )?;
        writeln!(
            f,
            "\t{}={}",
            STACK_FIELD.split_once('.').unwrap().1,
            self.stack()
        )?;
        writeln!(
            f,
            "\t{}={}",
            PUSH_REMOTE_FIELD.split_once('.').unwrap().1,
            self.push_remote()
        )?;
        writeln!(
            f,
            "\t{}={}",
            PULL_REMOTE_FIELD.split_once('.').unwrap().1,
            self.pull_remote()
        )?;
        writeln!(
            f,
            "\t{}={}",
            FORMAT_FIELD.split_once('.').unwrap().1,
            self.show_format()
        )?;
        writeln!(
            f,
            "\t{}={}",
            SHOW_COMMITS_FIELD.split_once('.').unwrap().1,
            self.show_commits()
        )?;
        writeln!(
            f,
            "\t{}={}",
            STACKED_FIELD.split_once('.').unwrap().1,
            self.show_stacked()
        )?;
        writeln!(
            f,
            "\t{}={}",
            AUTO_FIXUP_FIELD.split_once('.').unwrap().1,
            self.auto_fixup()
        )?;
        writeln!(
            f,
            "\t{}={}",
            AUTO_REPAIR_FIELD.split_once('.').unwrap().1,
            self.auto_repair()
        )?;
        writeln!(f, "[{}]", BACKUP_CAPACITY_FIELD.split_once('.').unwrap().0)?;
        writeln!(
            f,
            "\t{}={}",
            BACKUP_CAPACITY_FIELD.split_once('.').unwrap().1,
            self.capacity().unwrap_or(0)
        )?;
        Ok(())
    }
}

fn git_dir_config(repo: &git2::Repository) -> std::path::PathBuf {
    repo.path().join("config")
}

fn default_branch(config: &git2::Config) -> &str {
    config.get_str("init.defaultBranch").ok().unwrap_or("main")
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, clap::ValueEnum)]
pub enum Format {
    /// No output
    Silent,
    /// List branches in selected stacks
    List,
    /// Render a branch branch
    Graph,
    /// Internal data for debugging
    Debug,
}

impl std::fmt::Display for Format {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use clap::ValueEnum;
        self.to_possible_value()
            .expect("no values are skipped")
            .get_name()
            .fmt(f)
    }
}

impl FromStr for Format {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use clap::ValueEnum;
        for variant in Self::value_variants() {
            if variant.to_possible_value().unwrap().matches(s, false) {
                return Ok(*variant);
            }
        }
        Err(format!("Invalid variant: {s}"))
    }
}

impl Default for Format {
    fn default() -> Self {
        Self::Graph
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, clap::ValueEnum)]
pub enum ShowCommits {
    None,
    Unprotected,
    All,
}

impl std::fmt::Display for ShowCommits {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use clap::ValueEnum;
        self.to_possible_value()
            .expect("no values are skipped")
            .get_name()
            .fmt(f)
    }
}

impl FromStr for ShowCommits {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use clap::ValueEnum;
        for variant in Self::value_variants() {
            if variant.to_possible_value().unwrap().matches(s, false) {
                return Ok(*variant);
            }
        }
        Err(format!("Invalid variant: {s}"))
    }
}

impl Default for ShowCommits {
    fn default() -> Self {
        Self::Unprotected
    }
}

#[derive(clap::ValueEnum, Debug, Copy, Clone, PartialEq, Eq)]
pub enum Stack {
    /// Branches in BASE..HEAD
    Current,
    /// Branches in BASE..HEAD..
    Dependents,
    /// Branches in BASE..
    Descendants,
    /// Show all branches
    All,
}

impl std::fmt::Display for Stack {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use clap::ValueEnum;
        self.to_possible_value()
            .expect("no values are skipped")
            .get_name()
            .fmt(f)
    }
}

impl FromStr for Stack {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use clap::ValueEnum;
        for variant in Self::value_variants() {
            if variant.to_possible_value().unwrap().matches(s, false) {
                return Ok(*variant);
            }
        }
        Err(format!("Invalid variant: {s}"))
    }
}

impl Default for Stack {
    fn default() -> Self {
        Self::All
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, clap::ValueEnum)]
pub enum Fixup {
    /// No special processing
    Ignore,
    /// Move them to after the commit they fix
    Move,
    /// Squash into the commit they fix
    Squash,
}

impl Fixup {
    pub fn variants() -> [&'static str; 3] {
        ["ignore", "move", "squash"]
    }
}

impl std::fmt::Display for Fixup {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use clap::ValueEnum;
        self.to_possible_value()
            .expect("no values are skipped")
            .get_name()
            .fmt(f)
    }
}

impl FromStr for Fixup {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use clap::ValueEnum;
        for variant in Self::value_variants() {
            if variant.to_possible_value().unwrap().matches(s, false) {
                return Ok(*variant);
            }
        }
        Err(format!("Invalid variant: {s}"))
    }
}

impl Default for Fixup {
    fn default() -> Self {
        Self::Move
    }
}
