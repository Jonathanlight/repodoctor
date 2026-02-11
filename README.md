# RepoDoctor

A fast CLI tool that diagnoses the health of your repository. It detects your framework, runs 50+ checks across structure, dependencies, configuration, testing, and security, then gives you an actionable health score.

Built in Rust for speed. Single binary, no runtime dependencies.

## Features

- **Auto-detection** of framework (Symfony, Laravel, Flutter, Next.js, Rust, Node.js, Python)
- **50+ rules** across 7 analyzers covering structure, deps, config, testing, security, and documentation
- **Health score** from 0-100 with letter grades (A-F) and per-category breakdown
- **Auto-fix** for common issues (missing directories, `.gitignore`, `.editorconfig`)
- **Reports** in HTML, Markdown, and SVG badge formats
- **CI mode** with configurable exit codes for pipeline integration
- **Framework-specific rules** for Symfony, Flutter, and Next.js projects

## Installation

### From source

```bash
cargo install --path .
```

### Build from repository

```bash
git clone https://github.com/Jonathanlight/repodoctor.git
cd repodoctor
cargo build --release
# Binary at target/release/repodoctor
```

## Quick Start

```bash
# Scan current project
repodoctor scan .

# Initialize config file
repodoctor init

# Auto-fix detected issues
repodoctor fix . --auto

# Generate HTML report
repodoctor report . --format html --badge
```

## Commands

### `scan` - Diagnose your project

```bash
repodoctor scan [PATH] [OPTIONS]
```

| Option | Description |
|--------|-------------|
| `--format <table\|json>` | Output format (default: `table`) |
| `--severity <level>` | Minimum severity to display (`info`, `low`, `medium`, `high`, `critical`) |
| `--ci` | CI mode: exit code 1 if issues exceed threshold |
| `--fail-on <level>` | Severity threshold for CI failure (default: `high`) |

**Example output:**

```
RepoDoctor v0.1.0
────────────────────────────────────────────────────────────────

  Project:  /home/user/my-project
  Detected: Symfony 6.4
  Scan completed in 0.3s

────────────────────────────────────────────────────────────────

  HEALTH SCORE: 72/100 (Grade C)

  Category           Score    Issues   Status
  ──────────────────────────────────────────────────────────
  Structure          85/100   2        Good
  Dependencies       60/100   5        Needs attention
  Configuration      70/100   3        Needs attention
  Testing            45/100   4        Poor
  Security           90/100   1        Good
  Documentation      80/100   1        Good
```

### `fix` - Auto-fix issues

```bash
repodoctor fix [PATH] [OPTIONS]
```

| Option | Description |
|--------|-------------|
| `--dry-run` | Preview fixes without applying them |
| `--auto` | Apply all fixes without prompting |

Supported auto-fixes:
- Create missing directories (`src/`, `tests/`, `src/Controller/`, etc.)
- Create or update `.gitignore` with framework-appropriate entries
- Create `.editorconfig` with standard settings

### `report` - Generate reports

```bash
repodoctor report [PATH] [OPTIONS]
```

| Option | Description |
|--------|-------------|
| `--format <html\|markdown\|json>` | Report format (default: `html`) |
| `--output <FILE>` | Output file path |
| `--badge` | Also generate a health badge SVG |

### `init` - Create config file

```bash
repodoctor init [PATH] [OPTIONS]
```

| Option | Description |
|--------|-------------|
| `--force` | Overwrite existing `.repodoctor.yml` |

Generates a `.repodoctor.yml` with framework-appropriate defaults.

## Configuration

Create a `.repodoctor.yml` at the root of your project (or run `repodoctor init`):

```yaml
# Minimum severity to report (info, low, medium, high, critical)
severity_threshold: low

# Files and rules to ignore
ignore:
  paths:
    - vendor/
    - node_modules/
  rules:
    - DOC-003  # Skip CHANGELOG check
```

## CI/CD Integration

### GitHub Actions

```yaml
name: Repo Health
on: [push, pull_request]

jobs:
  health-check:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install RepoDoctor
        run: cargo install --path .

      - name: Run health scan
        run: repodoctor scan . --ci --fail-on high

      - name: Generate report
        if: always()
        run: repodoctor report . --format html --badge

      - name: Upload report
        if: always()
        uses: actions/upload-artifact@v4
        with:
          name: health-report
          path: |
            repodoctor-report.html
            repodoctor-badge.svg
```

### GitLab CI

```yaml
repo-health:
  script:
    - cargo install --path .
    - repodoctor scan . --ci --fail-on high
```

