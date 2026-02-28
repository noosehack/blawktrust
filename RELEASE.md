# Release Process

## Critical Rules

1. **Tags are immutable** - NEVER move or delete a pushed tag
2. **CI must pass** - Only tag commits that pass all CI checks
3. **API breaks require semver** - Breaking changes require major version bump

---

## Before Creating a Tag

### Run Release Gate

```bash
# Linux/macOS
./scripts/release_check.sh v0.2.0

# Windows
.\scripts\release_check.ps1 v0.2.0
```

This script verifies:
- ✅ Working directory is clean (no uncommitted changes)
- ✅ Code is formatted (`cargo fmt`)
- ✅ No clippy warnings
- ✅ All tests pass
- ✅ Release build succeeds
- ✅ Tag doesn't already exist

**The script MUST pass before creating any tag.**

---

## Creating a Release

1. **Ensure CI is green** on the commit you want to tag
   - Check: https://github.com/noosehack/blawktrust/actions

2. **Run release gate**:
   ```bash
   ./scripts/release_check.sh v0.2.0
   ```

3. **Create and push tag** (only if gate passed):
   ```bash
   git tag v0.2.0
   git push origin v0.2.0
   ```

4. **Verify tag on GitHub**:
   - https://github.com/noosehack/blawktrust/tags

---

## Semantic Versioning

- **Major (1.0.0 → 2.0.0)**: Breaking API changes
- **Minor (1.0.0 → 1.1.0)**: New features, backward compatible
- **Patch (1.0.0 → 1.0.1)**: Bug fixes, backward compatible

### API Breaking Changes

CI runs `cargo-semver-checks` on PRs. If it detects breaking changes:
- **Acceptable**: Bump major version
- **Not acceptable**: Revert the breaking change or provide migration path

Examples of breaking changes:
- Removing public types (`Column::Date`)
- Removing public functions
- Changing function signatures
- Changing struct fields

---

## What to Do If a Tag is Broken

**NEVER move or delete the tag.** Instead:

1. **Fix the issue** in a new commit
2. **Create a new tag**: `v0.2.1` or `v0.2.0-fixed`
3. **Document** what was fixed in commit message

### Example from 2026-02-28

```
v0.1.0-orientation-stable (broken) → DO NOT DELETE
v0.1.0-orientation-stable (fixed)  → CREATE NEW TAG
```

We created a new commit and re-tagged. The old tag was deleted only because it wasn't yet consumed by downstream.

---

## CI Workflow

GitHub Actions runs on every push and PR:

1. **Format check** - `cargo fmt --check`
2. **Clippy** - `cargo clippy -- -D warnings`
3. **Tests** - `cargo test --lib`
4. **Build** - `cargo build --release`
5. **Semver check** (PRs only) - `cargo semver-checks`

All must pass before merging to main.

---

## Downstream Impact (BLISP)

When releasing blawktrust:
1. BLISP will need to update its `Cargo.toml` to use the new tag
2. BLISP's integration test will verify the API surface still works
3. BLISP will run its own release gate before tagging

This creates a **safety chain**: blawktrust → BLISP → user

---

## Checklist

Before pushing a tag:

- [ ] CI is green on GitHub Actions
- [ ] `./scripts/release_check.sh` passes
- [ ] Version follows semver rules
- [ ] Tag doesn't already exist
- [ ] Commit message describes changes
- [ ] Breaking changes are documented

---

## Emergency: Yanking a Release

If a critical bug is found after release:

1. **DO NOT** delete the tag
2. **Create hotfix** in new commit
3. **Tag hotfix**: `v0.2.1` (patch bump)
4. **Announce** in CHANGELOG or GitHub Releases

---

## Questions?

- Check CI logs: https://github.com/noosehack/blawktrust/actions
- Review semver guide: https://semver.org/
- See incident report: `/OPTION_A_COMPLETE.md` (2026-02-28 API break)
