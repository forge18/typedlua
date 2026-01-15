---
name: security
description: OWASP vulnerability scan
---

Perform a security audit of the selected code. Scan for vulnerabilities based on OWASP Top 10:

## 1. Injection Flaws
- SQL injection (raw queries, string concatenation)
- Command injection (shell commands with user input)
- XSS (unescaped output)
- Path traversal

## 2. Broken Authentication
- Weak password requirements
- Insecure session management
- Hardcoded credentials
- Missing rate limiting

## 3. Sensitive Data Exposure
- Passwords, API keys, tokens in code
- Unencrypted sensitive data
- Data in logs that shouldn't be

## 4. Broken Access Control
- Missing authorization checks
- Insecure direct object references

## 5. Security Misconfiguration
- Debug mode in production
- Verbose error messages

## 6. XSS
- Unescaped user input in HTML
- Unsafe DOM manipulation

## 7. Insecure Deserialization
- Unsafe object deserialization

## 8. Known Vulnerabilities
- Outdated dependencies

## 9. Insufficient Logging
- Missing security event logging

For each issue found:
- **Severity**: Critical / High / Medium / Low
- **Vulnerability**: What it is
- **Location**: Where in code
- **Impact**: What could happen
- **Fix**: How to remediate with code example

List by severity (Critical first).
