---
name: release
description: Version bump + tag workflow
allowed-tools: Bash(git:*), Bash(npm:version), Edit(package.json:*), Edit(CHANGELOG.md:*)
argument-hint: [major|minor|patch]
---

Complete release workflow: update changelog, increment version, create and push tag.

## Current State

Version: !`grep -E '"version"' package.json 2>/dev/null || echo "No package.json found"`
Latest tag: !`git describe --tags --abbrev=0 2>/dev/null || echo "No tags yet"`
Branch: !`git rev-parse --abbrev-ref HEAD`

## Release Type

Release type: $1 (major, minor, or patch)

## Steps

1. **Update CHANGELOG.md**
   - Add entry for this release with today's date
   - Move items from "Unreleased" section
   - Use Keep a Changelog format

2. **Increment Version**
   - Determine new version number based on $1
   - Update package.json (or equivalent version file)
   - Follow semantic versioning

3. **Create Release Commit**
   - Stage CHANGELOG.md and version file
   - Commit with message: "Release v<version>"

4. **Create Git Tag**
   - Create annotated tag: `git tag -a v<version> -m "Release v<version>"`

5. **Push**
   - Push commit: `git push`
   - Push tag: `git push --tags`

Show the exact commands to run for each step.
