# Installation Guide

## Quick Install (Recommended)

From the project directory, run:

```bash
cargo install --path . --locked
```

This installs `circle-debug` to `~/.cargo/bin/` which should already be in your PATH.

## Verify Installation

```bash
# Check it's installed
which circle-debug

# Test it works
circle-debug --help
```

## Usage from Any Directory

Once installed, you can use it from anywhere:

```bash
# From any repo/directory
cd ~/repos/some-other-project
circle-debug build https://circleci.com/gh/org/repo/12345
```

## Updating

When you make changes to the tool:

```bash
# Just reinstall (Cargo detects version changes automatically)
cargo install --path . --locked
```

## Uninstall

```bash
cargo uninstall circle-debug
```

## Troubleshooting

### Command not found?

Make sure `~/.cargo/bin` is in your PATH:

```bash
# Add to ~/.zshrc or ~/.bashrc
export PATH="$HOME/.cargo/bin:$PATH"

# Reload shell
source ~/.zshrc
```

### Alternative: Symlink Method

If you want to use development builds directly:

```bash
# Build once
cargo build --release

# Create symlink
ln -s $(pwd)/target/release/circle-debug ~/.local/bin/circle-debug

# Make sure ~/.local/bin is in PATH
export PATH="$HOME/.local/bin:$PATH"
```

## For Team Distribution

If you want to share with your team:

```bash
# Team members can install directly from your repo
cargo install --git https://github.com/yourusername/circle-debug.git

# Or from a specific branch/tag
cargo install --git https://github.com/yourusername/circle-debug.git --tag v1.0.0
```