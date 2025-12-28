# Security Policy

## Supported Versions

The following versions of UltraLog receive security updates:

| Version | Supported          |
| ------- | ------------------ |
| 1.x.x   | :white_check_mark: |
| < 1.0   | :x:                |

We recommend always using the latest release version.

---

## Reporting a Vulnerability

We take security seriously. If you discover a security vulnerability in UltraLog, please report it responsibly.

### How to Report

**Please do NOT report security vulnerabilities through public GitHub issues.**

Instead, report vulnerabilities by:

1. **Email:** Send details to the maintainer privately
2. **GitHub Security Advisories:** Use [GitHub's private vulnerability reporting](https://github.com/SomethingNew71/UltraLog/security/advisories/new)

### What to Include

When reporting a vulnerability, please include:

- **Description** of the vulnerability
- **Steps to reproduce** the issue
- **Potential impact** of the vulnerability
- **Suggested fix** (if you have one)
- **Your contact information** for follow-up questions

### Response Timeline

| Action | Timeframe |
|--------|-----------|
| Initial acknowledgment | Within 48 hours |
| Status update | Within 7 days |
| Fix timeline estimate | Within 14 days |
| Public disclosure | After fix is released |

### What to Expect

1. **Acknowledgment:** We'll confirm receipt of your report
2. **Investigation:** We'll investigate and assess the vulnerability
3. **Updates:** We'll keep you informed of our progress
4. **Fix:** We'll develop and test a fix
5. **Release:** We'll release the fix and credit you (if desired)
6. **Disclosure:** We'll coordinate public disclosure timing with you

---

## Security Model

### Application Architecture

UltraLog is a **desktop application** that:

- Runs entirely locally on your computer
- Does not connect to any external servers
- Does not transmit any data over the network
- Does not require an internet connection

### Data Handling

| Data Type | Handling |
|-----------|----------|
| Log files | Read-only, processed locally |
| User preferences | Stored in memory only (not persisted) |
| Custom normalizations | Stored in memory only |
| Exported files | Written to user-specified locations |

### File Operations

UltraLog only performs file operations that you explicitly request:

- **Reading:** Only files you select via file dialog or drag-and-drop
- **Writing:** Only when you export to PNG/PDF at your chosen location

### No Telemetry

UltraLog does not:

- Collect usage analytics
- Send crash reports
- Phone home for updates
- Track user behavior

---

## Security Considerations

### Input Validation

- **File parsing:** All parsers validate input data and handle malformed files gracefully
- **Path handling:** File paths are validated before operations
- **Memory safety:** Rust's memory safety guarantees prevent buffer overflows and related vulnerabilities

### Dependencies

We regularly update dependencies to address known vulnerabilities:

- Dependencies are pinned in `Cargo.lock`
- Dependabot alerts are monitored
- Security updates are prioritized

### Build Security

- CI/CD runs on GitHub Actions (trusted infrastructure)
- No external build services with code access
- Release binaries are built from tagged commits

---

## Known Limitations

### Unsigned Binaries

Pre-built binaries are not code-signed:

- **Windows:** May trigger SmartScreen warnings
- **macOS:** May require Gatekeeper bypass

This is a distribution convenience issue, not a security vulnerability. Users building from source can verify the code themselves.

### No Auto-Update

UltraLog does not auto-update. Users must manually download new releases. This is intentional:

- Prevents update mechanism as attack vector
- Gives users control over when to update
- Simplifies security model

---

## Best Practices for Users

### Downloading UltraLog

- Always download from official [GitHub Releases](https://github.com/SomethingNew71/UltraLog/releases)
- Verify you're on the correct repository (SomethingNew71/UltraLog)
- Consider building from source for maximum assurance

### File Safety

- Only open log files from trusted sources
- Be cautious with log files from unknown origins
- UltraLog cannot execute code from log files, but malformed files could cause crashes

### Staying Updated

- Watch the repository for release notifications
- Update to new versions when available
- Check release notes for security-related fixes

---

## Security Updates

Security fixes will be:

1. Released as soon as possible after verification
2. Documented in release notes (after public disclosure)
3. Announced via GitHub releases

### Past Security Issues

No security vulnerabilities have been reported to date.

---

## Scope

This security policy covers:

- The UltraLog application code
- Official release binaries
- Documentation and wiki

This policy does NOT cover:

- Third-party forks or modifications
- Builds from unofficial sources
- User modifications to the code

---

## Recognition

We appreciate security researchers who help keep UltraLog safe. With your permission, we'll acknowledge your contribution in:

- Release notes
- Security advisories
- This document (Hall of Fame section, when applicable)

---

## Contact

For security concerns, contact the maintainer:

- **GitHub:** [@SomethingNew71](https://github.com/SomethingNew71)
- **Security Advisories:** [Report a vulnerability](https://github.com/SomethingNew71/UltraLog/security/advisories/new)

---

## License

This security policy is part of the UltraLog project and is covered under the GNU Affero General Public License v3.0 (AGPL-3.0).
