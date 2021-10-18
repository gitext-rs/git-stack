# `git-stack` Reference

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
| stack.fixup            | \-       | "ignore", "move", "squash" | Default fixup operation with `--rebase` |
