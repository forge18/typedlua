---
name: architecture
description: Architecture and design review
---

Perform an architecture and design review of the selected code.

## Architecture Analysis

### 1. Design Patterns
- What patterns are used? (MVC, Repository, Factory, Observer, etc.)
- Are they applied correctly?
- Are there anti-patterns?

### 2. Separation of Concerns
- Are responsibilities clearly separated?
- Is business logic mixed with presentation or data access?
- Are layers/modules properly isolated?

### 3. Dependencies
- Dependency direction (does it follow dependency inversion?)
- Are there circular dependencies?
- Are dependencies too tightly coupled?
- Missing abstractions?

### 4. Scalability Concerns
- How does this scale with increased load?
- Bottlenecks or single points of failure?
- Can components scale independently?

### 5. Maintainability
- How easy is it to modify or extend?
- Is the code following SOLID principles?
- Are there god objects or god methods?
- Clear module boundaries?

### 6. Dependency Injection
- Are dependencies injected (constructor/property injection)?
- Are concrete implementations hidden behind interfaces/abstractions?
- Is there proper use of IoC containers (if applicable)?
- Can dependencies be easily swapped or mocked?

### 7. Testability
- Can components be tested in isolation?
- Are side effects controlled and mockable?
- Is the code following unit testing best practices?
  - Single responsibility per test
  - Arrange-Act-Assert pattern
  - No test interdependencies
  - Fast, isolated, repeatable tests

### 8. Data Flow
- How does data move through the system?
- Is the flow clear and unidirectional?
- Are there hidden data transformations?

### 9. Error Handling Strategy
- Is there a consistent error handling approach?
- Are errors propagated appropriately?
- Recovery mechanisms?

### 10. Security Architecture
- Authentication and authorization strategy?
- Input validation at boundaries?
- Secure defaults and fail-safe mechanisms?
- Protection against common attacks (injection, XSS, CSRF)?

### 11. Performance Architecture
- Database query patterns (N+1 queries)?
- Caching strategy and cache invalidation?
- Resource pooling (connections, threads)?
- Lazy vs eager loading appropriateness?

### 12. Code Smells
- Long parameter lists (> 3-4 parameters)?
- Feature envy (method uses more of another class)?
- Primitive obsession (should be value objects)?
- Data clumps (same data always together)?
- Shotgun surgery (single change requires many edits)?

## Recommendations

For each issue found:
- **Category**: Design Pattern / Coupling / Scalability / etc.
- **Current State**: What's problematic
- **Impact**: Why it matters
- **Suggested Refactoring**: How to improve
- **Trade-offs**: What you gain/lose

Prioritize by architectural impact.
