# `git-stack` Reference

## Concepts

### Protected Branch

These are branches like `main` or `v3` that `git-stack` must not modify.  If
there is a matching branch in the `stack.push-remote`, we assume that is the
canonical version of the branch (the one being modified) and we will track the
local branch to that.

`git-stack` finds the best-match protected base branch for each development branch:
- `--pull` will only pull protected bases
- `--rebase` will move development development branches to the latest commit of this protected base

### pull-remote

The remote that contains shared branches you are developing against.  Because
these are shared branches, we do not want to modify their history locally.

### push-remote

The remote that contains your personal branches in preparation for being merged
into a shared branch in the pull-remote.  `git-stack` assumes the local version
is canonical (that no edits are happening in the remote) and that `git-stack`
is free to modify and force-push to this remote.

This may be the same as the `pull-remote` when working directly in the upstream org, rather than on a fork.

## Commands

### `git stack`

Visualizes the branch stacks on top of their protected bases.

Why not `git log --graph --all --oneline --decorate main..HEAD`?
- Doesn't show status as you progress through review
- Fairly verbose
- Have to manually select your base to limit to relevant commits
- Slower because it loads the entire commit graph into memory to sort it

### `git stack --pull`

Pulls your protected branches from the `stack.pull-remote` and then rebases
your development branches on top of their relevant protected branches.

Unlike `--rebase`, this does not perform any "auto" operations.

Note:
- This also performs a fetch of your `stack.push-remote` to prune any removed remotes

Why not `git pull --rebase upstream main`?
- Have to manually select your remote/branch
- Only updates current branch
- Even looping over all branches, the relationship between branches gets
  lost, requiring rebasing branches back on top of each other, making sure
  you do it in a way to avoid conflicts.
- Have to manually delete merged branches
- Only fetches from `upstream`, leaving your deleted `origin` branches lingering locally

### `git stack --rebase`

Rebase development branches on their relevant protected branches.

This performs "auto" operations, like
- `stack.auto-fixup`: see `--fixup`

Why not `git rebase -i --autosquash master`?
- Have to manually select the base
- By default, it will squash the `fixup!` commits.  If this isn't what you
  want, you are likely to defer this until you are ready to squash and you
  won't know of any merge-conflicts that arise from moving the `fixup!` commits.

### `git stack --fixup <action>`

Process [fixup!](https://git-scm.com/docs/git-commit#Documentation/git-commit.txt---fixupamendrewordltcommitgt) commits according to the specified action.

Note:
- This can be used to override `stack.auto-fixup` during a `--rebase`.

### `git stack --repair`

This attempts to clean up stacks
- If you commit directly on a parent stack, this will update the dependent stacks to be on top of that new commit
- If you used `git rebase`, then the stack will be split in two.  This will merge them.

### `git stack --push`

Push all "ready" development branches to your `stack.push-remote`.

A branch is ready if
- It is not stacked on top of any other development branches (see ["How do I stack my PRs in Github"](../README.md#how-do-i-stack-my-prs-in-github))
- It has no [WIP commits](../README.md#when-is-a-commit-considered-wip)

We consider branches with
[`fixup!` commits](https://git-scm.com/docs/git-commit#Documentation/git-commit.txt---fixupamendrewordltcommitgt)
to be ready in case you are wanting reviewers to see some intermediate states.
You can use a tool like [committed](https://github.com/crate-ci/committed) to
prevent these from being merged.

Why not `git push --set-upstream --force-with-lease origin <branch>`?
- A bit verbose to do this right
- Might forget to clean up your branch (e.g. WIP, fixup)

### `git branch-stash`

While `git stash` backs up and restores your working tree, `git branch-stash` backs up and restores the state of all of your branches.

`git-stack` implicitly does a `git branch-stash` whenever modifying the tree.

Why not `git reflog` and manually restoring the branches?
- A lot of manual work to find the correct commit SHAs and adjust the branches to point to them

## Configuration

### Sources

Configuration is read from the following (in precedence order):
- [`git -c`](https://git-scm.com/docs/git#Documentation/git.txt--cltnamegtltvaluegt)
- [`GIT_CONFIG`](https://git-scm.com/docs/git-config#Documentation/git-config.txt-GITCONFIGCOUNT)
- `$REPO/.git/config`
- `$REPO/.gitconfig`
- [Other `.gitconfig`](https://git-scm.com/docs/git-config#FILES)

### Config Fields

| Field                  | Argument | Format                     | Description |
|------------------------|----------|----------------------------|-------------|
| stack.protected-branch | \-       | multivar of globs          | Branch names that match these globs (`.gitignore` syntax) are considered protected branches |
| stack.protect-commit-count | \-   | integer                    | Protect commits that are on a branch with `count`+ commits |
| stack.protect-commit-age | \-     | time delta (e.g. 10days)   | Protect commits that older than the specified time |
| stack.stack            | --stack  | "current", "dependents", "descendants", "all" | Which development branch-stacks to operate on |
| stack.push-remote      | \-       | string                     | Development remote for pushing local branches |
| stack.pull-remote      | \-       | string                     | Upstream remote for pulling protected branches |
| stack.show-format      | --format | "silent", "branches", "branch-commits", "commits", "debug"  | How to show the stacked diffs at the end |
| stack.show-stacked     | \-       | bool                       | Show branches as stacked on top of each other, where possible |
| stack.auto-fixup       | --fixup  | "ignore", "move", "squash" | Default fixup operation with `--rebase` |
| stack.auto-repair      | \-       | bool                       | Perform branch repair with `--rebase` |
