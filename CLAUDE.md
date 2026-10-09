# deepgram-dictation

## Rule: create a ticket before every change

Always create a GitHub issue before you change the code or the docs. No change without a ticket.

1. Check for an existing issue: `gh issue list --repo rawasmhd/deepgram-dictation`.
2. If there is none, create one. Say what changes and why. Use the matching label (`enhancement`, `bug`, `documentation`).
3. Do the work on a branch for that issue.
4. Refer to the issue in the pull request, for example `Closes #8`.

If a change is part of a larger piece of work, link it to the tracking issue (for example #12 for the Rust rewrite).

## Rules: several agents at the same time

These rules apply when more than one agent (or person) works on the repository at the same time. They stop conflicts at planning, before the merge.

1. **Scope in every issue.** The issue lists the files or folders it changes, the files it must not touch, the issues it depends on, and what "done" means. Use the task template in `.github/ISSUE_TEMPLATE/task.md`.
2. **One agent per issue.** Each agent takes one open issue. Two agents never share an issue or a branch. Each agent works in its own worktree (`git worktree add ../dictation-<issue> -b <branch> main`), so the files of one agent never change under another.
3. **Start from the latest `main`.** Run `git fetch origin` and branch from `origin/main`. Name the branch `<type>/<issue>-<short-name>`, for example `feature/37-keyterms`.
4. **Change only the ticket.** Do not fix or tidy code outside the scope. If you see another problem, open a new issue and leave the code as it is.
5. **Shared files have one owner at a time.** `Cargo.toml`, `Cargo.lock`, `README.md`, `CLAUDE.md`, `.github/CONTRIBUTING.md` and the docs index are touched by many tasks. If two open issues need the same shared file, run them one after the other, not in parallel. Say which one goes first in the issues.
6. **Update, test, then merge.** If `main` moved after the branch started, merge `origin/main` into the branch (or rebase), run `cargo build` and `cargo test` again, and only then merge the pull request.
7. **Delete the branch after the merge.** The commits stay in `main`. The branch name is only a bookmark.

If a merge still shows a conflict, the agent that merges second resolves it on its own branch. `main` is never edited directly.
