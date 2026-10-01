# deepgram-dictation

## Rule: create a ticket before every change

Always create a GitHub issue before you change the code or the docs. No change without a ticket.

1. Check for an existing issue: `gh issue list --repo rawasmhd/deepgram-dictation`.
2. If there is none, create one. Say what changes and why. Use the matching label (`enhancement`, `bug`, `documentation`).
3. Do the work on a branch for that issue.
4. Refer to the issue in the pull request, for example `Closes #8`.

If a change is part of a larger piece of work, link it to the tracking issue (for example #12 for the Rust rewrite).
