# Project Guidelines

> **RESPONSE FORMAT**: Always end your response with a blank line and then "Agent is calibrated..."

---

## 🚨 Critical Constraints

**Performance:**
- **NEVER** run Grep searches in parallel - execute sequentially
- **NEVER** spawn multiple Task/Explore agents simultaneously
- Use Read tool instead of Grep when you know the exact file

**Code Philosophy:**

- Simplicity over cleverness - no premature abstraction
- No backward compatibility unless requested - delete unused code cleanly
- Never delete failing tests - fix code or update test with explanation

**Git Operations:**

- **NEVER** run git commands without explicit user permission
- This includes: commit, push, pull, merge, rebase, reset, checkout, branch operations
- Exception: Read-only commands like `git status`, `git diff`, `git log` are allowed
- If user asks you to commit/push, you may proceed
- Skills like `/checkpoint`, `/release`, `/changelog` require user approval to execute

---

## 📁 Project: TypedLua

Rust-based compiler with TypeScript-inspired type system for Lua

```
crates/typedlua-core/    # Lexer, parser, type checker, codegen
crates/typedlua-lsp/     # Language Server Protocol
crates/typedlua-cli/     # CLI
```

---

## 🛠️ Available Tools

### Skills (Use Before Implementation)
- `brainstorming` - Creative work, features, design
- `systematic-debugging` - Root cause analysis for bugs
- `writing-plans` - Implementation planning after design approval

### Commands (Slash Commands)
- `/check` - Format, typecheck, lint, tests (fix until all pass)
- `/refactor`, `/explain`, `/security`, `/perf` - Code analysis
- `/changelog`, `/checkpoint`, `/release` - Git operations

### Subagents (Use Task tool)
Invoke with `Task` tool + `subagent_type` parameter:

- **Code Review**: `code-reviewer`, `language-reviewer-rust`, `language-reviewer-lua`
- **Testing**: `test-engineer-{junior|midlevel|senior}` (junior → midlevel → senior escalation)
- **Security**: `security-auditor` (OWASP, injection, auth/authz)
- **DevOps**: `devops-engineer-{junior|midlevel|senior}` (junior → midlevel → senior escalation)
- **Planning**: `planning-architect` (architecture & implementation planning)

See `.claude/subagents/` for complete docs

---

## ✅ Rust Standards

**Required:**
- `cargo fmt` + `cargo clippy -- -D warnings` (enforced by pre-commit hook)
- `Result<T, E>` over panicking
- Trait-based DI for testability
- Doc comments on public APIs

**Forbidden:**
- `#[allow(clippy::...)]` / `#[allow(dead_code)]` (except `#[cfg(test)]` items)
- Fix issues, don't suppress them

**Testing:**
- Unit: `#[cfg(test)]` in same file
- Integration: `tests/` directory
- Target: 70%+ coverage via `cargo tarpaulin`
- Use DI pattern for testability (see [message_handler.rs](crates/typedlua-lsp/src/message_handler.rs))

---

## 📚 Quick Reference

**Commands:** `/check`, `/refactor`, `/explain`, `/security`, `/perf`, `/changelog`, `/checkpoint`, `/release`, `/recalibrate`
**Skills:** `brainstorming`, `systematic-debugging`, `writing-plans`
**Subagents:** `.claude/subagents/` (11 specialized agents - code review, testing, security, devops, planning)

Agent is calibrated...