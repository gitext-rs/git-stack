# Related Stacking Tools

## `arcanist` (`arc`)

[Website](https://secure.phabricator.com/book/phabricator/article/arcanist/)

Pros:
- Rebases each branch when merging
- Show review status of each Diff (Phab's equivalent of PR)
- Nicer status view than `git log`

Cons:
- Coupled to Phabricator which is EOL
- Auto-rebasing doesn't preserve branch relationships (stacks)
- No auto-rebase outside of "landing" a Diff (merging a PR)

## depo-tools

[Website](https://commondatastorage.googleapis.com/chrome-infra-docs/flat/depot_tools/docs/html/depot_tools_tutorial.html)

- `git rebase-update` to pull, rebase, and cleanup merged changes
- `git map` and `git map-branches` for showing branch and commit relationships
- `git reparent-branch` to rebase a tree of branches onto another branch
- `git nav-downstream` / `git nav-upstream` to move between parent / child branches in a stack
  - `git nav-downstream` prompts on ambiguity

Cons:
- Relies on a branch's upstream being set to the parent branch, rather than the remote used for PRs

## `git-branchless`

[Website](https://github.com/arxanas/git-branchless)

Pros:
- `git undo` seems to provide a nice experience!
- `git smartlog`
  - Identifies orphaned commits
  - Nice use of glyphs in visualization
- `git restack`
  - Fixes when a commit is rewritten but dependents weren't updated

Cons:
- Only as reliable as information it can gather through hooks (incompatible with `git-revise` and others)
- Assumes hook installs will append to existing hooks

## Graphite

[Website](https://github.com/screenplaydev/graphite-cli)

Uses refs to track what branches make up a stack

Supports creating PRs for multiple branches in a stack but they don't describe how they do this

Pros:
- Has web dashboard
- Interactive branch checkout
- Direct support for Github PRs
- Has "max days behind trunk"
- Can run a command on each branch in a stack

Cons:
- Has you replace `git` with `gt` with a slightly different interface
- Requires giving access to a third party
- Only supports Github
- Sounds like they require user-prefixes for branches

## `git-machete`

[Website](https://github.com/VirtusLab/git-machete)

Pros:
- Supports going up and down stacks (`go up`, `go down`, `go next`, `go prev`, `go root`)
- Quick way to diff a branch on a stack

Cons:
- Manually managed branch relationships
  - `discover` to get started
  - `add` to edit the file from the command-line

## `git spr`

[Website](https://github.com/ejoffe/spr)

Cons:
- Blackbox: no explanation for how the PRs are stacked or if any relationship data is shown to the user

## `spr`

[Website](https://getcord.github.io/spr/index.html)

Cons:
- Blackbox: no explanation for how the PRs are stacked or if any relationship data is shown to the user

## `ghstack`

[Website](https://github.com/ezyang/ghstack)

Pros:
- Authors can upload multiple PRs at once with each PR showing only the commits relevant for it.

Cons:
- Not integrated into `git` workflow (e.g. custom config file, rather than `.gitconfig`)
- Incompatible with fork workflow / requires upstream access
  - It manage custom branches
  - You must merge from `ghstack`
- Incompatible with host-side merge tools (auto-merge, merge queues, etc) and branch-protections
- Leaves behind stale branches in upstream, requiring custom cleanup
- Requires Python runtime / virtualenv

## `gh-stack`

[Website](https://github.com/timothyandrew/gh-stack)

Pros:
- Updates PR summary with other PRs in the stack

Cons:
- Requires each commit start with an identifier, grouping by identifier into a PR
  - In contrast, `git-stack` relies on branches (multi-commit PRs) and
     ["fixup" commits (auto-squashing)](https://thoughtbot.com/blog/autosquashing-git-commits)

## `git-ps`

[Website](https://github.com/uptech/git-ps)
- [Introduction](https://upte.ch/blog/how-we-should-be-using-git/)
- [Guide](https://github.com/uptech/git-ps/wiki/Guide)

Cons:
- Blackbox: no explanation for how they manage the patch/PR relationship
- Dependent on Swift support for your platform

## Jujutsu

[Website](https://github.com/martinvonz/jj)

Pros:
- When a commit is rewritten, descendants are automatically rebased
- Supports undo, including undo of a past operation
- Simpler CLI than `git` (e.g. no "index")
- Powerful history-editing features, such as for splitting and squashing
  commits, for moving parts of a commit to or from its parent, and for
  editing the contents or commit message of any commit
- First-class conflicts means that conflicts won't prevent rebase, and
  existing conflicts can be rebased or rolled back
- Merge commits are correctly rebased, edited, split, etc.

Cons:
- The working copy cannot be used with `git`, you have to use `jj` instead
- Missing functionality such as `git blame`, `git log <path>`, `git apply`
  - Can work around it by running the `git` commands on the underlying Git
    repository
- Working with multiple remotes requires many manual steps to manage
  branches

## Stacked Git

[Website](https://stacked-git.github.io/)

Cons:
- I've looked over the docs multiple times and haven't quite "gotten it" for
  how to use this in a PR workflow.

## `git-branchstack`

[Website](https://git.sr.ht/~krobelus/git-branchstack)

Cons:
- Requires each commit start with an identifier, grouping by identifier into a PR
  - In contrast, `git-stack` relies on branches (multi-commit PRs) and
     ["fixup" commits (auto-squashing)](https://thoughtbot.com/blog/autosquashing-git-commits)

## `git-series`

[Website](https://github.com/git-series/git-series)

## `git-chain`

[Website](https://github.com/Shopify/git-chain)
- [Rewrite](https://github.com/dashed/git-chain)

Cons:
- Requires manually defining a chain
