// Not correctly overriding on Windows
#![cfg(target_os = "linux")]

use snapbox::prelude::*;
use snapbox::str;

#[test]
fn list_no_config() {
    let root = snapbox::dir::DirRoot::mutable_temp().unwrap();
    let root_path = root.path().unwrap();

    let home_root = root_path.join("home");
    std::fs::create_dir_all(&home_root).unwrap();

    let repo_root = root_path.join("repo");
    git2::Repository::init(&repo_root).unwrap();

    snapbox::cmd::Command::new(snapbox::cmd::cargo_bin!("git-stack"))
        .arg("alias")
        .current_dir(&repo_root)
        .env("HOME", &home_root)
        .assert()
        .success()
        .stdout_eq_(str![[r#"
[alias]
#   next = stack next  # unregistered
#   prev = stack previous  # unregistered
#   reword = stack reword  # unregistered
#   amend = stack amend  # unregistered
#   sync = stack sync  # unregistered
#   run = stack run  # unregistered

"#]].raw())
        .stderr_eq_(str![[r#"
note: To register, pass `--register`

"#]]);

    root.close().unwrap();
}

#[test]
fn list_global_config() {
    let root = snapbox::dir::DirRoot::mutable_temp().unwrap();
    let root_path = root.path().unwrap();

    let home_root = root_path.join("home");
    std::fs::create_dir_all(&home_root).unwrap();
    std::fs::write(
        home_root.join(".gitconfig"),
        "
[alias]
  next = foo
",
    )
    .unwrap();

    let repo_root = root_path.join("repo");
    git2::Repository::init(&repo_root).unwrap();

    snapbox::cmd::Command::new(snapbox::cmd::cargo_bin!("git-stack"))
        .arg("alias")
        .current_dir(&repo_root)
        .env("HOME", &home_root)
        .assert()
        .success()
        .stdout_eq_(str![[r#"
[alias]
    next = foo  # instead of `stack next`
#   prev = stack previous  # unregistered
#   reword = stack reword  # unregistered
#   amend = stack amend  # unregistered
#   sync = stack sync  # unregistered
#   run = stack run  # unregistered

"#]].raw())
        .stderr_eq_(str![[r#"
note: To register, pass `--register`

"#]]);

    root.close().unwrap();
}

#[test]
fn register_no_config() {
    let root = snapbox::dir::DirRoot::mutable_temp().unwrap();
    let root_path = root.path().unwrap();

    let home_root = root_path.join("home");
    std::fs::create_dir_all(&home_root).unwrap();

    let repo_root = root_path.join("repo");
    git2::Repository::init(&repo_root).unwrap();

    snapbox::cmd::Command::new(snapbox::cmd::cargo_bin!("git-stack"))
        .arg("alias")
        .arg("--register")
        .current_dir(&repo_root)
        .env("HOME", &home_root)
        .assert()
        .success()
        .stdout_eq_(str![].raw())
        .stderr_eq_(str![[r#"
Registering: next="stack next"
Registering: prev="stack previous"
Registering: reword="stack reword"
Registering: amend="stack amend"
Registering: sync="stack sync"
Registering: run="stack run"

"#]]);

    snapbox::cmd::Command::new(snapbox::cmd::cargo_bin!("git-stack"))
        .arg("alias")
        .current_dir(&repo_root)
        .env("HOME", &home_root)
        .assert()
        .success()
        .stdout_eq_(str![[r#"
[alias]
    next = stack next  # registered
    prev = stack previous  # registered
    reword = stack reword  # registered
    amend = stack amend  # registered
    sync = stack sync  # registered
    run = stack run  # registered

"#]].raw())
        .stderr_eq_(str![[r#"
note: To unregister, pass `--unregister`

"#]]);

    root.close().unwrap();
}

#[test]
fn register_no_overwrite_alias() {
    let root = snapbox::dir::DirRoot::mutable_temp().unwrap();
    let root_path = root.path().unwrap();

    let home_root = root_path.join("home");
    std::fs::create_dir_all(&home_root).unwrap();
    std::fs::write(
        home_root.join(".gitconfig"),
        "
[alias]
  next = foo
  prev = stack previous -v
",
    )
    .unwrap();

    let repo_root = root_path.join("repo");
    git2::Repository::init(&repo_root).unwrap();

    snapbox::cmd::Command::new(snapbox::cmd::cargo_bin!("git-stack"))
        .arg("alias")
        .arg("--register")
        .current_dir(&repo_root)
        .env("HOME", &home_root)
        .assert()
        .failure()
        .stdout_eq_(str![].raw())
        .stderr_eq_(str![[r#"
error: next="foo" is registered, not overwriting with "stack next"
Registering: reword="stack reword"
Registering: amend="stack amend"
Registering: sync="stack sync"
Registering: run="stack run"

"#]]);

    snapbox::cmd::Command::new(snapbox::cmd::cargo_bin!("git-stack"))
        .arg("alias")
        .current_dir(&repo_root)
        .env("HOME", &home_root)
        .assert()
        .success()
        .stdout_eq_(str![[r#"
[alias]
    next = foo  # instead of `stack next`
    prev = stack previous -v  # diverged from "stack previous"
    reword = stack reword  # registered
    amend = stack amend  # registered
    sync = stack sync  # registered
    run = stack run  # registered

"#]].raw())
        .stderr_eq_(str![[r#"
note: To unregister, pass `--unregister`

"#]]);

    root.close().unwrap();
}

#[test]
fn register_unregister() {
    let root = snapbox::dir::DirRoot::mutable_temp().unwrap();
    let root_path = root.path().unwrap();

    let home_root = root_path.join("home");
    std::fs::create_dir_all(&home_root).unwrap();

    let repo_root = root_path.join("repo");
    git2::Repository::init(&repo_root).unwrap();

    snapbox::cmd::Command::new(snapbox::cmd::cargo_bin!("git-stack"))
        .arg("alias")
        .arg("--register")
        .current_dir(&repo_root)
        .env("HOME", &home_root)
        .assert()
        .success();

    snapbox::cmd::Command::new(snapbox::cmd::cargo_bin!("git-stack"))
        .arg("alias")
        .arg("--unregister")
        .current_dir(&repo_root)
        .env("HOME", &home_root)
        .assert()
        .success()
        .stdout_eq_(str![])
        .stderr_eq_(str![[r#"
Unregistering: next="stack next"
Unregistering: prev="stack previous"
Unregistering: reword="stack reword"
Unregistering: amend="stack amend"
Unregistering: sync="stack sync"
Unregistering: run="stack run"

"#]]);

    root.close().unwrap();
}

#[test]
fn reregister() {
    let root = snapbox::dir::DirRoot::mutable_temp().unwrap();
    let root_path = root.path().unwrap();

    let home_root = root_path.join("home");
    std::fs::create_dir_all(&home_root).unwrap();

    let repo_root = root_path.join("repo");
    git2::Repository::init(&repo_root).unwrap();

    snapbox::cmd::Command::new(snapbox::cmd::cargo_bin!("git-stack"))
        .arg("alias")
        .arg("--register")
        .current_dir(&repo_root)
        .env("HOME", &home_root)
        .assert()
        .success();

    snapbox::cmd::Command::new(snapbox::cmd::cargo_bin!("git-stack"))
        .arg("alias")
        .arg("--register")
        .current_dir(&repo_root)
        .env("HOME", &home_root)
        .assert()
        .success()
        .stdout_eq_(str![].raw())
        .stderr_eq_(str![]);

    root.close().unwrap();
}

#[test]
fn unregister_no_config() {
    let root = snapbox::dir::DirRoot::mutable_temp().unwrap();
    let root_path = root.path().unwrap();

    let home_root = root_path.join("home");
    std::fs::create_dir_all(&home_root).unwrap();

    let repo_root = root_path.join("repo");
    git2::Repository::init(&repo_root).unwrap();

    snapbox::cmd::Command::new(snapbox::cmd::cargo_bin!("git-stack"))
        .arg("alias")
        .arg("--unregister")
        .current_dir(&repo_root)
        .env("HOME", &home_root)
        .assert()
        .success()
        .stdout_eq_(str![].raw())
        .stderr_eq_(str![]);

    root.close().unwrap();
}

#[test]
fn unregister_existing_config() {
    let root = snapbox::dir::DirRoot::mutable_temp().unwrap();
    let root_path = root.path().unwrap();

    let home_root = root_path.join("home");
    std::fs::create_dir_all(&home_root).unwrap();
    std::fs::write(
        home_root.join(".gitconfig"),
        "
[alias]
  next = foo
  prev = stack previous -v
  reword = stack reword
",
    )
    .unwrap();

    let repo_root = root_path.join("repo");
    git2::Repository::init(&repo_root).unwrap();

    snapbox::cmd::Command::new(snapbox::cmd::cargo_bin!("git-stack"))
        .arg("alias")
        .arg("--unregister")
        .current_dir(&repo_root)
        .env("HOME", &home_root)
        .assert()
        .success()
        .stdout_eq_(str![].raw())
        .stderr_eq_(str![[r#"
Unregistering: prev="stack previous -v"
Unregistering: reword="stack reword"

"#]]);

    snapbox::cmd::Command::new(snapbox::cmd::cargo_bin!("git-stack"))
        .arg("alias")
        .current_dir(&repo_root)
        .env("HOME", &home_root)
        .assert()
        .success()
        .stdout_eq_(str![[r#"
[alias]
    next = foo  # instead of `stack next`
#   prev = stack previous  # unregistered
#   reword = stack reword  # unregistered
#   amend = stack amend  # unregistered
#   sync = stack sync  # unregistered
#   run = stack run  # unregistered

"#]].raw())
        .stderr_eq_(str![[r#"
note: To register, pass `--register`

"#]]);

    root.close().unwrap();
}
