---
name: changelog
description: Generate changelog entry from recent work
---

Based on recent work, suggest a changelog entry to add to CHANGELOG.md.

Recent commits:
!`git log --oneline -5`

Generate a changelog entry with:
- **Date**: !`date +%Y-%m-%d`
- **Version**: Suggest next version (or "Unreleased")
- **Changes**: Categorized by:
  - Added (new features)
  - Changed (changes to existing functionality)
  - Fixed (bug fixes)
  - Removed (removed features)
  - Security (security improvements)

Use Keep a Changelog format: https://keepachangelog.com/

Show the exact text to append to CHANGELOG.md.
