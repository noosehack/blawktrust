#!/usr/bin/env bash
# Release gate for blawktrust
# Run this before creating any release tag
# Exit code 0 = ready to release, non-zero = not ready

set -e  # Exit on any error

echo "=== blawktrust Release Gate ==="
echo ""

# Check we're on a clean commit
if ! git diff-index --quiet HEAD --; then
    echo "❌ FAIL: Working directory has uncommitted changes"
    echo "Commit or stash changes before releasing"
    exit 1
fi
echo "✅ Working directory is clean"

# Check we're on main/master branch
BRANCH=$(git rev-parse --abbrev-ref HEAD)
if [[ "$BRANCH" != "master" && "$BRANCH" != "main" ]]; then
    echo "⚠️  WARNING: Not on master/main branch (currently on: $BRANCH)"
    echo "Consider releasing from master/main"
    read -p "Continue anyway? (y/N) " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        exit 1
    fi
fi
echo "✅ Branch check passed: $BRANCH"

# Run formatting check
echo ""
echo "Running cargo fmt check..."
if ! cargo fmt --all -- --check; then
    echo "❌ FAIL: Code is not formatted"
    echo "Run: cargo fmt --all"
    exit 1
fi
echo "✅ Formatting check passed"

# Run clippy
echo ""
echo "Running clippy..."
if ! cargo clippy --all-targets --all-features -- -D warnings; then
    echo "❌ FAIL: Clippy found issues"
    exit 1
fi
echo "✅ Clippy check passed"

# Run tests
echo ""
echo "Running test suite..."
if ! cargo test --lib; then
    echo "❌ FAIL: Tests failed"
    exit 1
fi
echo "✅ Tests passed"

# Build release
echo ""
echo "Building release..."
if ! cargo build --lib --release; then
    echo "❌ FAIL: Release build failed"
    exit 1
fi
echo "✅ Release build succeeded"

# Check if tag already exists
if [ -n "$1" ]; then
    TAG=$1
    if git rev-parse "$TAG" >/dev/null 2>&1; then
        echo ""
        echo "⚠️  WARNING: Tag $TAG already exists"
        echo "NEVER move existing tags! Create a new tag instead (e.g., $TAG-fixed or increment version)"
        exit 1
    fi
    echo "✅ Tag $TAG does not exist yet"
fi

# All checks passed
echo ""
echo "========================================="
echo "✅ ALL CHECKS PASSED - READY TO RELEASE"
echo "========================================="
echo ""
echo "Next steps:"
echo "  1. Create tag:  git tag v0.x.y"
echo "  2. Push tag:    git push origin v0.x.y"
echo "  3. NEVER move or delete tags once pushed"
echo ""
echo "Remember: Tags are immutable contracts!"

exit 0
