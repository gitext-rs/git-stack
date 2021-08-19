# Related Stacking Tools

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

## Stacked Git

[Website](https://stacked-git.github.io/)

Cons:
- I've looked over the docs multiple times and haven't quite "gotten it" for
  how to use this in a PR workflow.

## `git-branchless`

[Website](https://git.sr.ht/~krobelus/git-branchless)

Cons:
- Requires each commit start with an identifier, grouping by identifier into a PR
  - In contrast, `git-stack` relies on branches (multi-commit PRs) and
     ["fixup" commits (auto-squashing)](https://thoughtbot.com/blog/autosquashing-git-commits)

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
