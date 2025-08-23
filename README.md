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

✅ **Parse CircleCI URLs** - Extract build information from standard CircleCI URLs  
✅ **Fetch build details** - Get comprehensive build status and failure information  
✅ **Identify failed steps** - Automatically highlight which steps failed and why  
✅ **Display failure logs** - Automatically fetches and analyzes build logs with smart error detection  
✅ **Progressive disclosure** - Shows smart summary + last 50 lines by default, with --full option for complete logs  
✅ **Contextual suggestions** - Provides fix suggestions based on error type (e.g., "Check file case sensitivity" for README errors)  
✅ **Filter logs** - Focus on specific packages or tasks with `--filter` option  
✅ **Timing analysis** - Shows slowest steps and identifies performance bottlenecks  
✅ **Beautiful output** - Color-coded terminal output with error highlighting  
✅ **Quick actions** - Generate links for reruns and artifact viewing  

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

## Setup

Set your CircleCI API token:

```bash
export CIRCLECI_TOKEN="your-circleci-personal-token"
```

Get your token from: https://app.circleci.com/settings/user/tokens

## Usage

### Basic Usage

Analyze a failed build (automatically fetches and analyzes logs):

```bash
cdb build https://circleci.com/gh/org/repo/12345
```

### Additional Options

```bash
# Show complete logs when error not found in summary
cdb build --full https://circleci.com/gh/org/repo/12345

# Show only last N lines
cdb build --tail 100 https://circleci.com/gh/org/repo/12345

# Filter logs to specific package/task
cdb build --filter "@stitch-fix/graphql-api-provider" https://circleci.com/gh/org/repo/12345

# Skip log fetching (only show metadata)
cdb build --no-fetch https://circleci.com/gh/org/repo/12345

# Check PR status and CircleCI checks
cdb pr 123 --repo org/repo

# Auto-detect current PR and repo
cdb pr
```

### Auto-Detection

`cdb` can automatically detect your current PR and repository:

```bash
# Auto-detect both PR and repository from current branch
cdb pr

# Auto-detect repository, specify PR
cdb pr 123

# Specify both explicitly
cdb pr 123 --repo org/repo
```

### Finding Your Current PR Manually

When working on a feature branch, you often need to find the PR number for use with `cdb`. Here are several ways to determine your current PR:

```bash
# Method 1: Use GitHub CLI to view current PR
gh pr view
# Shows PR details including the PR number

# Method 2: Get just the PR number
gh pr view --json number -q .number

# Method 3: Open PR in browser (shows URL with PR number)
gh pr view --web

# Method 4: List all PRs for current branch
gh pr list --head $(git branch --show-current)
```

### Common Workflow Examples

#### Debug CI failures for current PR
```bash
# Get current PR number and check its CI status
PR=$(gh pr view --json number -q .number)
cdb pr $PR --repo org/repo

# Or combine in one line
cdb pr $(gh pr view --json number -q .number) --repo org/repo
```

#### Debug a specific failing check
```bash
# First, view the PR to see failing checks
gh pr view

# Then use the CircleCI URL from the failing check
cdb build https://circleci.com/gh/org/repo/12345
```

#### Create an alias for quick PR debugging
Add to your shell config (`.bashrc`, `.zshrc`, etc.):
```bash
alias cdb-pr='cdb pr $(gh pr view --json number -q .number) --repo $(gh repo view --json nameWithOwner -q .nameWithOwner)'
```

Then simply run:
```bash
cdb-pr  # Automatically uses current PR and repo
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

- `cdb build <url>` - Analyze a specific build with smart error detection
- `cdb pr <pr-number>` - Check PR status and CircleCI checks

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