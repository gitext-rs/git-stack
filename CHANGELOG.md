# Change Log
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](http://keepachangelog.com/)
and this project adheres to [Semantic Versioning](http://semver.org/).

<!-- next-header -->
## [Unreleased] - ReleaseDate

#### Fixes

- Ensure default format shows all branches

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
[Unreleased]: https://github.com/epage/git-stack/compare/v0.2.1...HEAD
[0.2.1]: https://github.com/epage/git-stack/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/epage/git-stack/compare/v0.1.0...v0.2.0
[v0.1.0]: https://github.com/epage/git-stack/compare/3137a1293f...v0.1.0