### Exit codes

| Code | Meaning |
|------|---------|
| `0` | No issues above threshold |
| `1` | Issues found at or above `--fail-on` severity |

## Supported Frameworks

| Framework | Detection | Rules | Auto-fix |
|-----------|-----------|-------|----------|
| **Symfony** | `symfony.lock`, `config/bundles.php` | 20 rules (SYM-*) | Directories, .gitignore |
| **Laravel** | `artisan` | Generic rules | Generic fixes |
| **Flutter** | `pubspec.yaml` | 18 rules (FLT-*) | Directories, .gitignore |
| **Next.js** | `next.config.js/mjs/ts` | 22 rules (NJS-*) | Directories, .gitignore |
| **Rust/Cargo** | `Cargo.toml` | Generic rules | Generic fixes |
| **Node.js** | `package.json` | Generic rules | Generic fixes |
| **Python** | `pyproject.toml`, `requirements.txt` | Generic rules | Generic fixes |

## Analyzers & Rules

### Generic Analyzers (all projects)

#### Structure (STR-*)

| ID | Severity | Title | Auto-fix |
|----|----------|-------|----------|
| STR-001 | High | Missing `src/` directory | Yes |
| STR-002 | Medium | Missing `README.md` | No |
| STR-003 | Medium | Missing `.gitignore` | Yes |
| STR-004 | Low | Missing `LICENSE` file | No |
| STR-005 | Info | Missing `CHANGELOG.md` | No |
| STR-006 | Medium | Deep directory nesting (>6 levels) | No |

#### Dependencies (DEP-*)

| ID | Severity | Title |
|----|----------|-------|
| DEP-001 | Medium | Lock file missing |
| DEP-002 | Low | No dependency file found |
| DEP-003 | Info | Multiple package managers detected |
| DEP-004 | Low | Git dependencies detected |
| DEP-005 | Medium | Dev dependency in production section |

#### Configuration (CFG-*)

| ID | Severity | Title | Auto-fix |
|----|----------|-------|----------|
| CFG-001 | Medium | Missing CI/CD configuration | No |
| CFG-002 | Low | Missing `.editorconfig` | Yes |
| CFG-003 | Medium | Missing `.env` in `.gitignore` | Yes |
| CFG-004 | Low | Missing linter configuration | No |

#### Security (SEC-*)

| ID | Severity | Title | Auto-fix |
|----|----------|-------|----------|
| SEC-001 | Critical | Hardcoded secrets in source code | No |
| SEC-002 | High | `.env` file committed to repository | No |
| SEC-003 | Medium | Sensitive files not in `.gitignore` | Yes |

### Symfony Rules (SYM-*)

| ID | Severity | Title | Auto-fix |
|----|----------|-------|----------|
| SYM-001 | High | Missing `src/Controller/` | Yes |
| SYM-002 | Medium | Missing `src/Entity/` | Yes |
| SYM-003 | Medium | Controllers outside `src/Controller/` | No |
| SYM-004 | Low | Services outside `src/Service/` | No |
| SYM-012 | Critical | `APP_SECRET` has default value | No |
| SYM-013 | Critical | Debug mode in production config | No |
| SYM-020 | High | Outdated Symfony version | No |
| SYM-022 | Low | Missing `symfony/runtime` | No |
| SYM-030 | Medium | Missing `phpunit.xml.dist` | No |
| SYM-031 | High | No `tests/` directory | Yes |
| SYM-032 | High | PHPUnit not in dev dependencies | No |
| SYM-040 | Critical | Hardcoded database credentials | No |
| SYM-041 | Medium | Missing CORS configuration | No |
| SYM-042 | Critical | Unsafe deserialization detected | No |
| SYM-050 | Medium | Missing `.gitignore` entries | Yes |
| SYM-052 | Info | Missing `rector.php` | No |
| SYM-053 | Medium | PHPStan not configured | No |

### Flutter Rules (FLT-*)

| ID | Severity | Title | Auto-fix |
|----|----------|-------|----------|
| FLT-003 | Medium | Business logic in `lib/main.dart` | No |
| FLT-004 | Medium | No clear architecture | No |
| FLT-010 | Low | `pubspec.yaml` missing description | No |
| FLT-011 | High | Outdated Flutter SDK constraint | No |
| FLT-021 | Medium | Dev dependency in dependencies | No |
| FLT-022 | Low | Git dependencies in pubspec | No |
| FLT-030 | High | No widget tests | No |
| FLT-031 | Medium | No integration tests | Yes |
| FLT-032 | High | Missing `flutter_test` dependency | No |
| FLT-041 | High | Insecure HTTP URLs | No |
| FLT-042 | High | Debug flags in release | No |
| FLT-050 | Medium | Missing Android signing config | No |
| FLT-051 | Medium | iOS missing capabilities | No |
| FLT-052 | Low | Missing app icons | No |
| FLT-053 | Medium | Missing `.gitignore` entries | Yes |

