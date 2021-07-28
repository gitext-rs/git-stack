# `git-stack` Reference

## Configuration

### Sources

Configuration is read from the following (in precedence order):
- `$REPO/.git/config`
- `$REPO/.gitconfig`

### Config Fields

| Field                  | Argument | Format                    | Description |
|------------------------|----------|---------------------------|-------------|
| stack.protected-branch | \-       | multivar of globs         | Branch names that match these globs (`.gitignore` syntax) are considered protected branches |
| stack.branch           | --branch | "current", "dependents"   | Which development branches to operate on |
| stack.pull-remoate     | \-       | string                    | Upstream remote for pulling protected branchesWhich development branches to operate on |
| stack.show-format      | --format | "silent", "brief", "full" | How to show the stacked diffs at the end |
| stack.show-stacked     | \-       | bool                      | Show branches as stacked on top of each other, where possible |
