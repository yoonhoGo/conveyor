# Git Hooks

This directory contains git hooks to ensure code quality before pushing.

## Installation

Run the install script to set up the git hooks:

```bash
./hooks/install.sh
```

## Available Hooks

### pre-push

Runs before `git push` to ensure code quality:

- ✅ Code formatting check (`cargo fmt --all -- --check`)
- ✅ Clippy lint check (`cargo clippy --all-features --workspace -- -D warnings`)

## Bypassing Hooks

If you need to bypass the hooks (use sparingly):

```bash
git push --no-verify
```

## Fixing Issues

If the pre-push hook fails:

**Formatting issues:**
```bash
cargo fmt --all
```

**Clippy warnings:**
```bash
cargo clippy --all-features --workspace -- -D warnings
# Fix the reported issues
```
