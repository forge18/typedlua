---
name: checkpoint
description: WIP commit helper
allowed-tools: Bash(git add:*), Bash(git commit:*), Bash(git status:*)
---

Create a WIP (Work In Progress) checkpoint commit.

Current branch: !`git rev-parse --abbrev-ref HEAD`

Changed files:
!`git status --short`

Generate a commit message that:
1. Starts with "WIP:" prefix
2. Briefly describes what's in progress
3. Is specific enough to understand the state

Then run:
```bash
git add -A
git commit -m "<your generated message>"
```

Show the exact commands to run.
