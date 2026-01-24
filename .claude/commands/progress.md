---
name: progress
description: Update TODO.md task list - mark complete, add, or remove tasks
---

# /progress - Update TODO.md Task List

Update the TODO.md task list by marking tasks complete, adding new tasks, or removing obsolete tasks.

## Instructions

You are updating the TODO.md file based on recently completed work or new requirements.

### Critical Rules

1. **NEVER change task text without asking the user first**
   - Task descriptions are canonical - they define the work
   - If you think a task should be reworded, ask the user for permission

2. **NEVER defer or modify tasks without user approval**
   - Don't change priorities, assignments, or dependencies unilaterally
   - Don't move tasks between phases without explicit instruction

3. **Use TODO.md ONLY for task tracking, not documentation**
   - Don't add explanatory notes, rationales, or documentation
   - Keep the file focused on actionable tasks

4. **Mark tasks complete with `[x]` when done**
   - Change `[ ]` to `[x]` for completed tasks
   - Only mark complete if the task is fully finished

5. **Add new tasks when discovered during implementation**
   - Follow the existing format and conventions
   - Place in the appropriate phase/section
   - Include model assignment tag: `[Haiku]`, `[Sonnet]`, or `[Opus]`

6. **Remove tasks only when truly obsolete**
   - If a task is no longer needed due to architecture changes
   - If a task was a duplicate
   - Ask user before removing if uncertain

### Task Format

Follow this exact format for all tasks:

```markdown
- [ ] ⏳ **[ModelTier]** Task description goes here
- [x] ✅ **[ModelTier]** Completed task description
```

Where `ModelTier` is one of: `Haiku`, `Sonnet`, `Opus`

Status icons:
- `⏳` = Pending/In Progress
- `✅` = Completed

### Workflow

1. **Read TODO.md** to understand current state
2. **Identify changes** needed based on completed work or new discoveries
3. **Ask user for confirmation** if:
   - Changing any task text
   - Removing any tasks
   - Modifying task assignments or priorities
   - Moving tasks between phases
4. **Apply changes** only to task status (`[ ]` → `[x]`) and adding new tasks in standard format
5. **Verify** the file still follows the established structure

### Example Updates

**Marking complete:**
```markdown
- [x] ✅ **[Sonnet]** Configure build dependencies (bindgen, cc)
```

**Adding new task:**
```markdown
- [ ] ⏳ **[Sonnet]** Write tests for FFI binding generation
```

**Never do this without asking:**
```markdown
# BAD - Changed task text without permission
- [ ] ⏳ **[Sonnet]** Configure build dependencies and create build script

# BAD - Added documentation
- [ ] ⏳ **[Sonnet]** Configure build dependencies (bindgen, cc)
  Note: This uses vendored headers to avoid requiring Lua at build time
```

### Response Format

After updating TODO.md, provide a brief summary:

```
Updated TODO.md:
- Marked complete: [list tasks]
- Added: [list new tasks]
- Removed: [list removed tasks if any]
```

---

## Usage

```bash
/todo              # Update based on recent work
/todo --complete   # Mark specific task complete (will prompt)
/todo --add        # Add new task (will prompt for details)
```
