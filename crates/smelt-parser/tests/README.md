# smelt-parser Test Suite

This directory contains the test suite for the smelt SQL parser, including property-based tests and round-trip validation.

## Test Structure

### Unit Tests (`src/parser.rs`)
Located in the main parser module with `#[cfg(test)]`:
- **68 unit tests** covering all parser phases (1-13)
- Tests for: SELECT, JOIN, WHERE, GROUP BY, ORDER BY, LIMIT, window functions, CTEs
- Tests for error recovery and edge cases

### Printer Tests (`src/printer.rs`)
Located in the printer module with `#[cfg(test)]`:
- **10 round-trip tests** verifying parse → print → parse preservation
- Tests basic and complex SQL constructs

### Property-Based Tests (`tests/`)

#### `proptest_generators.rs`
SQL generators for property-based testing:
- **Basic generators**: identifiers, numbers, string literals, column references
- **Expression generators**: simple expressions, comparison expressions, binary operations
- **Clause generators**: WHERE, GROUP BY, HAVING, ORDER BY, LIMIT
- **SELECT generators**: Simple and complex SELECT statements with various clauses

#### `proptest_round_trip.rs`
Property-based round-trip tests using proptest:
- **8 property tests** generating 100 cases each (800 total)
- **2 robustness tests** generating 1000 cases each (2000 total)
- **10 edge case tests** for specific scenarios
- **Total: 2810+ test cases** run on every test execution

## Test Coverage

### What's Tested

✅ **Parser robustness**: Never panics on any input
✅ **Round-trip preservation**: Parse → Print → Parse maintains AST structure
✅ **All SQL features**: SELECT, FROM, WHERE, JOIN, GROUP BY, HAVING, ORDER BY, LIMIT
✅ **Advanced features**: Window functions, CTEs, subqueries
✅ **Error recovery**: Parser handles invalid input gracefully
✅ **Edge cases**: Empty selects, multiple columns, qualified names, complex expressions

### Test Results

**Current status** (as of Phase 2 completion):
- ✅ 78 unit tests passing (68 parser + 10 printer)
- ✅ 20 property tests passing
- ✅ 2810+ generated test cases passing
- ✅ 0 failures
- ✅ 0 edge cases found requiring fixes

## Running Tests

```bash
# Run all tests
cargo test -p smelt-parser

# Run only unit tests
cargo test -p smelt-parser --lib

# Run only property tests
cargo test -p smelt-parser --test proptest_round_trip

# Run with more property test cases
PROPTEST_CASES=10000 cargo test -p smelt-parser --test proptest_round_trip

# Run with verbose output
cargo test -p smelt-parser -- --nocapture
```

## Property Test Configuration

By default, property tests run:
- **100 cases** for round-trip tests
- **1000 cases** for robustness tests

You can increase test cases with the `PROPTEST_CASES` environment variable:

```bash
# Run 10,000 cases per test
PROPTEST_CASES=10000 cargo test -p smelt-parser --test proptest_round_trip
```

## Test Philosophy

### Round-Trip Testing
The core testing strategy is **round-trip validation**:
1. Generate valid SQL using proptest generators
2. Parse the SQL into an AST
3. Print the AST back to SQL
4. Re-parse the printed SQL
5. Verify both ASTs are structurally identical (ignoring whitespace)

This ensures:
- The printer generates valid SQL
- The parser can handle its own output
- AST structure is preserved
- No information is lost in round-trips

### Generator Strategy
We use a **grammar-based generation** approach:
- Generators compose to build valid SQL
- Each generator produces syntactically correct fragments
- Composition ensures valid overall structure
- Avoids generating invalid SQL that would be rejected

This is more effective than mutation-based testing because:
- Higher success rate (most generated SQL is valid)
- Tests the happy path extensively
- Finds edge cases in valid SQL handling

### Future Additions
Planned test enhancements:
- **Mutation-based generators** for error recovery testing
- **Fuzzing with cargo-fuzz** for crash detection
- **Coverage-guided testing** to find untested code paths
- **Grammar-based mutations** to test near-valid SQL

## Edge Cases Found

### Phase 1 (Printer Implementation)
- None - all 68 existing tests round-trip successfully

### Phase 2 (Property-Based Testing)
- None - all 2810+ generated test cases pass
- Parser handles all generated SQL correctly
- Printer produces valid SQL for all inputs

## Regression Tests

When bugs are found via property testing:
1. Add a specific regression test to `proptest_round_trip.rs`
2. Fix the bug
3. Verify the regression test passes
4. Keep the test to prevent future regressions

Example:
```rust
#[test]
fn test_regression_issue_123() {
    // Specific SQL that caused a failure
    let sql = "SELECT ...";
    assert_round_trip(sql);
}
```
