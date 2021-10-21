# Design

The goal of `git-stack` is to streamline the PR workflow.

Requirements:
- Prioritize the PR workflow
- Interoperate with non-`git-stack` PR workflows
  - Allow gradual adoption
  - Allow dropping down to more familiar, widely documented commands (i.e. I can apply answers from stack overflow)

Example: When pushing a branch and creating a PR, people general mark the
remote branch as the upstream for their branch, allowing them to do a simple
`git push` in the future.  We need to set this for the user and can't use it like 
[depot-tools](https://commondatastorage.googleapis.com/chrome-infra-docs/flat/depot_tools/docs/html/depot_tools_tutorial.html)
which simplifies some of `git-stack`s work by having the parent branch be the
upstream.
