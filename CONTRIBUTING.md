# Contributing to UltraLog

Thank you for your interest in contributing to UltraLog! This document provides guidelines and information for contributors.

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Getting Started](#getting-started)
- [How to Contribute](#how-to-contribute)
- [Development Workflow](#development-workflow)
- [Pull Request Process](#pull-request-process)
- [Coding Standards](#coding-standards)
- [Commit Guidelines](#commit-guidelines)
- [Testing](#testing)
- [Documentation](#documentation)
- [Community](#community)

---

## Code of Conduct

By participating in this project, you agree to maintain a respectful and inclusive environment. Please:

- Be respectful and constructive in discussions
- Welcome newcomers and help them get started
- Focus on what's best for the community and project
- Accept constructive criticism gracefully

---

## Getting Started

### Prerequisites

Before contributing, ensure you have:

- [Rust](https://rustup.rs/) (latest stable version)
- [Git](https://git-scm.com/)
- Platform-specific build tools (see below)

### Setting Up Your Development Environment

1. **Fork the repository** on GitHub

2. **Clone your fork:**
   ```bash
   git clone https://github.com/YOUR_USERNAME/UltraLog.git
   cd UltraLog
   ```

3. **Add the upstream remote:**
   ```bash
   git remote add upstream https://github.com/SomethingNew71/UltraLog.git
   ```

4. **Install dependencies:**

   **Linux (Ubuntu/Debian):**
   ```bash
   sudo apt-get update
   sudo apt-get install -y \
       build-essential \
       libxcb-render0-dev \
       libxcb-shape0-dev \
       libxcb-xfixes0-dev \
       libxkbcommon-dev \
       libssl-dev \
       libgtk-3-dev \
       libglib2.0-dev \
       libatk1.0-dev \
       libcairo2-dev \
       libpango1.0-dev \
       libgdk-pixbuf2.0-dev
   ```

   **macOS:**
   ```bash
   xcode-select --install
   ```

   **Windows:**
   - Install [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/)
   - Select "Desktop development with C++"

5. **Verify the build works:**
   ```bash
   cargo build
   cargo test
   ```

---

## How to Contribute

### Types of Contributions

We welcome many types of contributions:

| Type | Description |
|------|-------------|
| **Bug Fixes** | Fix reported issues |
| **New Features** | Add new functionality |
| **ECU Support** | Add support for new ECU formats |
| **Documentation** | Improve README, wiki, or code comments |
| **Tests** | Add or improve test coverage |
| **Performance** | Optimize parsing or rendering |
| **Accessibility** | Improve accessibility features |
| **Translations** | Help with internationalization |

### Finding Issues to Work On

- Look for issues labeled [`good first issue`](https://github.com/SomethingNew71/UltraLog/labels/good%20first%20issue) for beginner-friendly tasks
- Check [`help wanted`](https://github.com/SomethingNew71/UltraLog/labels/help%20wanted) for issues where we need community help
- Feel free to propose new features by opening an issue first

### Before Starting Work

1. **Check existing issues** - Someone may already be working on it
2. **Open an issue** - Discuss your proposed changes before starting significant work
3. **Get feedback** - For large changes, wait for maintainer feedback before investing time

---

## Development Workflow

### 1. Create a Branch

Always create a new branch for your work:

```bash
# Update your main branch
git checkout main
git pull upstream main

# Create a feature branch
git checkout -b feature/your-feature-name
```

Branch naming conventions:
- `feature/` - New features
- `fix/` - Bug fixes
- `docs/` - Documentation changes
- `refactor/` - Code refactoring
- `test/` - Test additions/changes

### 2. Make Your Changes

- Write clean, readable code
- Follow the [coding standards](#coding-standards)
- Add tests for new functionality
- Update documentation as needed

### 3. Test Your Changes

```bash
# Run all tests
cargo test

# Check formatting
cargo fmt --all -- --check

# Run lints
cargo clippy -- -D warnings

# Build release to ensure it compiles
cargo build --release
```

### 4. Commit Your Changes

Follow the [commit guidelines](#commit-guidelines):

```bash
git add .
git commit -m "feat: add support for NewECU format"
```

### 5. Keep Your Branch Updated

Regularly sync with upstream:

```bash
git fetch upstream
git rebase upstream/main
```

### 6. Push and Create PR

```bash
git push origin feature/your-feature-name
```

Then open a Pull Request on GitHub.

---

## Pull Request Process

### Before Submitting

- [ ] Code compiles without errors (`cargo build`)
- [ ] All tests pass (`cargo test`)
- [ ] Code is formatted (`cargo fmt`)
- [ ] No clippy warnings (`cargo clippy -- -D warnings`)
- [ ] Documentation is updated if needed
- [ ] Commit messages follow guidelines

### PR Description Template

When creating a PR, please include:

```markdown
## Description
Brief description of what this PR does.

## Type of Change
- [ ] Bug fix (non-breaking change that fixes an issue)
- [ ] New feature (non-breaking change that adds functionality)
- [ ] Breaking change (fix or feature that would cause existing functionality to change)
- [ ] Documentation update

## Related Issues
Fixes #(issue number)

## Testing
Describe how you tested your changes.

## Screenshots (if applicable)
Add screenshots for UI changes.

## Checklist
- [ ] I have read the CONTRIBUTING guidelines
- [ ] My code follows the project's coding standards
- [ ] I have added tests for my changes
- [ ] All new and existing tests pass
- [ ] I have updated documentation as needed
```

### Review Process

1. **Automated checks** - CI must pass (build, test, lint, format)
2. **Code review** - A maintainer will review your code
3. **Feedback** - Address any requested changes
4. **Approval** - Once approved, a maintainer will merge

### After Merge

- Delete your feature branch
- Update your local main branch
- Celebrate your contribution! 🎉

---

## Coding Standards

### Rust Style

- Follow standard Rust conventions
- Use `rustfmt` for formatting
- Address all `clippy` warnings

### Formatting

Run before committing:

```bash
cargo fmt --all
```

### Linting

Ensure no warnings:

```bash
cargo clippy -- -D warnings
```

### Naming Conventions

| Type | Convention | Example |
|------|------------|---------|
| Functions | snake_case | `parse_log_file` |
| Variables | snake_case | `channel_count` |
| Types/Structs | PascalCase | `LoadedFile` |
| Traits | PascalCase | `Parseable` |
| Constants | SCREAMING_SNAKE | `MAX_CHANNELS` |
| Modules | snake_case | `haltech_parser` |

### Code Organization

- Keep functions focused and small
- Use meaningful names
- Add comments for complex logic
- Group related functionality into modules

### Error Handling

- Use `anyhow` for error propagation in parsers
- Use `thiserror` for custom error types
- Provide helpful error messages
- Handle errors gracefully in UI (toast notifications)

---

## Commit Guidelines

### Commit Message Format

```
type(scope): short description

Longer description if needed. Explain the what and why,
not the how (the code shows how).

Fixes #123
```

### Types

| Type | Description |
|------|-------------|
| `feat` | New feature |
| `fix` | Bug fix |
| `docs` | Documentation only |
| `style` | Formatting, no code change |
| `refactor` | Code restructuring |
| `perf` | Performance improvement |
| `test` | Adding/updating tests |
| `chore` | Maintenance tasks |

### Scope (Optional)

The area of the codebase:
- `parser` - Parsing code
- `ui` - User interface
- `export` - Export functionality
- `units` - Unit conversion
- `normalize` - Field normalization

### Examples

```
feat(parser): add MegaSquirt log format support

fix(ui): prevent crash when loading empty files

docs: update installation instructions for Linux

refactor(chart): extract LTTB algorithm to separate function

test(parser): add tests for Haltech edge cases
```

### Guidelines

- Keep the first line under 72 characters
- Use imperative mood ("add" not "added" or "adds")
- Don't end the first line with a period
- Reference issues when applicable

---

## Testing

### Running Tests

```bash
# Run all tests
cargo test

# Run specific test
cargo test test_name

# Run tests with output
cargo test -- --nocapture
```

### Writing Tests

- Add tests for new functionality
- Test edge cases and error conditions
- Place unit tests in the same file as the code
- Place integration tests in the `tests/` directory

### Test File Locations

- Unit tests: Inline in source files
- Integration tests: `tests/` directory
- Sample data: `exampleLogs/` directory

### Example Test

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_haltech_header() {
        let header = "%DataLog%";
        assert!(is_haltech_format(header));
    }

    #[test]
    fn test_unit_conversion_celsius() {
        let kelvin = 300.0;
        let celsius = kelvin_to_celsius(kelvin);
        assert!((celsius - 26.85).abs() < 0.01);
    }
}
```

---

## Documentation

### Code Comments

- Add doc comments (`///`) for public APIs
- Explain *why*, not *what* (code shows what)
- Keep comments up to date with code changes

### README Updates

Update README.md when:
- Adding new features
- Changing installation steps
- Modifying supported formats

### Wiki Updates

Update wiki pages for:
- New ECU format support
- New configuration options
- Changed workflows

---

## Adding ECU Support

If you're adding support for a new ECU format:

### 1. Create an Issue First

Discuss the new format before implementing. Include:
- ECU system name and model
- File format details
- Sample files (if possible)

### 2. Implementation Steps

1. Add ECU type to `EcuType` enum in `src/parsers/types.rs`
2. Create parser module `src/parsers/newecu.rs`
3. Implement the `Parseable` trait
4. Add format detection logic
5. Add field normalizations in `src/normalize.rs`
6. Add sample files to `exampleLogs/`
7. Update documentation

### 3. Testing Requirements

- Include sample log files
- Add parser tests
- Test with various file sizes
- Verify all channel types parse correctly

---

## Community

### Getting Help

- **Questions**: Open a [Discussion](https://github.com/SomethingNew71/UltraLog/discussions)
- **Bugs**: Open an [Issue](https://github.com/SomethingNew71/UltraLog/issues)
- **Features**: Open an Issue to discuss first

### Recognition

Contributors are recognized in:
- Release notes
- Contributors list
- Commit history

---

## License

By contributing to UltraLog, you agree that your contributions will be licensed under the GNU Affero General Public License v3.0 (AGPL-3.0).

---

Thank you for contributing to UltraLog! Your efforts help make this tool better for everyone in the automotive tuning community.
