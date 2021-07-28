# Related Stacking Tools

## `ghstack`

[Website](https://github.com/ezyang/ghstack)

Pros:
- Authors can upload multiple PRs at once with each PR showing only the commits relevant for it.

Cons:
- Custom config file, rather than `.gitconfig`
- Incompatible with fork workflow / requires upstream access
  - Manage custom branches
  - Merge from `ghstack`
- Incompatible with host-side merge tools (auto-merge, merge queues, etc) and branch-protections
- Leaves behind stale branches in upstream, requiring custom cleanup
- Requires Python runtime / virtualenv

## `gh-stack`

[Website](https://github.com/timothyandrew/gh-stack)

Pros:
- Updates PR summary with other PRs in the stack

Cons:
- Requires each commit start with an identifier, grouping by identifier into a PR
  - `git-stack` relies on branches (multi-commit PRs) and
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

## `git-branchless`

[Website](https://git.sr.ht/~krobelus/git-branchless)

Cons:
- Requires each commit start with an identifier, grouping by identifier into a PR
  - `git-stack` relies on branches (multi-commit PRs) and
     ["fixup" commits (auto-squashing)](https://thoughtbot.com/blog/autosquashing-git-commits)
