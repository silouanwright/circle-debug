# circle-debug 🔍

A Rust CLI tool for quickly debugging CircleCI build failures directly from your terminal.

## Features

✅ **Parse CircleCI URLs** - Extract build information from standard CircleCI URLs  
✅ **Fetch build details** - Get comprehensive build status and failure information  
✅ **Identify failed steps** - Automatically highlight which steps failed and why  
✅ **Display failure logs** - Optionally fetch and display the last 20 lines of failed step logs  
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

Analyze a failed build from URL:

```bash
circle-debug build https://circleci.com/gh/stitchfix/web-frontend/156093
```

### With Log Fetching

To also fetch and display the last 20 lines of failed step logs:

```bash
CIRCLE_DEBUG_FETCH_LOGS=1 circle-debug build https://circleci.com/gh/stitchfix/web-frontend/156093
```

### Example Output

```
Analyzing CircleCI Build
========================
→ Organization: stitchfix
→ Project: web-frontend
→ Build Number: 156093

Build Summary
=============
→ Status: failed
→ Branch: feature/new-component
→ Commit: Fix broken tests

Failed Steps
============

▸ Run tests
  ✗ npm test
  → Log URL: https://circle-artifacts.com/...

Quick Actions
=============
• Rerun: https://circleci.com/gh/stitchfix/web-frontend/156093/retry
• SSH Debug: Click 'Rerun' → 'Rerun job with SSH' in CircleCI UI
• View artifacts: https://circleci.com/gh/stitchfix/web-frontend/156093/artifacts
```

## Commands

- `circle-debug build <url>` - Analyze a specific build
- `circle-debug workflow <pipeline-id>` - Get workflow details (coming soon)
- `circle-debug pr <pr-number>` - Check PR status (coming soon)

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

- [ ] GitHub PR integration
- [ ] Workflow analysis
- [ ] Config validation
- [ ] Artifact downloading
- [ ] Multiple build comparison
- [ ] SSH debug automation

## License

MIT