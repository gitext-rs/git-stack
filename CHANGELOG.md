# Change Log
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](http://keepachangelog.com/)
and this project adheres to [Semantic Versioning](http://semver.org/).

<!-- next-header -->
## [Unreleased] - ReleaseDate

## [0.8.0] - 2022-03-18

### Compatibility

- Pull remote handling changed enough that regressions could have been introduced.
- Slight changes to how `--base` and `--onto` are defaulted
- `--pull --onto <remote>/<branch>` behavior changed

### Fixes

- Show remote branches when they diverge from base branch
- `--base` defaults to the local branch of `--onto`
- `--onto` defaults to the remote branch of `--base`
- Pull using the remote specified in `--onto` rather than just the upstream

## [0.7.4] - 2022-03-17

### Features

- Support commits and tags for `--onto` and `--base` arguments

## [0.7.3] - 2022-03-17

### Features

- Support remote tracking branches for `--onto` and `--base` arguments

## [0.7.2] - 2022-03-17

### Fixes

- Call `post-rewrite` and `reference-transaction` git hooks

## [0.7.1] - 2022-03-17

### Fixes

- Consistent spacing between stacks

## [0.7.0] - 2022-03-16

### Compatibility

- Commit graph implementation changed enough that regressions could have been introduced

### Features

- `-C <path>` support for changing the current directory

### Fixes

- Be smarter about picking the protected branch for a given feature branch

### Performance

- Speed up operations on large, complex commit histories like `gecko-dev`

## [0.6.0] - 2022-03-01

### Breaking Changes

- `--stack`, `--format`, `--fixup` are now case sensitive
- Most `--format` options are replaced with `--format graph --show-commits ...`

### Features

- `--format list` to list branches that are part of the selected stacks (`--stack`).

### Fixes

- Be more explicit in why a push didn't happen
- Decouple showing of commits from `--format`

## [0.5.6] - 2022-02-28

### Fixes

- Respect existing upstream configured for current branch
- Respect `remote.pushDefault`

## [0.5.5] - 2022-01-26

### Fixes

- Don't panic on `--base --onto --stack all`

## [0.5.4] - 2022-01-11

## [0.5.3] - 2021-11-13

### Fixes

- Only prune branches when they don't exist on the server, rather than also if they have a `/`

## [0.5.2] - 2021-11-11

### Fixes

- Do not auto-protect (by age or user) HEAD

## [0.5.1] - 2021-11-11

### Fixes

- Read `protect-commit-age` from gitconfig

## [0.5.0] - 2021-11-09

### Breaking Changes

### Features

- New `--repair` flag
  - Re-stacks branches on top of each other
  - Tries to merge branches that have diverged

### Fixes

- Stack visualization
  - Made it more compact
  - Change the commit glyph
  - Made it more consistently linear
  - Fix sorting so the longest branches are last
  - Always show leaf commits
- Refined stack visualization
- Don't lose tranbhes with `--onto`
- Don't treat base/onto as protected branches
- Don't pull all when there is nothing to pull
- Respect `--format=commits`
- Preserve old commit time on `--rebase`
- Branch backup now includes the rebase during `--pull`
- Show `--pull`s behavior on dry-run
- Allow dirty tree on dry-run

### Performance

- Reduce the amount of data we process
- Reduce stack usage when rendering

## [0.4.8] - 2021-10-25

### Fixes

- We should only squash the fixup and not the ones before it

## [0.4.7] - 2021-10-23

### Fixes

- Detect multi-commit branches are pushable

## [0.4.6] - 2021-10-22

### Fixes

- Further reduce the chance for stackoverflows

## [0.4.5] - 2021-10-22

### Fixes

- Summarize other people's branches to unclutter visualization
- Avoid summarizing a branch with HEAD

## [0.4.4] - 2021-10-22

### Fixes

- Always prune from the push-remote, not just when configured
- Speed up fetching large push-remotes by only fetching what is needed
- Don't fetch the push-remote on dry-run
- Don't mark local edits as protected

## [0.4.3] - 2021-10-22

### Fixes

- Color log level, regardless of min log level

## [0.4.2] - 2021-10-22

### Fixes

- Clean up stack visualization
  - Remove nesting by not showing merge-bases of protected branches
  - Treat large branches as protected, abbreviating them
  - Summarize empty stacks
  - Summarize old branches
- Reduced or eliminated stackoverflows

## [0.4.1] - 2021-10-21

### Fixes

- Read all values from `.gitconfig`, rather than just some

## [0.4.0] - 2021-10-21

### Breaking Changes

- Renamed config `stack.fixp` to `stack.auto-fixup` to clarify role

### Fixes

- Changed `--pull` to not perform `stack.auto-fixup`
- Allow `--fixup` to run without `--rebase`

## [0.3.0] - 2021-10-20

### Breaking Changes

- Command line argument values have changed
- Renamed `git-branch-backup` to `git-branch-stash`

### Features

- Auto-stash support

### Fixes

- Switched command line arguments to match config file
- Vendor libgit2
- Don't panic on some merge conflicts
- Correctly detect `init.defaultBranch` as a protected branch
- Correctly detect some more protected commit cases
- Reduce scope of dirty checks
- Some visualization improvements
- Fix some branch deletion corner cases
- Auto-delete branches from squash-merges

## [0.2.9] - 2021-10-07

### Features

- `git stack --pull` will also fetch the push-remote, ensuring we show the latest status relative to it.

### Fixes

- Highlight detached HEAD
- Changed branch status precedence
- Tweaked colors
- Smarter color control

## [0.2.8] - 2021-09-10

### Fixes

- Stack View:
  - Make highlights stand out more by using less color
  - Highlight dev branches pointing to protected commits
  - Make HEAD more obvious by listing it first
  - Removed a superfluous remote status

## [0.2.7] - 2021-09-01

### Fixes

- Stack View:
  - Ensure protected commits are hidden when showing multiple protected branches

## [0.2.6] - 2021-09-01

### Fixes

- Crash on merge of parent branch into child branch

## [0.2.5] - 2021-08-31

### Fixes

- Don't stack unrelated branches (broken in 0.2.3)

### Features

- Stack View
  - List HEAD branch after all dev branches to make it easier to spot
  - Highlight HEAD branch

## [0.2.4] - 2021-08-31

### Fixes

- Resolved some more stack construction corner cases
- Stack View
  - Removed some degenerate cases by prioritizing protected branches over development branches
  - We elide "o" joints, where possible
  - Improved legibility of debug view by grouping non-nesting fields

## [0.2.3] - 2021-08-30

### Fixes

- Don't crash with multiple protected branches
- `--dump-config` now dumps in `gitconfig` format
- Stack View: don't duplicate commits

## [0.2.2] - 2021-08-27

### Fixes

- Rebase
  - Don't backup during dry-run
- Stack View:
  - Ensure default format shows all branches
  - Don't use warning-color on protected commits
  - Use distinct color for commits and protected branches
  - Reduce nesting in stack view in some degenerate cases
  - Show on rebase+dry-run, show tree as-if rebase succeeded

## [0.2.1] - 2021-08-25

### Fixes

- Close a quote in the undo message

## [0.2.0] - 2021-08-25

### Features

- Undo option
  - Built on new `git branch-backup` command which is like `git stash` for branch state
  - atm only backs up the result of a rebase and not `--pull`
- Stack View
  - Added new `--format branch-commits` option, now the default
  - Added new `--format debug` option to help with reporting issues
  - Abbreviate commit IDs
  - Show per-branch status, separating from commit status
- Auto-delete branches on `--pull` that were merged into a protected branch

### Fixes

- Reduced conflicts during `--rebase`
- Load config when in a worktree
- Restore correct HEAD when multiple branches on the same commit

### Breaking Chanages

- Renamed `--format` options:
  - `brief` -> `branches`
  - `full` -> `commits`

<!-- next-url -->
[Unreleased]: https://github.com/gitext-rs/git-stack/compare/v0.8.0...HEAD
[0.8.0]: https://github.com/gitext-rs/git-stack/compare/v0.7.4...v0.8.0
[0.7.4]: https://github.com/gitext-rs/git-stack/compare/v0.7.3...v0.7.4
[0.7.3]: https://github.com/gitext-rs/git-stack/compare/v0.7.2...v0.7.3
[0.7.2]: https://github.com/gitext-rs/git-stack/compare/v0.7.1...v0.7.2
[0.7.1]: https://github.com/gitext-rs/git-stack/compare/v0.7.0...v0.7.1
[0.7.0]: https://github.com/gitext-rs/git-stack/compare/v0.6.0...v0.7.0
[0.6.0]: https://github.com/gitext-rs/git-stack/compare/v0.5.6...v0.6.0
[0.5.6]: https://github.com/gitext-rs/git-stack/compare/v0.5.5...v0.5.6
[0.5.5]: https://github.com/gitext-rs/git-stack/compare/v0.5.4...v0.5.5
[0.5.4]: https://github.com/gitext-rs/git-stack/compare/v0.5.3...v0.5.4
[0.5.3]: https://github.com/gitext-rs/git-stack/compare/v0.5.2...v0.5.3
[0.5.2]: https://github.com/gitext-rs/git-stack/compare/v0.5.1...v0.5.2
[0.5.1]: https://github.com/gitext-rs/git-stack/compare/v0.5.0...v0.5.1
[0.5.0]: https://github.com/gitext-rs/git-stack/compare/v0.4.8...v0.5.0
[0.4.8]: https://github.com/gitext-rs/git-stack/compare/v0.4.7...v0.4.8
[0.4.7]: https://github.com/gitext-rs/git-stack/compare/v0.4.6...v0.4.7
[0.4.6]: https://github.com/gitext-rs/git-stack/compare/v0.4.5...v0.4.6
[0.4.5]: https://github.com/gitext-rs/git-stack/compare/v0.4.4...v0.4.5
[0.4.4]: https://github.com/gitext-rs/git-stack/compare/v0.4.3...v0.4.4
[0.4.3]: https://github.com/gitext-rs/git-stack/compare/v0.4.2...v0.4.3
[0.4.2]: https://github.com/gitext-rs/git-stack/compare/v0.4.1...v0.4.2
[0.4.1]: https://github.com/gitext-rs/git-stack/compare/v0.4.0...v0.4.1
[0.4.0]: https://github.com/gitext-rs/git-stack/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/gitext-rs/git-stack/compare/v0.2.10...v0.3.0
[0.2.10]: https://github.com/gitext-rs/git-stack/compare/v0.2.9...v0.2.10
[0.2.9]: https://github.com/gitext-rs/git-stack/compare/v0.2.8...v0.2.9
[0.2.8]: https://github.com/gitext-rs/git-stack/compare/v0.2.7...v0.2.8
[0.2.7]: https://github.com/gitext-rs/git-stack/compare/v0.2.6...v0.2.7
[0.2.6]: https://github.com/gitext-rs/git-stack/compare/v0.2.5...v0.2.6
[0.2.5]: https://github.com/gitext-rs/git-stack/compare/v0.2.4...v0.2.5
[0.2.4]: https://github.com/gitext-rs/git-stack/compare/v0.2.3...v0.2.4
[0.2.3]: https://github.com/gitext-rs/git-stack/compare/v0.2.2...v0.2.3
[0.2.2]: https://github.com/gitext-rs/git-stack/compare/v0.2.1...v0.2.2
[0.2.1]: https://github.com/gitext-rs/git-stack/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/gitext-rs/git-stack/compare/v0.1.0...v0.2.0
[v0.1.0]: https://github.com/gitext-rs/git-stack/compare/3137a1293f...v0.1.0
