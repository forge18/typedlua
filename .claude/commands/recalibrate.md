---
name: recalibrate
description: Re-load all project documentation, skills, subagents, and commands
---

Re-calibrate your context by reading all project documentation and configuration files. This ensures you have the latest guidelines, available tools, and coding standards.

Execute these steps in order:

1. **Read Project Guidelines**
   - Read `/Users/forge18/Repos/typed-lua/CLAUDE.md` (main project documentation)
   - If system-level CLAUDE.md exists, read it as well

2. **Read All Skills** (`.claude/skills/`)
   - List all files in `.claude/skills/` directory
   - Read each skill file to understand available capabilities
   - Summarize skills found

3. **Read All Subagents** (`.claude/subagents/`)
   - List all files in `.claude/subagents/` directory
   - Read each subagent file to understand specialized agents
   - Summarize subagents found (by category if possible)

4. **Read All Commands** (`.claude/commands/`)
   - List all files in `.claude/commands/` directory
   - Read each command file to understand available slash commands
   - Summarize commands found

**Final Report:**
Provide a comprehensive summary of what was loaded:
- 📋 Project guidelines loaded (CLAUDE.md location and key sections)
- 🛠️ Skills available (count and names)
- 🤖 Subagents available (count and categories)
- ⚡ Commands available (count and names)
- ✅ Confirmation that context is up-to-date

**Purpose:**
Use this command when:
- Claude is ignoring the CLAUDE.md file.
