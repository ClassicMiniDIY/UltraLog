# Privacy & Analytics

UltraLog respects your privacy while collecting minimal, anonymous usage data to help improve the application.

---

## Overview

Starting with version 1.5.1, UltraLog includes optional anonymous analytics powered by [PostHog](https://posthog.com/). This data helps the development team understand how the application is used and prioritize new features.

**Key Privacy Principles:**

- **No personal data** is ever collected
- **No log file content** is ever transmitted
- **No file names or paths** are sent
- All data is **anonymous** and cannot be traced to you
- Analytics are used solely to **improve UltraLog**

---

## What Data Is Collected

UltraLog collects the following anonymous usage events:

| Event                     | Data Collected                              | Purpose                                  |
| ------------------------- | ------------------------------------------- | ---------------------------------------- |
| `app_started`             | App version, platform (Windows/macOS/Linux) | Track active installations               |
| `file_loaded`             | ECU type (e.g., "Haltech"), file size in KB | Understand which ECU formats are popular |
| `channel_selected`        | Number of channels selected                 | Understand typical usage patterns        |
| `chart_exported`          | Export format (PNG or PDF)                  | Track feature usage                      |
| `tool_switched`           | Tool name (Log Viewer or Scatter Plot)      | Understand which features are used       |
| `playback_started`        | Playback speed multiplier                   | Track playback feature usage             |
| `colorblind_mode_toggled` | Enabled/disabled                            | Track accessibility feature usage        |
| `update_checked`          | Whether update was available                | Track update adoption                    |

### Common Properties

Every event includes:
- **App version** (e.g., "1.5.1")
- **Platform** (windows, macos, or linux)
- **Anonymous session ID** (random UUID, not persistent)

---

## What Is NOT Collected

UltraLog explicitly **does not** collect:

- Your name, email, or any personal information
- IP addresses (PostHog is configured not to store these)
- File names or file paths
- Log file contents or data values
- Channel names from your ECU
- Screenshots or visual data
- Crash reports or error logs
- System specifications beyond OS type
- Location data
- Any identifiable information

---

## How Data Is Stored

Analytics data is sent to PostHog's servers using HTTPS encryption:

- **Provider**: [PostHog](https://posthog.com/)
- **Data Center**: PostHog Cloud (US)
- **Retention**: 90 days
- **Encryption**: TLS 1.3 in transit
- **Access**: Only the UltraLog development team

---

## Session Identification

Each time you launch UltraLog, a new random UUID is generated as your session ID. This ID:

- Is **not** stored on disk
- Is **different** every time you launch the app
- Cannot be used to identify you across sessions
- Cannot be linked to any personal information

This means even PostHog cannot track you across different sessions or identify you as a returning user.

---

## Technical Implementation

Analytics are implemented using the [posthog-rs](https://crates.io/crates/posthog-rs) Rust crate:

```rust
// Events are sent in background threads to avoid blocking the UI
// Errors are silently ignored - analytics never affect app functionality
std::thread::spawn(move || {
    let _ = posthog_rs::capture(event);
});
```

Key implementation details:

- Events are **non-blocking** (sent in background threads)
- Failed events are **silently dropped** (no retries, no error messages)
- Analytics code is **isolated** and cannot access your log data
- The app functions normally even if analytics fail

---

## Why Collect Analytics?

Anonymous usage data helps the UltraLog team:

1. **Prioritize ECU support** - Know which formats are most requested
2. **Focus development** - Understand which features are actually used
3. **Fix issues** - Identify common error scenarios
4. **Improve accessibility** - Track adoption of accessibility features
5. **Plan releases** - Understand update adoption rates

---

## Your Rights

As analytics are completely anonymous with no persistent identifiers:

- There is no personal data to delete
- There is no account to manage
- Sessions cannot be linked together
- You cannot be identified from the collected data

---

## Disabling Analytics

Currently, analytics cannot be disabled through the UI. If you have strong privacy concerns:

1. **Network blocking**: Block outbound connections to `app.posthog.com`
2. **Firewall rules**: Add a firewall rule for the PostHog domain
3. **Build from source**: Remove the analytics module and recompile

A UI toggle for disabling analytics is being considered for a future release.

---

## Contact

If you have questions about privacy or data collection:

- **GitHub Issues**: [Open an issue](https://github.com/SomethingNew71/UltraLog/issues)
- **Discussions**: [GitHub Discussions](https://github.com/SomethingNew71/UltraLog/discussions)

---

## Changes to This Policy

This privacy policy may be updated as UltraLog evolves. Changes will be documented in release notes and this page will be updated accordingly.

**Last Updated**: December 2024

---

## See Also

- [[FAQ]] - Frequently asked questions
- [[Getting-Started]] - Quick start guide
- [PostHog Privacy Policy](https://posthog.com/privacy)
