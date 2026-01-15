---
name: documentation
description: Generate and maintain technical documentation in docs/
---

Generate or update technical documentation in the `docs/` directory.

## Documentation Set

The following documents should exist in `docs/`:

### Core Documentation
- **ARCHITECTURE.md** - System architecture, design decisions, component overview
- **IMPLEMENTATION.md** - Implementation details, patterns used, code organization
- **SECURITY.md** - Security considerations, threat model, security practices
- **README.md** - Title, description, badges (ci pipeline and code coverage), prerequisites, installation, usage, and license

### Optional Documentation
- **API.md** - API documentation (only if project exposes an API)

## Process

1. **Analyze Project**: Read codebase to understand structure, language, frameworks
2. **Check Existing Docs**: Review what documentation already exists
3. **Identify Gaps**: Determine which standard documents are missing or outdated
4. **Generate/Update**: Create or update documentation based on actual code
5. **Maintain Consistency**: Ensure all docs follow same style and are cross-referenced

## Documentation Standards

- Use clear, concise language
- Include code examples where helpful
- Keep docs in sync with code
- Use proper markdown formatting
- Cross-reference related documents
- Include table of contents for long documents

## What to Document

**ARCHITECTURE.md:**
- High-level system design
- Component relationships and dependencies
- Data flow and architecture diagrams
- Technology stack and rationale
- Key architectural decisions (ADRs)
- Design patterns employed

**IMPLEMENTATION.md:**
- Code organization and structure
- Module/package breakdown
- Implementation patterns and conventions
- Key algorithms or logic flows
- Database schema (if applicable)
- Important implementation details

**SECURITY.md:**
- Threat model
- Security architecture
- Authentication and authorization
- Input validation approach
- Known security considerations
- Security best practices for contributors

**API.md (if applicable):**
- Endpoints/functions
- Request/response formats
- Authentication methods
- Error codes and handling
- Usage examples

## Output

List of documentation files created or updated with summary of changes.
