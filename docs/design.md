# Design

The goal of `git-stack` is to streamline the PR workflow.

Requirements:
- Prioritize the PR workflow
- Interoperate with non-`git-stack` PR workflows
  - Allow gradual adoption
  - Allow dropping down to more familiar, widely documented commands (i.e. I can apply answers from stack overflow)
  - Do not interfere with other tools

Example: When pushing a branch and creating a PR, people general mark the
remote branch as the upstream for their branch, allowing them to do a simple
`git push` in the future.  We need to set this for the user and can't use it like 
[depot-tools](https://commondatastorage.googleapis.com/chrome-infra-docs/flat/depot_tools/docs/html/depot_tools_tutorial.html)
which simplifies some of `git-stack`s work by having the parent branch be the
upstream.

## Defining stacks

A stack is a series of commits with branches on some of the commits that ends
on a commit in a protected branch.  We assume the closest protected branch to
that protected commit is the base.

[Other tools](./comparison.md) rely on external information for defining and
maintaining stacks, including:
- git hooks
- A branch's "upstream"
- A data file
- Identifiers in commits

To meet `git-stack`s goals, we cannot rely on any of these.  `git-stack`
provides some operations, like `--rebase` and `--fixup`, to modify the tree of stacks
without losing relationships.  For when the stack gets messed up outside of
`--rebase` and `--fixup`, a `--repair` will be provided that assumes that
`HEAD` is the core of the stack and fixes what it can (see
[Issue #6](https://github.com/gitext-rs/git-stack/issues/6)).  Outside of that, it
is left to the user to fix the stacks.
