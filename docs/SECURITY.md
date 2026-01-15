# TypedLua Security

**Version:** 1.0
**Last Updated:** 2026-01-13

## Table of Contents

- [Overview](#overview)
- [Threat Model](#threat-model)
- [Security Architecture](#security-architecture)
- [Input Validation](#input-validation)
- [Code Generation Security](#code-generation-security)
- [Dependency Security](#dependency-security)
- [Development Security](#development-security)
- [Reporting Security Issues](#reporting-security-issues)

---

## Overview

TypedLua is a compiler that transforms typed Lua source code into executable Lua. As a development tool, security considerations focus on preventing vulnerabilities in generated code, protecting developer environments, and maintaining secure development practices.

### Security Principles

1. **Defense in Depth** - Multiple layers of protection
2. **Least Privilege** - Minimal permissions required
3. **Secure by Default** - Safe defaults in configuration
4. **Fail Securely** - Errors don't leak sensitive information
5. **Auditability** - Security-relevant events are logged

---

## Threat Model

### In Scope

TypedLua security considerations include:

1. **Malicious Input Files**
   - Crafted source files designed to exploit parser/compiler
   - Files with malicious content in comments or strings
   - Extremely large or deeply nested structures (DoS)

2. **Configuration Injection**
   - Malicious `tlconfig.yaml` files
   - Command-line argument injection
   - Environment variable manipulation

3. **Code Injection via Generated Code**
   - Template literals with unsanitized content
   - String interpolation vulnerabilities
   - Eval-like constructs in generated Lua

4. **Information Disclosure**
   - Leaking file paths in error messages
   - Exposing sensitive data in diagnostics
   - Source maps revealing proprietary code

5. **Supply Chain**
   - Compromised dependencies
   - Malicious LSP extensions
   - Tampered build artifacts

### Out of Scope

The following are explicitly **not** security boundaries:

1. **Trusted Code Execution** - TypedLua compiles code the user has written or explicitly chosen to compile. We assume the source files are trusted.

2. **Sandbox Escape** - TypedLua does not run untrusted code. Generated Lua is executed in the user's chosen environment (which may or may not be sandboxed).

3. **Lua Runtime Security** - Securing the Lua runtime environment is the responsibility of the deployment platform, not TypedLua.

---

## Security Architecture

### Isolation and Sandboxing

TypedLua operates as a **non-sandboxed compiler** that:

- Reads files from disk (within project directory)
- Writes compiled output to disk
- Does NOT execute arbitrary code during compilation
- Does NOT make network requests (except for LSP over local socket)

**No Dynamic Code Execution:**

The compiler never uses `eval`, `exec`, or similar constructs. All code generation is template-based and statically analyzable.

### Privilege Separation

```
┌─────────────────────────────────────────────┐
│         User's Operating System             │
│  (Filesystem, Network, Process Management)  │
└──────────────────┬──────────────────────────┘
                   │
         ┌─────────▼──────────┐
         │   TypedLua CLI     │
         │  (User privileges) │
         └─────────┬──────────┘
                   │
    ┌──────────────┼──────────────┐
    ▼              ▼              ▼
┌────────┐   ┌──────────┐   ┌─────────┐
│ Lexer  │   │  Parser  │   │ Type    │
│        │   │          │   │ Checker │
└────────┘   └──────────┘   └─────────┘
                   │
                   ▼
            ┌──────────┐
            │ CodeGen  │
            └──────────┘
                   │
                   ▼
            ┌──────────┐
            │  Output  │
            │  (Lua)   │
            └──────────┘
```

**Key Points:**
- Runs with user privileges (not elevated)
- File access limited by OS permissions
- No special capabilities required
- Does not spawn subprocesses (except for LSP)

---

## Input Validation

### File Path Validation

**All file paths are validated before access:**

```rust
fn validate_file_path(path: &Path) -> Result<(), SecurityError> {
    // 1. Reject absolute paths outside project root
    let canonical = path.canonicalize()?;
    if !canonical.starts_with(&project_root) {
        return Err(SecurityError::PathTraversal);
    }

    // 2. Reject symlinks outside project
    if canonical.is_symlink() {
        let target = fs::read_link(&canonical)?;
        if !target.starts_with(&project_root) {
            return Err(SecurityError::SymlinkEscape);
        }
    }

    // 3. Check file extension
    if !allowed_extensions.contains(&canonical.extension()) {
        return Err(SecurityError::InvalidExtension);
    }

    Ok(())
}
```

**Protections:**
- ✅ Path traversal prevention (`../../etc/passwd`)
- ✅ Symlink escape detection
- ✅ File extension validation
- ✅ Project root boundary enforcement

### Configuration Validation

**`tlconfig.yaml` is validated on load:**

```rust
fn load_config(path: &Path) -> Result<CompilerConfig, ConfigError> {
    // 1. Size limit (prevent DoS)
    let metadata = fs::metadata(path)?;
    if metadata.len() > MAX_CONFIG_SIZE {
        return Err(ConfigError::FileTooLarge);
    }

    // 2. Parse YAML safely (no arbitrary code execution)
    let content = fs::read_to_string(path)?;
    let config: CompilerConfig = serde_yaml::from_str(&content)?;

    // 3. Validate values
    validate_config_values(&config)?;

    Ok(config)
}

fn validate_config_values(config: &CompilerConfig) -> Result<(), ConfigError> {
    // Check include/exclude patterns for path traversal
    for pattern in &config.include {
        if pattern.contains("..") {
            return Err(ConfigError::InvalidPattern);
        }
    }

    // Validate output directory is within project
    if let Some(out_dir) = &config.compiler_options.out_dir {
        validate_output_directory(out_dir)?;
    }

    Ok(())
}
```

**Protections:**
- ✅ File size limits (prevents DoS)
- ✅ No arbitrary code execution in YAML
- ✅ Path traversal in patterns rejected
- ✅ Output directory validation

### Source Code Validation

**Limits on source file complexity:**

```rust
const MAX_FILE_SIZE: usize = 5_000_000;        // 5 MB
const MAX_NESTING_DEPTH: usize = 128;          // Prevent stack overflow
const MAX_IDENTIFIER_LENGTH: usize = 512;
const MAX_STRING_LITERAL_LENGTH: usize = 1_000_000;
```

**Protections:**
- ✅ File size limits (DoS prevention)
- ✅ Nesting depth limits (stack overflow prevention)
- ✅ Identifier length limits (buffer overflow prevention)
- ✅ String literal length limits (memory exhaustion prevention)

---

## Code Generation Security

### No Injection Vulnerabilities

TypedLua generates Lua code using **safe templating** with no string interpolation of user input:

```rust
// SAFE: User input is not interpolated into code
fn generate_function_call(&mut self, callee: &str, args: &[Arg]) {
    self.emit_identifier(callee);  // Validated identifier
    self.emit("(");
    for (i, arg) in args.iter().enumerate() {
        if i > 0 {
            self.emit(", ");
        }
        self.generate_expression(arg);  // Recursive generation
    }
    self.emit(")");
}

// UNSAFE (NOT USED):
// self.emit(format!("{}({})", callee, args)); // NEVER DO THIS
```

### Template Literal Safety

**Template literals are escaped:**

```rust
fn generate_template_literal(&mut self, parts: &[TemplatePart]) {
    for part in parts {
        match part {
            TemplatePart::String(s) => {
                // Escape special characters
                let escaped = escape_lua_string(s);
                self.emit(&escaped);
            }
            TemplatePart::Expression(expr) => {
                self.emit("tostring(");
                self.generate_expression(expr);
                self.emit(")");
            }
        }
    }
}

fn escape_lua_string(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\0', "\\0")
}
```

### Source Map Safety

**Source maps do not expose sensitive information:**

- ✅ Only relative paths (never absolute)
- ✅ No comments from source
- ✅ No type annotations
- ✅ Optional generation (can be disabled)

```rust
fn generate_source_map(&self, output: &str) -> SourceMap {
    SourceMap {
        version: 3,
        file: self.output_filename.clone(),
        sources: vec![self.relative_source_path()], // Relative only
        names: vec![],
        mappings: self.encode_mappings(),
        source_content: None, // Don't embed source
    }
}
```

---

## Dependency Security

### Dependency Management

**All dependencies are vetted:**

1. **Minimal Dependencies** - Only essential crates
2. **Trusted Sources** - Only from crates.io
3. **Version Pinning** - Lock file committed
4. **Regular Audits** - `cargo audit` in CI/CD

**Current Dependencies:**

| Crate | Purpose | Audit Status |
|-------|---------|--------------|
| `thiserror` | Error handling | ✅ Trusted |
| `anyhow` | Error propagation | ✅ Trusted |
| `serde` | Serialization | ✅ Trusted |
| `serde_yaml` | Config parsing | ✅ Trusted |
| `clap` | CLI parsing | ✅ Trusted |
| `bumpalo` | Arena allocation | ✅ Trusted |
| `lsp-server` | LSP protocol | ✅ Trusted |

### Supply Chain Protection

**CI/CD pipeline includes:**

```yaml
# .github/workflows/security.yml
- name: Audit dependencies
  run: cargo audit

- name: Check for outdated dependencies
  run: cargo outdated --exit-code 1

- name: Verify reproducible builds
  run: cargo build --release --locked
```

### Vulnerability Disclosure

**Process for handling dependency vulnerabilities:**

1. CI automatically runs `cargo audit` on every push
2. Dependabot creates PRs for security updates
3. Security updates are prioritized and merged ASAP
4. Release notes mention fixed vulnerabilities

---

## Development Security

### Git Hooks

**Pre-commit hook enforces security practices:**

```bash
# .git/hooks/pre-commit

# 1. Prevent committing secrets
if grep -rE '(API_KEY|SECRET|PASSWORD|TOKEN)=[A-Za-z0-9+/]{20,}' .; then
    echo "ERROR: Potential secret detected"
    exit 1
fi

# 2. Prevent debug macros
if grep -r 'dbg!' --include='*.rs' .; then
    echo "ERROR: dbg!() found"
    exit 1
fi

# 3. Format and lint
cargo fmt --check
cargo clippy -- -D warnings
```

**Configuration:** `.git/hooks/pre-commit-config.json`

```json
{
  "custom_checks": [
    {
      "name": "no-dbg-macro",
      "command": "grep -q 'dbg!' ",
      "fail_on_match": true,
      "error_message": "Found dbg!() macro - remove before committing",
      "file_patterns": ["*.rs"]
    }
  ],
  "file_size_limits": {
    "enabled": true,
    "max_file_size_kb": 5000
  }
}
```

### Secrets Management

**The following files are gitignored:**

```gitignore
# Environment files
.env
.env.local
*.pem
*.key
credentials.json

# IDE files (may contain local configs)
.vscode/
.idea/

# Build artifacts
target/
```

**Guidelines:**
- ❌ Never commit API keys or passwords
- ❌ Never commit private keys or certificates
- ❌ Never commit credentials.json or .env files
- ✅ Use environment variables for secrets
- ✅ Document required env vars in README

### Code Review Requirements

**All code changes must:**

1. ✅ Pass automated security checks
2. ✅ Be reviewed by at least one maintainer
3. ✅ Include tests for security-relevant changes
4. ✅ Update docs if security posture changes

---

## Security Best Practices for Contributors

### DO:
- ✅ Validate all external input (files, CLI args, env vars)
- ✅ Use Result<T, E> for error handling (never panic)
- ✅ Limit recursion depth (prevent stack overflow)
- ✅ Set size limits on data structures (prevent DoS)
- ✅ Escape output when generating code
- ✅ Use cargo audit regularly
- ✅ Keep dependencies up to date
- ✅ Review diffs carefully before committing

### DON'T:
- ❌ Trust user input without validation
- ❌ Use unsafe Rust without justification
- ❌ Interpolate user input into generated code
- ❌ Log sensitive information
- ❌ Commit secrets or credentials
- ❌ Ignore compiler warnings
- ❌ Use deprecated or vulnerable dependencies

---

## Known Security Considerations

### 1. Compiler Bombs

**Issue:** Malicious input can cause excessive compilation time.

**Example:**
```lua
type T1 = [string, string]
type T2 = [T1, T1]
type T3 = [T2, T2]
-- ... exponential growth
type T20 = [T19, T19]  -- 2^20 = 1M elements
```

**Mitigation:**
- Type expansion depth limit: 128
- Type complexity limit: 10,000 nodes
- Compilation timeout: 60 seconds

### 2. Resource Exhaustion

**Issue:** Large files or deeply nested structures can exhaust memory.

**Mitigation:**
- File size limit: 5 MB
- Nesting depth limit: 128
- Arena size monitoring
- Graceful error on OOM

### 3. Path Traversal

**Issue:** Malicious config could access files outside project.

**Mitigation:**
- All paths validated before access
- Symlinks resolved and checked
- Output directory must be within project root

### 4. Information Disclosure in Errors

**Issue:** Error messages might reveal sensitive paths.

**Mitigation:**
- Use relative paths in diagnostics
- Sanitize error messages before display
- Option to redact file paths (`--no-file-paths`)

---

## Security Testing

### Automated Security Tests

```rust
#[test]
fn test_path_traversal_prevention() {
    let config = CompilerConfig::default();
    let result = compile_file(&config, "../../etc/passwd");
    assert!(result.is_err());
}

#[test]
fn test_nesting_depth_limit() {
    let source = generate_deeply_nested_expression(200);
    let result = compile_source(&source);
    assert!(matches!(result.unwrap_err(), Error::NestingTooDeep));
}

#[test]
fn test_no_code_injection() {
    let source = r#"
        const evil = "'; os.execute('rm -rf /'); --"
        print(evil)
    "#;
    let lua_code = compile_source(source).unwrap();
    assert!(!lua_code.contains("os.execute"));
}
```

### Fuzzing

**Fuzzing with cargo-fuzz:**

```rust
// fuzz/fuzz_targets/lexer.rs
#![no_main]
use libfuzzer_sys::fuzz_target;
use typedlua_core::Lexer;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let _ = Lexer::new(s, diagnostics.clone()).tokenize();
    }
});
```

**Run fuzzing:**
```bash
cargo fuzz run lexer -- -max_len=10000 -timeout=5
```

---

## Reporting Security Issues

### Responsible Disclosure

**If you discover a security vulnerability in TypedLua:**

1. **DO NOT** open a public GitHub issue
2. **DO** email security@typedlua.dev with:
   - Description of the vulnerability
   - Steps to reproduce
   - Potential impact assessment
   - Suggested fix (if known)

3. Allow 90 days for response and patching before public disclosure

### Response Process

1. **Acknowledgment** - Within 48 hours
2. **Assessment** - Severity rating within 7 days
3. **Fix Development** - Patch created ASAP
4. **Coordinated Disclosure** - Public announcement with patch
5. **Credit** - Reporter credited in release notes (if desired)

### Security Updates

**Critical vulnerabilities:**
- Patch released immediately
- Security advisory published
- All users notified via GitHub and mailing list

**Non-critical vulnerabilities:**
- Included in next regular release
- Mentioned in changelog

---

## Security Checklist for Releases

Before each release:

- [ ] Run `cargo audit` and address all issues
- [ ] Run `cargo outdated` and update dependencies
- [ ] Review all changed files for secrets
- [ ] Test with fuzzer for 1 hour minimum
- [ ] Verify pre-commit hooks are enforced
- [ ] Update SECURITY.md if security posture changed
- [ ] Sign release artifacts
- [ ] Generate SHA256 checksums for binaries

---

## References

- [OWASP Secure Coding Practices](https://owasp.org/www-project-secure-coding-practices-quick-reference-guide/)
- [Rust Security Best Practices](https://anssi-fr.github.io/rust-guide/)
- [Supply Chain Security](https://slsa.dev/)
- [Vulnerability Disclosure Policy](https://cheatsheetseries.owasp.org/cheatsheets/Vulnerability_Disclosure_Cheat_Sheet.html)

---

## Appendix: Security Contact

**Security Email:** security@typedlua.dev
**PGP Key:** Available at https://typedlua.dev/security.asc
**Security Policy:** https://github.com/forge18/typed-lua/security/policy

---

**Version:** 1.0
**Contributors:** TypedLua Security Team
**License:** MIT
