# cdb - CircleCI Debugger 🔍

A Rust CLI tool for quickly debugging CircleCI build failures directly from your terminal.

## Features

✅ **Parse CircleCI URLs** - Extract build information from standard CircleCI URLs  
✅ **Fetch build details** - Get comprehensive build status and failure information  
✅ **Identify failed steps** - Automatically highlight which steps failed and why  
✅ **Display failure logs** - Automatically fetches and analyzes build logs with smart error detection  
✅ **Progressive disclosure** - Shows smart summary + last 50 lines by default, with --full option for complete logs  
✅ **Beautiful output** - Color-coded terminal output for easy scanning  
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

# Skip log fetching (only show metadata)
cdb build --no-fetch https://circleci.com/gh/org/repo/12345

# Check PR status and CircleCI checks
cdb pr 123 --repo org/repo
```

### Example Output

```
Analyzing CircleCI Build
========================
→ Organization: org
→ Project: repo  
→ Build Number: 12345

Build Summary
=============
→ Status: failed
→ Branch: feature/new-component
→ Commit: Fix broken tests

Failed Steps
============

▸ Run tests
  ✗ npm test
  [Module Resolution] Failed to resolve package...

Quick Actions
=============
• Rerun: https://circleci.com/gh/org/repo/12345/retry
• SSH Debug: Click 'Rerun' → 'Rerun job with SSH' in CircleCI UI
• View artifacts: https://circleci.com/gh/org/repo/12345/artifacts
```

## Commands

- `cdb build <url>` - Analyze a specific build with smart error detection
- `cdb pr <pr-number>` - Check PR status and CircleCI checks
- `cdb workflow <pipeline-id>` - Get workflow details (coming soon)

## Why Rust?

- **Fast** - Near-instant parsing and API responses
- **Reliable** - Strong error handling for network issues
- **Portable** - Single binary, no dependencies
- **Concurrent** - Can handle multiple API calls efficiently

## Development

```bash
# Run in development
cargo run -- build https://circleci.com/gh/org/repo/12345

# Run tests
cargo test

# Format code
cargo fmt

# Lint
cargo clippy
```

## Roadmap

- [x] GitHub PR integration
- [x] Smart error detection
- [x] Progressive log disclosure
- [ ] Workflow analysis
- [ ] Config validation
- [ ] Artifact downloading
- [ ] Multiple build comparison
- [ ] SSH debug automation

## License

MIT