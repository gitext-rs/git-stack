# `git-stack` Reference

## Actions

### `--pull`

Pulls your protected branches from the `stack.pull-remote` and then rebases
your development branches on top of their relevant protected branches.

Unlike `--rebase`, this does not perform any "auto" operations.

Note:
- This also performs a fetch of your `stack.push-remote` to take advantage of
  [`fetch.prune`](https://git-scm.com/docs/git-config#Documentation/git-config.txt-fetchprune)
  (`git config --global fetch.prune true`).

### `--rebase`

Rebase development branches on their relevant protected branches.

This performs "auto" operations, like
- `stack.auto-fixup`: see `--fixup`

### `--fixup <action>`

Process [fixup!](https://git-scm.com/docs/git-config#Documentation/git-config.txt-fetchprune) commits according to the specified action.

Note:
- This can be used to override `stack.auto-fixup` during a `--rebase`.

### `--push`

Push all "ready" development branches to your `stack.push-remote`.

A branch is ready if
- It is not stacked on top of any other development branches (see ["How do I stack my PRs in Github"](../README.md#how-do-i-stack-my-prs-in-github))
- It has no [WIP commits](../README.md#when-is-a-commit-considered-wip)

We consider branches with
[`fixup!` commits](https://git-scm.com/docs/git-commit#Documentation/git-commit.txt---fixupamendrewordltcommitgt)
to be ready in case you are wanting reviewers to see some intermediate states.
You can use a tool like [committed](https://github.com/crate-ci/committed) to
prevent these from being merged.

## Configuration

### Sources

Configuration is read from the following (in precedence order):
- [`git -c`](https://git-scm.com/docs/git#Documentation/git.txt--cltnamegtltvaluegt)
- [`GIT_CONFIG`](https://git-scm.com/docs/git-config#Documentation/git-config.txt-GITCONFIGCOUNT)
- `$REPO/.git/config`
- `$REPO/.gitconfig`
- [Other `.gitconfig`](https://git-scm.com/docs/git-config#FILES)

### Config Fields

| Field                  | Argument | Format                    | Description |
|------------------------|----------|---------------------------|-------------|
| stack.protected-branch | \-       | multivar of globs          | Branch names that match these globs (`.gitignore` syntax) are considered protected branches |
| stack.stack            | --stack  | "current", "dependents", "descendants", "all" | Which development branch-stacks to operate on |
| stack.push-remote      | \-       | string                     | Development remote for pushing local branches |
| stack.pull-remote      | \-       | string                     | Upstream remote for pulling protected branches |
| stack.show-format      | --format | "silent", "brief", "full"  | How to show the stacked diffs at the end |
| stack.show-stacked     | \-       | bool                       | Show branches as stacked on top of each other, where possible |
| stack.auto-fixup       | --fixup  | "ignore", "move", "squash" | Default fixup operation with `--rebase` |
