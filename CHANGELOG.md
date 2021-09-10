# Change Log
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](http://keepachangelog.com/)
and this project adheres to [Semantic Versioning](http://semver.org/).

<!-- next-header -->
## [Unreleased] - ReleaseDate

#### Fixes

- Stack View:
  - Make highlights stand out more by using less color
  - Highlight dev branches pointing to protected commits
  - Make HEAD more obvious by listing it first
  - Removed a superfluous remote status

## [0.2.7] - 2021-09-01

#### Fixes

- Stack View:
  - Ensure protected commits are hidden when showing multiple protected branches

## [0.2.6] - 2021-09-01

#### Fixes

- Crash on merge of parent branch into child branch

## [0.2.5] - 2021-08-31

#### Fixes

- Don't stack unrelated branches (broken in 0.2.3)

#### Features

- Stack View
  - List HEAD branch after all dev branches to make it easier to spot
  - Highlight HEAD branch

## [0.2.4] - 2021-08-31

#### Fixes

- Resolved some more stack construction corner cases
- Stack View
  - Removed some degenerate cases by prioritizing protected branches over development branches
  - We elide "o" joints, where possible
  - Improved legibility of debug view by grouping non-nesting fields

## [0.2.3] - 2021-08-30

#### Fixes

- Don't crash with multiple protected branches
- `--dump-config` now dumps in `gitconfig` format
- Stack View: don't duplicate commits

## [0.2.2] - 2021-08-27

#### Fixes

- Rebase
  - Don't backup during dry-run
- Stack View:
  - Ensure default format shows all branches
  - Don't use warning-color on protected commits
  - Use distinct color for commits and protected branches
  - Reduce nesting in stack view in some degenerate cases
  - Show on rebase+dry-run, show tree as-if rebase succeeded

## [0.2.1] - 2021-08-25

#### Fixes

- Close a quote in the undo message

## [0.2.0] - 2021-08-25

#### Features

- Undo option
  - Built on new `git branch-backup` command which is like `git stash` for branch state
  - atm only backs up the result of a rebase and not `--pull`
- Stack View
  - Added new `--format branch-commits` option, now the default
  - Added new `--format debug` option to help with reporting issues
  - Abbreviate commit IDs
  - Show per-branch status, separating from commit status
- Auto-delete branches on `--pull` that were merged into a protected branch

#### Fixes

- Reduced conflicts during `--rebase`
- Load config when in a worktree
- Restore correct HEAD when multiple branches on the same commit

#### Breaking Chanages

- Renamed `--format` options:
  - `brief` -> `branches`
  - `full` -> `commits`

<!-- next-url -->
[Unreleased]: https://github.com/epage/git-stack/compare/v0.2.7...HEAD
[0.2.7]: https://github.com/epage/git-stack/compare/v0.2.6...v0.2.7
[0.2.6]: https://github.com/epage/git-stack/compare/v0.2.5...v0.2.6
[0.2.5]: https://github.com/epage/git-stack/compare/v0.2.4...v0.2.5
[0.2.4]: https://github.com/epage/git-stack/compare/v0.2.3...v0.2.4
[0.2.3]: https://github.com/epage/git-stack/compare/v0.2.2...v0.2.3
[0.2.2]: https://github.com/epage/git-stack/compare/v0.2.1...v0.2.2
[0.2.1]: https://github.com/epage/git-stack/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/epage/git-stack/compare/v0.1.0...v0.2.0
[v0.1.0]: https://github.com/epage/git-stack/compare/3137a1293f...v0.1.0
