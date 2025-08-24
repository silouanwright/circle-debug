# cdb - CircleCI Debugger 🔍

A Rust CLI tool for quickly debugging CircleCI build failures directly from your terminal.

## Who Should Use This?

If you're tired of:
- Clicking through CircleCI's web interface to find error messages
- Scrolling through thousands of lines of logs to find the actual failure
- Manually copying build URLs to share with teammates
- Not knowing which step is slowing down your builds

Then `cdb` is for you! It's designed for developers who want to quickly diagnose CI failures without leaving their terminal.

## Features

### 🎯 Smart Error Detection
- **Pattern matching** for common CI failures (TypeScript, tests, dependencies, etc.)
- **Contextual fix suggestions** based on error type
- **Progressive disclosure**: Smart summary → Last 50 lines → Full logs
- **Automatic log caching** to `/tmp` for quick re-analysis

### 🔍 Auto-Detection 
- **Current PR detection** - No need to find PR numbers
- **Repository detection** - Works in any git repo with GitHub remote
- **Failed check extraction** - Automatically finds CircleCI URLs from PR checks

### ⚡ Performance Analysis
- **Timing breakdown** - See how long each step takes
- **Bottleneck detection** - Identifies steps taking >50% of build time
- **Sorted by duration** - Quickly spot the slowest operations

### 🎨 Beautiful Output
- **Color-coded** - Red for errors, yellow for warnings, green for success
- **Error highlighting** - Failed lines have special background highlighting
- **Clean formatting** - Strips ANSI codes when saving to files
- **Line numbers** - Easy to reference specific log locations  

## Installation

```bash
# Clone and build
git clone <your-repo>
cd circle-debug
cargo build --release

# Install to PATH
cargo install --path .
```

## Prerequisites

