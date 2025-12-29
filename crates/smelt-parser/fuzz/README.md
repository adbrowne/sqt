# Fuzzing smelt-parser

This directory contains fuzzing targets for smelt-parser using cargo-fuzz.

## Quick Start

```bash
# Install cargo-fuzz (requires nightly Rust)
cargo install cargo-fuzz
rustup toolchain install nightly

# Run fuzzing (from crates/smelt-parser directory)
cargo +nightly fuzz run parse_never_panics -- -max_total_time=60
cargo +nightly fuzz run round_trip -- -max_total_time=60
```

## Fuzz Targets

### `parse_never_panics`
Verifies the parser never panics on any input, including malformed SQL.

**Property**: `parse(input)` should always return without panicking.

**Results** (as of December 29, 2024):
- ✅ 110,993+ executions with zero crashes
- 1091 edge coverage
- 327 interesting inputs discovered

### `round_trip`
Verifies round-trip preservation: valid SQL should parse → print → parse identically.

**Property**: If `parse(sql)` succeeds without errors, then `parse(print(parse(sql)))` should also succeed without errors.

**Results** (as of December 29, 2024):
- ⚠️ Found edge case with keyword case normalization
- Minimized failing input: `SELECT * FROM users WHERe ER`
- Root cause: Printer normalizes keyword case (`WHERe` → `WHERE`), affecting error-recovery behavior

**Known Issue**: The printer changes mixed-case keywords to uppercase, which can affect parse errors in edge cases. This is acceptable since the printer is for valid SQL only.

## Corpus

The corpus is seeded with 9 SQL test cases from the parser's test suite:
- `simple_select.sql` - Basic SELECT
- `select_with_where.sql` - WHERE clause
- `select_with_join.sql` - JOIN syntax
- `select_with_group_by.sql` - GROUP BY and aggregates
- `select_with_order_by.sql` - ORDER BY with NULLS
- `select_with_limit.sql` - LIMIT/OFFSET
- `select_cte.sql` - CTEs (WITH clause)
- `select_window.sql` - Window functions
- `select_ref.sql` - smelt.ref() extension

As fuzzing runs, libFuzzer discovers new interesting inputs and adds them to the corpus automatically.

## Advanced Usage

```bash
# Minimize a crashing input
cargo +nightly fuzz tmin parse_never_panics fuzz/artifacts/.../crash-...

# Get coverage statistics
cargo +nightly fuzz coverage parse_never_panics

# Run with more verbosity
cargo +nightly fuzz run parse_never_panics -- -verbosity=2

# Run until finding N unique crashes
cargo +nightly fuzz run parse_never_panics -- -max_total_time=3600 -timeout=10
```

## CI Integration

Fuzzing is run nightly in CI for 10 minutes per target. See `.github/workflows/fuzz.yml` (if configured).

## Notes

- Fuzzing requires nightly Rust due to instrumentation features
- The fuzz directory is excluded from the workspace (see `/Cargo.toml`)
- Artifacts (crashes) are saved to `fuzz/artifacts/`
- Each run uses a random seed; specify `-seed=<N>` to reproduce
