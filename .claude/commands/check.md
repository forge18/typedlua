---
name: check
description: Run typecheck, linting, and unit tests - fix issues until all pass
---

Run the full development check workflow for this Rust project. Execute these steps in order:

1. **Format Check** (`cargo fmt --check`)
   - If fails: Run `cargo fmt` to auto-fix
   - Re-run check to confirm

2. **Type Check** (`cargo check`)
   - If fails: Analyze errors and fix code
   - Re-run until passes

3. **Linting** (`cargo clippy -- -D warnings`)
   - If fails: Fix clippy warnings
   - Re-run until passes

4. **Unit Tests** (`cargo test`)
   - If fails: Analyze failures and fix tests or code
   - Re-run until all tests pass

**Continue iterating on each step until it passes before moving to the next step.**

**Final Report:**
Once all checks pass, provide:
- ✅ Summary of what was fixed
- 📊 Test results (passed/total)
- ⏱️ Total time taken
- 💡 Any remaining recommendations

**Critical:**
- DO NOT skip to the next step if current step fails
- DO NOT stop until all 4 steps pass
- Show output of failed commands so issues are clear
- Fix issues one at a time, re-running after each fix
- All warnings and errors need to be fixed. It is not acceptable to have any warnings or errors when completed. 