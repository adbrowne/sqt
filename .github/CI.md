# Continuous Integration

This document describes the CI/CD setup for the smelt project.

## Workflows

### Tests (`test.yml`)

**Triggers**:
- Push to `main` or `feature/*` branches
- Pull requests to `main`

**Jobs**:

#### `test` - Main Test Suite
- **Formatting**: Check code formatting with `cargo fmt`
- **Linting**: Run clippy with `-D warnings` (fails on warnings)
- **Build**: Compile all targets
- **Unit Tests**: Run all unit tests (`cargo test --lib`)
- **Property Tests**: Run property-based tests with 100 cases (quick mode)

**Duration**: ~3-5 minutes

#### `fuzz-build` - Fuzz Targets Build Check
- Verify fuzz targets compile with nightly Rust
- Uses cargo-fuzz to build all targets
- Ensures fuzzing infrastructure stays functional

**Duration**: ~1-2 minutes

### Fuzzing (`fuzz.yml`)

**Triggers**:
- Pull requests (quick 60s run per target)
- Nightly at 2 AM UTC (thorough 600s run per target)
- Manual via workflow_dispatch (custom duration)

**Targets**:
- `parse_never_panics` - Ensures parser never panics
- `round_trip` - Verifies parse → print → parse preservation

**Features**:
- Runs targets in parallel (matrix strategy)
- Caches builds for faster execution
- Automatically uploads crash artifacts
- Fails CI if crashes are found
- Provides minimization instructions

**Duration**:
- PR checks: ~2 minutes total (60s per target, parallel)
- Nightly: ~20 minutes total (600s per target, parallel)

## Caching Strategy

All workflows use aggressive caching to minimize CI time and costs:

- **Cargo registry**: `~/.cargo/registry`
- **Cargo git index**: `~/.cargo/git`
- **Build artifacts**: `target/` (test workflow)
- **Fuzz build**: `crates/smelt-parser/fuzz/target` (fuzz workflows)

Cache keys include `Cargo.lock` hash to invalidate when dependencies change.

## Running Locally

### Reproduce Test Workflow

```bash
# Formatting
cargo fmt --all -- --check

# Linting
cargo clippy --all-targets -- -D warnings

# Build
cargo build --all-targets

# Tests
cargo test --lib
cargo test --test proptest_round_trip -- --test-threads=1
```

### Reproduce Fuzz Workflow

```bash
# Install nightly and cargo-fuzz
rustup toolchain install nightly
cargo install cargo-fuzz

# Build fuzz targets
cd crates/smelt-parser
cargo +nightly fuzz build

# Run fuzzing (quick mode like PR)
cargo +nightly fuzz run parse_never_panics -- -max_total_time=60
cargo +nightly fuzz run round_trip -- -max_total_time=60

# Run fuzzing (nightly mode)
cargo +nightly fuzz run parse_never_panics -- -max_total_time=600
cargo +nightly fuzz run round_trip -- -max_total_time=600
```

## Debugging CI Failures

### Test Failures

1. Check the failing job in GitHub Actions
2. Look at the specific test output
3. Reproduce locally with the same command
4. Fix and push

### Fuzz Failures

1. Download crash artifact from GitHub Actions
2. Reproduce locally:
   ```bash
   cargo +nightly fuzz run <target> path/to/crash-file
   ```
3. Minimize the crashing input:
   ```bash
   cargo +nightly fuzz tmin <target> path/to/crash-file
   ```
4. Debug and fix the parser
5. Add regression test
6. Re-run fuzzing to verify fix

### Build Failures

1. Check if dependencies changed
2. Verify `Cargo.lock` is up to date
3. Check for clippy warnings (CI fails on warnings)
4. Ensure formatting is correct (`cargo fmt`)

## Adding New Workflows

When adding new workflows:

1. Follow the existing naming convention
2. Use caching for faster builds
3. Set appropriate timeout limits
4. Provide clear failure messages
5. Document in this file

## Best Practices

**For Contributors**:
- Run `cargo fmt` before committing
- Fix all clippy warnings
- Run tests locally before pushing
- Check CI status before requesting review

**For Reviewers**:
- Wait for CI to pass before approving
- Check fuzz results for new parser features
- Verify test coverage for new code

## Future Enhancements

**Potential additions**:
- Code coverage reporting (e.g., codecov)
- Performance benchmarking (e.g., criterion)
- Dependency scanning (e.g., cargo-audit)
- Documentation building (e.g., cargo doc)
- Release automation (e.g., cargo-release)
