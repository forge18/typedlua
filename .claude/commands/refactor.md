---
name: refactor
description: Analyze code for refactoring opportunities
---

Analyze the selected code for refactoring opportunities. Provide specific suggestions for:

1. **Extract Function**: Identify any code blocks that should be extracted into separate functions. Explain what each extracted function should do.

2. **Extract Variable**: Find repeated expressions or complex calculations that should be extracted into named variables for clarity.

3. **Simplify Logic**: Look for complex conditionals, nested loops, or boolean expressions that can be simplified.

4. **Remove Duplication**: Identify any duplicated code that could be consolidated.

5. **Improve Naming**: Suggest better names for variables, functions, or parameters that would make the code more self-documenting.

6. **Reduce Complexity**: If any function is too complex (doing too many things), suggest how to break it down.

For each suggestion:
- Explain WHY the refactoring improves the code
- Show BEFORE and AFTER code examples
- Note any trade-offs or risks

Prioritize suggestions by impact (high impact first).
