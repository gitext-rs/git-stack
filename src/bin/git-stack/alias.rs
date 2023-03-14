use std::io::Write;

use proc_exit::prelude::*;

#[derive(clap::Args)]
pub struct AliasArgs {
    #[arg(long)]
    register: bool,

    #[arg(long)]
    unregister: bool,
}

impl AliasArgs {
    pub fn exec(&self) -> proc_exit::ExitResult {
        if self.register {
            register()?;
        } else if self.unregister {
            unregister()?;
        } else {
            status()?;
        }

        Ok(())
    }
}

fn register() -> proc_exit::ExitResult {
    let config = if let Ok(config) = open_repo_config() {
        config
    } else {
        git2::Config::open_default().with_code(proc_exit::Code::FAILURE)?
    }
    .snapshot()
    .with_code(proc_exit::Code::FAILURE)?;

    let mut user_config = git2::Config::open_default()
        .with_code(proc_exit::Code::FAILURE)?
        .open_global()
        .with_code(proc_exit::Code::FAILURE)?;

    let stderr_palette = crate::ops::Palette::colored();
    let mut stderr = anstyle_stream::stderr().lock();

    let mut success = true;
    for alias in ALIASES {
        let key = format!("alias.{}", alias.alias);
        match config.get_string(&key) {
            Ok(value) => {
                if value == alias.action {
                    log::debug!("{}=\"{}\" is registered", alias.alias, value);
                } else if value.starts_with(alias.action_base) {
                    log::debug!(
                        "{}=\"{}\" is registered but diverged from \"{}\"",
                        alias.alias,
                        value,
                        alias.action
                    );
                } else {
                    let _ = writeln!(
                        stderr,
                        "{}: {}=\"{}\" is registered, not overwriting with \"{}\"",
                        stderr_palette.error("error"),
                        alias.alias,
                        value,
                        alias.action_base
                    );
                    success = false;
                }
            }
            Err(_) => {
                let _ = writeln!(
                    stderr,
                    "{}: {}=\"{}\"",
                    stderr_palette.good("Registering"),
                    alias.alias,
                    alias.action
                );
                user_config
                    .set_str(&key, alias.action)
                    .with_code(proc_exit::Code::FAILURE)?;
            }
        }
    }

    if success {
        Ok(())
    } else {
        Err(proc_exit::Code::FAILURE.as_exit())
    }
}

fn unregister() -> proc_exit::ExitResult {
    let config = if let Ok(config) = open_repo_config() {
        config
    } else {
        git2::Config::open_default().with_code(proc_exit::Code::FAILURE)?
    }
    .snapshot()
    .with_code(proc_exit::Code::FAILURE)?;

    let mut user_config = git2::Config::open_default()
        .with_code(proc_exit::Code::FAILURE)?
        .open_global()
        .with_code(proc_exit::Code::FAILURE)?;

    let stderr_palette = crate::ops::Palette::colored();
    let mut stderr = anstyle_stream::stderr().lock();

    let mut entries = config
        .entries(Some("alias.*"))
        .with_code(proc_exit::Code::FAILURE)?;
    while let Some(entry) = entries.next() {
        let entry = entry.with_code(proc_exit::Code::FAILURE)?;
        let Some(key) = entry.name() else {continue};
        let name = key.split_once('.').map(|n| n.1).unwrap_or(key);
        let Some(value) = entry.value() else {continue};

        let mut unregister = false;
        if let Some(alias) = ALIASES.iter().find(|a| a.alias == name) {
            if value == alias.action {
                unregister = true;
            } else if value.starts_with(alias.action_base) {
                unregister = true;
            }
        } else if let Some(_alias) = ALIASES.iter().find(|a| value.starts_with(a.action_base)) {
            unregister = true;
        }

        if unregister {
            let _ = writeln!(
                stderr,
                "{}: {}=\"{}\"",
                stderr_palette.good("Unregistering"),
                name,
                value
            );
            user_config
                .remove(key)
                .with_code(proc_exit::Code::FAILURE)?;
        }
    }

    Ok(())
}

fn status() -> proc_exit::ExitResult {
    let config = if let Ok(config) = open_repo_config() {
        config
    } else {
        git2::Config::open_default().with_code(proc_exit::sysexits::USAGE_ERR)?
    };

    let stdout_palette = crate::ops::Palette::colored();
    let stderr_palette = crate::ops::Palette::colored();
    let mut stdout = anstyle_stream::stdout().lock();
    let mut stderr = anstyle_stream::stderr().lock();
    let _ = writeln!(stdout, "[alias]");

    let mut registered = false;
    let mut covered = std::collections::HashSet::new();
    let mut entries = config
        .entries(Some("alias.*"))
        .with_code(proc_exit::Code::FAILURE)?;
    while let Some(entry) = entries.next() {
        let entry = entry.with_code(proc_exit::Code::FAILURE)?;
        let Some(name) = entry.name() else {continue};
        let name = name.split_once('.').map(|n| n.1).unwrap_or(name);
        let Some(value) = entry.value() else {continue};

        if let Some(alias) = ALIASES.iter().find(|a| a.alias == name) {
            if value == alias.action {
                let _ = writeln!(
                    stdout,
                    "{}{}",
                    stdout_palette.good(format_args!("    {name} = {value}")),
                    stdout_palette.hint("  # registered")
                );
                registered = true;
            } else if value.starts_with(alias.action_base) {
                let _ = writeln!(
                    stdout,
                    "{}{}",
                    stdout_palette.warn(format_args!("    {name} = {value}")),
                    stdout_palette.hint(format_args!("  # diverged from \"{}\"", alias.action))
                );
                registered = true;
            } else {
                let _ = writeln!(
                    stdout,
                    "{}{}",
                    stdout_palette.error(format_args!("    {name} = {value}")),
                    stdout_palette.hint(format_args!("  # instead of `{}`", alias.action))
                );
            }
            covered.insert(name.to_owned());
        } else if let Some(_alias) = ALIASES.iter().find(|a| value.starts_with(a.action_base)) {
            let _ = writeln!(stdout, "    {name} = {value}");
            registered = true;
        }
    }

    let mut unregistered = false;
    for alias in ALIASES {
        if covered.contains(alias.alias) {
            continue;
        }
        let _ = writeln!(
            stdout,
            "{}{}",
            stdout_palette.error(format_args!("#   {} = {}", alias.alias, alias.action)),
            stdout_palette.hint("  # unregistered")
        );
        unregistered = true;
    }

    if registered {
        let _ = writeln!(
            stderr,
            "{}: To unregister, pass {}",
            stderr_palette.info("note"),
            stderr_palette.error("`--unregister`")
        );
    }
    if unregistered {
        let _ = writeln!(
            stderr,
            "{}: To register, pass {}",
            stderr_palette.info("note"),
            stderr_palette.good("`--register`")
        );
    }

    Ok(())
}

pub struct Alias {
    pub alias: &'static str,
    pub action: &'static str,
    pub action_base: &'static str,
}

const ALIASES: &[Alias] = &[
    crate::next::NextArgs::alias(),
    crate::prev::PrevArgs::alias(),
    crate::reword::RewordArgs::alias(),
    crate::amend::AmendArgs::alias(),
    crate::sync::SyncArgs::alias(),
    crate::run::RunArgs::alias(),
];

fn open_repo_config() -> Result<git2::Config, eyre::Error> {
    let cwd = std::env::current_dir()?;
    let repo = git2::Repository::discover(cwd)?;
    let config = repo.config()?;
    Ok(config)
}
