# Release gate for blawktrust (PowerShell version)
# Run this before creating any release tag
# Exit code 0 = ready to release, non-zero = not ready

$ErrorActionPreference = "Stop"

Write-Host "=== blawktrust Release Gate ===" -ForegroundColor Cyan
Write-Host ""

# Check we're on a clean commit
$gitStatus = git diff-index --quiet HEAD --
if ($LASTEXITCODE -ne 0) {
    Write-Host "❌ FAIL: Working directory has uncommitted changes" -ForegroundColor Red
    Write-Host "Commit or stash changes before releasing"
    exit 1
}
Write-Host "✅ Working directory is clean" -ForegroundColor Green

# Check we're on main/master branch
$branch = git rev-parse --abbrev-ref HEAD
if ($branch -ne "master" -and $branch -ne "main") {
    Write-Host "⚠️  WARNING: Not on master/main branch (currently on: $branch)" -ForegroundColor Yellow
    Write-Host "Consider releasing from master/main"
    $response = Read-Host "Continue anyway? (y/N)"
    if ($response -ne "y" -and $response -ne "Y") {
        exit 1
    }
}
Write-Host "✅ Branch check passed: $branch" -ForegroundColor Green

# Run formatting check
Write-Host ""
Write-Host "Running cargo fmt check..."
cargo fmt --all -- --check
if ($LASTEXITCODE -ne 0) {
    Write-Host "❌ FAIL: Code is not formatted" -ForegroundColor Red
    Write-Host "Run: cargo fmt --all"
    exit 1
}
Write-Host "✅ Formatting check passed" -ForegroundColor Green

# Run clippy
Write-Host ""
Write-Host "Running clippy..."
cargo clippy --all-targets --all-features -- -D warnings
if ($LASTEXITCODE -ne 0) {
    Write-Host "❌ FAIL: Clippy found issues" -ForegroundColor Red
    exit 1
}
Write-Host "✅ Clippy check passed" -ForegroundColor Green

# Run tests
Write-Host ""
Write-Host "Running test suite..."
cargo test --lib
if ($LASTEXITCODE -ne 0) {
    Write-Host "❌ FAIL: Tests failed" -ForegroundColor Red
    exit 1
}
Write-Host "✅ Tests passed" -ForegroundColor Green

# Build release
Write-Host ""
Write-Host "Building release..."
cargo build --lib --release
if ($LASTEXITCODE -ne 0) {
    Write-Host "❌ FAIL: Release build failed" -ForegroundColor Red
    exit 1
}
Write-Host "✅ Release build succeeded" -ForegroundColor Green

# Check if tag already exists
if ($args.Count -gt 0) {
    $tag = $args[0]
    git rev-parse $tag 2>$null
    if ($LASTEXITCODE -eq 0) {
        Write-Host ""
        Write-Host "⚠️  WARNING: Tag $tag already exists" -ForegroundColor Yellow
        Write-Host "NEVER move existing tags! Create a new tag instead (e.g., $tag-fixed or increment version)"
        exit 1
    }
    Write-Host "✅ Tag $tag does not exist yet" -ForegroundColor Green
}

# All checks passed
Write-Host ""
Write-Host "=========================================" -ForegroundColor Green
Write-Host "✅ ALL CHECKS PASSED - READY TO RELEASE" -ForegroundColor Green
Write-Host "=========================================" -ForegroundColor Green
Write-Host ""
Write-Host "Next steps:"
Write-Host "  1. Create tag:  git tag v0.x.y"
Write-Host "  2. Push tag:    git push origin v0.x.y"
Write-Host "  3. NEVER move or delete tags once pushed"
Write-Host ""
Write-Host "Remember: Tags are immutable contracts!"

exit 0