### Next.js Rules (NJS-*)

| ID | Severity | Title | Auto-fix |
|----|----------|-------|----------|
| NJS-001 | High | Missing `pages/` or `app/` directory | Yes |
| NJS-002 | Medium | Missing `public/` directory | No |
| NJS-003 | Medium | Missing `components/` directory | Yes |
| NJS-004 | Low | Missing `styles/` directory | Yes |
| NJS-010 | High | Missing Next.js configuration | Yes |
| NJS-011 | Medium | Missing `tsconfig.json` | No |
| NJS-012 | Medium | Missing ESLint config | No |
| NJS-013 | Low | Missing Prettier config | No |
| NJS-020 | Medium | Outdated Next.js version | No |
| NJS-021 | Medium | Dev dependency in dependencies | No |
| NJS-022 | Low | Missing recommended deps | No |
| NJS-030 | High | No test configuration | No |
| NJS-031 | Medium | No `__tests__/` directory | Yes |
| NJS-032 | High | Test framework not installed | No |
| NJS-040 | Critical | API keys in source code | No |
| NJS-041 | High | Exposed API routes without auth | No |
| NJS-042 | High | Insecure headers configuration | No |
| NJS-050 | Medium | Missing `.gitignore` entries | Yes |
| NJS-051 | Low | Missing `.env.example` | Yes |
| NJS-052 | Low | Missing `.nvmrc` | No |

## Scoring System

The health score is calculated from 0-100 using weighted category scores:

| Category | Weight |
|----------|--------|
| Structure | 20% |
| Dependencies | 20% |
| Configuration | 15% |
| Testing | 25% |
| Security | 15% |
| Documentation | 5% |

Each issue applies a penalty based on severity:

| Severity | Penalty |
|----------|---------|
| Critical | -25 |
| High | -15 |
| Medium | -8 |
| Low | -3 |
| Info | 0 |

**Grades:**

| Grade | Score Range |
|-------|-------------|
| A | 90-100 |
| B | 80-89 |
| C | 70-79 |
| D | 60-69 |
| F | 0-59 |

## Project Structure

```
repodoctor/
├── src/
│   ├── main.rs              # Entry point
│   ├── cli/                  # CLI commands and output formatting
│   │   ├── commands/
│   │   │   ├── scan.rs       # Scan command
│   │   │   ├── fix.rs        # Fix command
│   │   │   ├── report.rs     # Report command
│   │   │   └── init.rs       # Init command
│   │   └── output.rs         # Terminal/JSON formatters
│   ├── core/                 # Core logic
│   │   ├── project.rs        # Project detection
│   │   ├── scanner.rs        # Scan orchestration
│   │   ├── score.rs          # Health score calculation
│   │   └── config.rs         # .repodoctor.yml parser
│   ├── analyzers/            # Issue detection
│   │   ├── structure.rs      # Project structure checks
│   │   ├── dependencies.rs   # Dependency analysis
│   │   ├── config_files.rs   # Config file checks
│   │   ├── security.rs       # Secret detection
│   │   ├── symfony.rs        # Symfony-specific rules
│   │   ├── flutter.rs        # Flutter-specific rules
│   │   └── nextjs.rs         # Next.js-specific rules
│   ├── fixers/               # Auto-fix system
│   │   ├── directory.rs      # Create missing directories
│   │   ├── gitignore.rs      # Create/update .gitignore
│   │   └── editorconfig.rs   # Create .editorconfig
│   ├── reporters/            # Report generation
│   │   ├── html.rs           # HTML report
│   │   ├── markdown.rs       # Markdown report
│   │   └── badge.rs          # SVG health badge
│   ├── frameworks/           # Framework detection
│   │   └── detector.rs       # Auto-detect framework/language
│   └── utils/                # Shared utilities
│       └── fs.rs             # File system helpers
└── SPEC.md                   # Full technical specification
```

## Development

```bash
# Build
cargo build

# Run tests
cargo test

# Lint
cargo clippy -- -D warnings

# Run locally
cargo run -- scan .
cargo run -- fix . --dry-run
cargo run -- report . --format markdown
cargo run -- init
```

## License

MIT
