# Change Log
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](http://keepachangelog.com/)
and this project adheres to [Semantic Versioning](http://semver.org/).

<!-- next-header -->
## [Unreleased] - ReleaseDate

## [0.10.20] - 2025-08-05

### Internal

- Update libgit2

## [0.10.19] - 2025-05-20

### Fixes

- Checkout rewritten detached HEAD commit when there are multiple branches

## [0.10.18] - 2025-01-22

### Performance

- Removed inefficient tree walking

### Fixes

- Improved likelihood that an operation will leave you at the correct commit / branch

## [0.10.17] - 2023-08-09

### Fixes

- Show the status for the correct branch when multiple point to the same commit

## [0.10.16] - 2023-05-02

### Fixes

- Don't panic on `git stack --onto <sha>`

## [0.10.15] - 2023-04-13

### Internal

- Update dependency

## [0.10.14] - 2023-03-16

### Internal

- Update dependency

## [0.10.13] - 2023-03-14

### Fixes

- Correctly handle `CLICOLOR=1`
- Correctly handle `NO_COLOR=`

### Performance

- Do not checkout files if the tree is unchanged

## [0.10.12] - 2023-02-02

### Fixes

- Imitate git in editing commit messages, allowing at least `vim` to recognize the filetype

## [0.10.11] - 2023-01-25

### Fixes

- Commit and sign with committer signature

## [0.10.10] - 2023-01-18

### Fixes

- Write to the users config, whichever location it is in

## [0.10.9] - 2023-01-12

### Fixes

- Improve error reporting when operations like rebase are in progress

## [0.10.8] - 2023-01-12

### Fixes

- `git amend <rev>` no longer drops commits after `<rev>`

## [0.10.7] - 2023-01-12

### Fixes

- Sign rebased commits

## [0.10.6] - 2023-01-11

### Compatibility

- `git amend` no-ops now fail

### Features

- `git amend <rev>` is now supported

### Fixes

- `git amend` only reports an amend happening on success
- `git amend` no-ops are now quieter
- `git amend` no-ops now fail
- `git amend` can now amend fixups
- `git amend` won't lose unstaged changes anymore
- `git amend` moves to fixup commit on failure
- `git run` is more explicit about what it is testing and what the status was for

## [0.10.5] - 2023-01-10

### Fixes

- `git amend` correctly checks out new HEAD when amending a detached HEAD

## [0.10.4] - 2023-01-10

### Fixes

- `git sync` rebases to the fetched remote, not the remote at process start

## [0.10.3] - 2023-01-10

### Fixes

- `git amend` can commit staged changes

## [0.10.2] - 2023-01-09

### Features

- `git reword` now takes an optional rev for rewording commits other than HEAD

### Fixes

- `git amend -m` didn't work, now it does
- `git amend` can now work on detached HEADS

## [0.10.1] - 2023-01-06

### Features

- `git run --switch` to checkout the commit that failed
- `git run --fail-fast` now exists and is the default, disable with `--no-fail-fast` / `--no-ff`

### Fixes

- `git sync` now rebases onto the fetched branch

## [0.10.0] - 2023-01-05

### Features

- Initial `gpgSign` support.  This is untested so if `commit.gpgSign` is causing `git-stack` to fail, you can use `stack.gpgSign` to disable it.

## [0.9.0] - 2023-01-04

### Features

New `git stack` subcommands, including
- `next` and `prev`
- `sync` (with the aim of replacing `git stack --pull`)
- `amend`
- `reword`
- `run`

With `git stack alias` for creating alias for these

## [0.8.5] - 2022-09-06

### Fixes

- Provide more information on crash/panic

## [0.8.4] - 2022-08-10

### Fixes

- Correctly identify parent branch when a protected branch points to a commit that is a direct ancestor of another protected branch

## [0.8.3] - 2022-07-02

### Fixes

- Foreign branches are always protected

## [0.8.2] - 2022-03-23

### Fixes

- Find a better base for commits without a protected branch

### Performance

- Split off old branches into dedicated stacks before building the commit graph, controlled by `stack.auto-base-commit-count`.
- Cache some git operations in-memory

## [0.8.1] - 2022-03-21

### Fixes

- Don't fail on undetected base

### Performance

- Speed up view rendering with branches on very old commits by caching git operations in-memory

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

### Breaking Changes

- Renamed `--format` options:
  - `brief` -> `branches`
  - `full` -> `commits`

<!-- next-url -->
[Unreleased]: https://github.com/gitext-rs/git-stack/compare/v0.10.20...HEAD
[0.10.20]: https://github.com/gitext-rs/git-stack/compare/v0.10.19...v0.10.20
[0.10.19]: https://github.com/gitext-rs/git-stack/compare/v0.10.18...v0.10.19
[0.10.18]: https://github.com/gitext-rs/git-stack/compare/v0.10.17...v0.10.18
[0.10.17]: https://github.com/gitext-rs/git-stack/compare/v0.10.16...v0.10.17
[0.10.16]: https://github.com/gitext-rs/git-stack/compare/v0.10.15...v0.10.16
[0.10.15]: https://github.com/gitext-rs/git-stack/compare/v0.10.14...v0.10.15
[0.10.14]: https://github.com/gitext-rs/git-stack/compare/v0.10.13...v0.10.14
[0.10.13]: https://github.com/gitext-rs/git-stack/compare/v0.10.12...v0.10.13
[0.10.12]: https://github.com/gitext-rs/git-stack/compare/v0.10.11...v0.10.12
[0.10.11]: https://github.com/gitext-rs/git-stack/compare/v0.10.10...v0.10.11
[0.10.10]: https://github.com/gitext-rs/git-stack/compare/v0.10.9...v0.10.10
[0.10.9]: https://github.com/gitext-rs/git-stack/compare/v0.10.8...v0.10.9
[0.10.8]: https://github.com/gitext-rs/git-stack/compare/v0.10.7...v0.10.8
[0.10.7]: https://github.com/gitext-rs/git-stack/compare/v0.10.6...v0.10.7
[0.10.6]: https://github.com/gitext-rs/git-stack/compare/v0.10.5...v0.10.6
[0.10.5]: https://github.com/gitext-rs/git-stack/compare/v0.10.4...v0.10.5
[0.10.4]: https://github.com/gitext-rs/git-stack/compare/v0.10.3...v0.10.4
[0.10.3]: https://github.com/gitext-rs/git-stack/compare/v0.10.2...v0.10.3
[0.10.2]: https://github.com/gitext-rs/git-stack/compare/v0.10.1...v0.10.2
[0.10.1]: https://github.com/gitext-rs/git-stack/compare/v0.10.0...v0.10.1
[0.10.0]: https://github.com/gitext-rs/git-stack/compare/v0.9.0...v0.10.0
[0.9.0]: https://github.com/gitext-rs/git-stack/compare/v0.8.5...v0.9.0
[0.8.5]: https://github.com/gitext-rs/git-stack/compare/v0.8.4...v0.8.5
[0.8.4]: https://github.com/gitext-rs/git-stack/compare/v0.8.3...v0.8.4
[0.8.3]: https://github.com/gitext-rs/git-stack/compare/v0.8.2...v0.8.3
[0.8.2]: https://github.com/gitext-rs/git-stack/compare/v0.8.1...v0.8.2
[0.8.1]: https://github.com/gitext-rs/git-stack/compare/v0.8.0...v0.8.1
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