- **For `build` command**: No additional requirements
- **For `pr` command**: Requires [GitHub CLI (gh)](https://cli.github.com/) installed and authenticated
  ```bash
  # Install GitHub CLI
  brew install gh  # macOS
  # or see https://cli.github.com/ for other platforms
  
  # Authenticate
  gh auth login
  ```

## Quick Start

```bash
# 1. Set your CircleCI token
export CIRCLECI_TOKEN="your-circleci-personal-token"

# 2. Check your current PR's CI status (auto-detects everything!)
cdb pr

# 3. Debug any failed builds
cdb build https://circleci.com/gh/org/repo/12345
```

That's it! No complex setup required.

## Setup

Get your CircleCI token from: https://app.circleci.com/settings/user/tokens

Add to your shell config (`~/.bashrc`, `~/.zshrc`, etc.):
```bash
export CIRCLECI_TOKEN="your-circleci-personal-token"
```

## Usage

### Basic Commands

#### Analyze a failed build
```bash
# Analyze a CircleCI build URL (smart error detection + last 50 lines)
cdb build https://circleci.com/gh/org/repo/12345

# Show complete logs when error not found in summary
cdb build --full https://circleci.com/gh/org/repo/12345

# Save logs to a specific file
cdb build --output debug.log https://circleci.com/gh/org/repo/12345
```

#### Check PR status
```bash
# Auto-detect current PR and repository
cdb pr

# Specify PR number, auto-detect repository  
cdb pr 123

# Use PR URL
cdb pr https://github.com/org/repo/pull/123

# Specify everything explicitly
cdb pr 123 --repo org/repo
```

### Advanced Options

```bash
# Show only last N lines
cdb build --tail 100 https://circleci.com/gh/org/repo/12345

# Filter logs to specific package/task
cdb build --filter "@stitch-fix/graphql-api-provider" https://circleci.com/gh/org/repo/12345

# Skip log fetching (only show metadata)
cdb build --no-fetch https://circleci.com/gh/org/repo/12345
```

### Auto-Detection Magic ✨

`cdb pr` automatically detects:
- **Current PR**: Uses `gh` to find the PR for your current branch
- **Current Repository**: Uses `gh` to detect the repo you're in

No need to manually find PR numbers or repository names!

### Common Workflow Examples

#### Quick PR debugging (most common)
```bash
# Just run this from your feature branch - everything is auto-detected!
cdb pr
```

#### Debug a specific failing check
```bash
# 1. Check your PR status
cdb pr

# 2. Copy the CircleCI URL from any failed check and analyze it
cdb build https://circleci.com/gh/org/repo/12345
```

#### Progressive debugging workflow
```bash
# 1. Start with smart summary (default)
cdb build https://circleci.com/gh/org/repo/12345

# 2. If error not visible, show full logs
cdb build --full https://circleci.com/gh/org/repo/12345

# 3. Or filter to specific package if you know where the error is
cdb build --filter "@mypackage" https://circleci.com/gh/org/repo/12345
```

### Example Output

#### Smart Error Detection with Contextual Suggestions
```
=== SMART ERROR DETECTION ===
Found 1 error pattern(s):
[File Not Found] Line 26: ENOENT: no such file or directory 'README.md'
💡 Suggestion: Check file case sensitivity (README.md vs readme.md)

=== LAST 50 LINES (BUILD EXIT ZONE) ===
   24 │ [command]/usr/bin/git add .
   25 │ [command]/usr/bin/git commit -m "Update dependencies"
   26 ► ENOENT: no such file or directory 'README.md'
   27 │ errno: -2,
   28 │ code: 'ENOENT',
```

#### Timing Analysis
```
Timing Analysis
===============
Total build time: 2m 45s

Slowest steps:
  1. Build - 1m 32s (55%)
  2. Run tests - 45s (27%)
  3. Setup environment - 28s (17%)

⚠ Bottleneck detected: 'Build' takes 55% of total time
  Consider optimizing or parallelizing this step
```

#### Filtered Logs
```
Filter '@stitch-fix/graphql-api-provider': 127 of 2341 lines

@stitch-fix/graphql-api-provider:lint: ✖ 1 problem (1 error, 0 warnings)
@stitch-fix/graphql-api-provider:lint: npm error Lifecycle script `lint` failed
💡 Suggestion: Run 'npm run lint -- --fix' to auto-fix some issues
```

## Commands

### `cdb build <url>` - Analyze CircleCI builds
Fetches and analyzes CircleCI build logs with smart error detection.

**Options:**
- `--full, -f` - Show complete logs instead of smart summary
- `--output, -o <file>` - Save logs to file (auto-saves to `/tmp/cdb-<build>.log`)
- `--tail <lines>` - Show only last N lines
- `--filter <text>` - Filter logs to lines containing text
- `--no-fetch` - Skip log fetching, only show build metadata

### `cdb pr [pr-number]` - Check PR status
Shows all CircleCI checks for a GitHub PR.

**Arguments:**
- `pr-number` - Optional PR number or URL (auto-detects if omitted)
- `--repo, -r <org/repo>` - Repository (auto-detects if omitted)

**Note:** Requires GitHub CLI (`gh`) installed and authenticated

## Why Rust?

- **Fast** - Near-instant parsing and API responses
- **Reliable** - Strong error handling for network issues
- **Portable** - Single binary, no dependencies
- **Concurrent** - Can handle multiple API calls efficiently

## Contributing

We welcome contributions! Here's how to get started:

### Development Setup

1. Clone the repository:
```bash
git clone https://github.com/silouanwright/circle-debug
cd circle-debug
```

2. Build the project:
```bash
cargo build
```

3. Run in development:
```bash
cargo run -- build https://circleci.com/gh/org/repo/12345
```

### Testing

```bash
# Run all tests
cargo test

# Run with verbose output
cargo test -- --nocapture

# Run documentation tests
cargo test --doc
```

### Code Quality

Before submitting a PR, please run:
```bash
# Format code
cargo fmt

# Run linter
cargo clippy -- -D warnings

# Generate and review documentation
cargo doc --open
```

### Reporting Issues

Found a bug or have a feature request? Please open an issue at:
https://github.com/silouanwright/circle-debug/issues

## Roadmap

- [x] GitHub PR integration
- [x] Smart error detection with pattern matching
- [x] Progressive log disclosure (smart summary → last 50 → full)
- [x] Contextual fix suggestions
- [x] Filter logs by package/task
- [x] Timing analysis and bottleneck detection
- [ ] Workflow analysis
- [ ] Config validation
- [ ] Artifact downloading
- [ ] Multiple build comparison
- [ ] SSH debug automation
- [ ] Pattern learning (remember common errors in repo)

## License

MIT