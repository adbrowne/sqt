# smelt Development Roadmap

This document tracks the implementation status of smelt, aligned with the spec in [DESIGN.md](DESIGN.md).

## Current Status

**Multi-Backend Architecture with Basic Incremental Materialization Complete**: Parser, LSP, multi-backend CLI with DuckDB and Spark (stub) implementations, and basic incremental materialization support.

```sql
-- ✅ Supported syntax (parser & LSP)
SELECT * FROM smelt.ref('model_name')
SELECT * FROM smelt.ref('events', filter => date > '2024-01-01')
SELECT * FROM smelt.ref('orders', filter => status = 'active', limit => 100)
```

```bash
# ✅ Supported CLI commands
smelt run                           # Execute all models
smelt run --show-results            # Preview query results
smelt run --verbose                 # Show compiled SQL
smelt run --dry-run                 # Validate without executing
smelt run --target prod             # Execute against Spark target
```

```yaml
# ✅ Supported configuration
targets:
  dev:
    type: duckdb
    database: dev.duckdb
    schema: main
  prod:
    type: spark
    connect_url: sc://localhost:15002
    catalog: spark_catalog
    schema: production
```

---

## ✅ Phase 1: Async Backend Architecture (COMPLETED)

**Completed**: December 27, 2025

### What Was Implemented

- **smelt-backend** crate with async Backend trait
  - All operations async (execute_sql, create_table_as, create_view_as, etc.)
  - Arrow RecordBatch for data interchange
  - BackendCapabilities for feature detection
  - SqlDialect enum (DuckDB, SparkSQL, PostgreSQL)
  - ExecutionResult and Materialization types

- **Fully async CLI**
  - Converted main() to async with tokio runtime
  - All executor operations async
  - Clean async/await throughout

### Key Changes

- **New crate**: `crates/smelt-backend/` - Backend trait definition
- **Updated**: `crates/smelt-cli/src/main.rs` - Async main function
- **Updated**: `crates/smelt-cli/Cargo.toml` - Added tokio dependency

### Test Results

All 31 existing tests passing after async conversion.

---

## ✅ Phase 2: DuckDB Backend Implementation (COMPLETED)

**Completed**: December 27, 2025

### What Was Implemented

- **smelt-backend-duckdb** crate
  - Full Backend trait implementation for DuckDB
  - Arc<Mutex<Connection>> for thread-safe async access
  - All operations wrapped in tokio::spawn_blocking
  - Comprehensive test suite (5 tests)

- **CLI refactored to use Backend trait**
  - executor.rs converted to backend-agnostic async functions
  - execute_model() and validate_sources() accept any Backend
  - Removed direct DuckDB dependency from CLI

### Implementation Details

**New crate**: `crates/smelt-backend-duckdb/`
- `src/lib.rs` - DuckDbBackend implementation
  - execute_sql: Prepares statement and queries Arrow RecordBatch
  - create_table_as/create_view_as: DDL operations
  - drop_table/view_if_exists: Safe cleanup
  - get_row_count: Efficient counting
  - get_preview: Limited result sets
  - table_exists: Information schema queries
  - ensure_schema: CREATE SCHEMA IF NOT EXISTS
  - dialect(): Returns SqlDialect::DuckDB
  - capabilities(): DuckDB features (QUALIFY, MERGE, CREATE OR REPLACE)

**Updated files**:
- `crates/smelt-cli/src/executor.rs` - Backend-agnostic functions
- `crates/smelt-cli/src/lib.rs` - Updated exports
- `crates/smelt-cli/Cargo.toml` - Added smelt-backend-duckdb dependency

### Test Results

All 36 tests passing (5 new DuckDB backend tests + 31 existing).

---

## ✅ Phase 3: Spark Backend Support (COMPLETED)

**Completed**: December 27, 2025

### What Was Implemented

- **smelt-backend-spark** crate (stub implementation)
  - Defines interface for Spark Connect integration
  - Documents requirements (protoc, spark-connect crate)
  - Working stub that returns appropriate errors
  - 2 tests for creation and stub behavior
  - Ready for real Spark Connect implementation

- **Multi-backend configuration**
  - Target struct supports both DuckDB and Spark
  - Optional fields: database (DuckDB), connect_url/catalog (Spark)
  - BackendType enum for backend selection
  - backend_type() method determines backend from config

- **Feature-flagged compilation**
  - default = ["duckdb"]
  - spark = ["smelt-backend-spark"]
  - Spark backend optional to reduce binary size
  - Clear error if Spark target used without --features spark

- **Runtime backend selection**
  - Box<dyn Backend> for polymorphism
  - Backend created based on target configuration
  - Prints backend type and connection details at startup

### Configuration Format

```yaml
# DuckDB target
targets:
  dev:
    type: duckdb
    database: dev.duckdb
    schema: main

# Spark target
targets:
  prod:
    type: spark
    connect_url: sc://localhost:15002
    catalog: spark_catalog
    schema: production
```

### Implementation Details

**New crate**: `crates/smelt-backend-spark/`
- Stub implementation of Backend trait
- Documents Spark Connect requirements
- Qualified table names: catalog.schema.table
- Future work: Real Spark Connect integration

**Updated files**:
- `crates/smelt-cli/src/config.rs` - Multi-backend Target struct
- `crates/smelt-cli/src/main.rs` - Backend selection logic
- `crates/smelt-cli/Cargo.toml` - Feature flags

### Benefits

- **Clean separation**: Each backend is its own crate
- **Optional dependencies**: Spark only when needed
- **Extensible**: Easy to add new backends
- **Backward compatible**: Existing configs still work
- **Validated architecture**: Multi-backend pattern proven with stub

### Test Results

All 38 tests passing (5 DuckDB + 2 Spark + 18 CLI + 10 db + 3 parser).

---

## ⏸️ Phase 4: Dialect Handling (DEFERRED)

**Status**: Deferred - architecture proven, implementation not urgent

### Why Deferred

The multi-backend architecture is now validated with DuckDB (working) and Spark (stub). Dialect handling can be implemented when needed for real Spark integration or additional backends.

### What Would Be Implemented

**Dialect-aware SQL rewriting**:
- Automatic rewriting for safe transformations
  - Date literal syntax: DuckDB `DATE '2024-01-01'` → Spark `DATE('2024-01-01')`
  - String concatenation normalization
  - Function name translations

- Error on impossible transformations
  - DuckDB QUALIFY → Spark (no direct equivalent)
  - Backend-specific functions with no translation

- Opt-in rewriting for risky transformations
  - User annotations like `-- @allow_rewrite: qualify`
  - Transforms that might change semantics or performance
  - QUALIFY → subquery rewrite (adds overhead)

**Implementation approach**:
```rust
// In smelt-backend crate
pub trait SqlRewriter {
    fn rewrite(&self, sql: &str, from: SqlDialect, to: SqlDialect) -> Result<String>;
}

// Safe rewrites (automatic)
pub struct SafeRewriter;

// Opt-in rewrites (requires annotations)
pub struct OptInRewriter;
```

### Dialect Differences to Handle

| Feature | DuckDB | Spark SQL | Translation |
|---------|--------|-----------|-------------|
| Date literal | `DATE '2024-01-01'` | `DATE('2024-01-01')` | ✅ Auto |
| String concat | `\|\|` | `CONCAT()` or `\|\|` | ✅ Auto |
| QUALIFY | ✅ Native | ❌ None | ❌ Error or 🔄 Opt-in subquery |
| MERGE | ✅ Native | ✅ Delta Lake | ✅ Check capability |
| Array literal | `[1, 2, 3]` | `ARRAY(1, 2, 3)` | ✅ Auto |
| CREATE OR REPLACE TABLE | ✅ | ❌ | 🔄 DROP + CREATE |

### Files to Create/Modify

- `crates/smelt-backend/src/rewrite.rs` - Rewriting framework
- `crates/smelt-backend/src/dialect.rs` - Dialect-specific rules
- `crates/smelt-parser/` - Parse `@allow_rewrite` annotations
- `crates/smelt-cli/src/compiler.rs` - Integrate rewriter

### Effort

Medium - requires parser updates, rewriting framework, comprehensive testing

### When to Implement

- When adding real Spark Connect integration
- When users need to run same models on multiple backends
- When adding backends with significant dialect differences (BigQuery, Snowflake)

---

## 🔮 Future Phases (Not Started)

### Phase 5: Named Parameter Compilation

**Value**: Make named parameters functional in CLI execution

**Work**:
- Parse `filter =>` parameter expressions
- Inject as WHERE clause in compiled SQL
- Validate parameter types and compatibility

**Effort**: Medium

---

### Phase 6: Incremental Materialization

**Value**: Faster execution by only recomputing changed data

**Work**:
- Track model state (checksums, timestamps)
- Detect incremental-safe models
- Generate incremental SQL (DELETE+INSERT, MERGE)
- Handle batch boundaries and watermarks

**Design**: See [DESIGN.md](DESIGN.md#incremental-table-builds) for full specification.

**Effort**: Medium-High

---

### Phase 7: Additional Backends

**Candidates**:
- PostgreSQL (via tokio-postgres)
- BigQuery (via google-cloud-bigquery)
- Snowflake (via snowflake-connector-rs)
- Databricks SQL (via REST API)

**Pattern**: Each backend is a new crate implementing Backend trait

**Effort**: Low-Medium per backend (architecture is proven)

---

## ✅ Phase 8: JOIN Syntax Support (COMPLETED)

**Completed**: December 27, 2024

### What Was Implemented

- **Full JOIN syntax support** in parser
  - All JOIN types: INNER, LEFT, RIGHT, FULL OUTER, CROSS
  - ON conditions with expressions
  - USING conditions with column lists
  - Proper error recovery for incomplete JOINs

- **Lexer updates**
  - 9 new keywords: JOIN, INNER, LEFT, RIGHT, FULL, OUTER, CROSS, ON, USING
  - All keywords recognized case-insensitively

- **Parser enhancements**
  - parse_join_clause() with complete JOIN type handling
  - parse_join_condition() for ON and USING clauses
  - Updated parse_from_clause() to parse JOINs instead of comma-separated tables
  - LSP-friendly error recovery maintains usable CST even with partial syntax

- **AST wrappers**
  - JoinClause type with join_type(), table_ref(), and condition() methods
  - JoinType enum (Inner, Left, Right, Full, Cross)
  - JoinCondition type with is_on(), is_using(), on_expression(), using_columns()
  - FromClause::joins() iterator

- **Examples updated**
  - example2_naive.rs and example2_optimized.rs now use explicit CROSS JOIN
  - Comma-separated FROM syntax no longer supported (breaking change)

### Test Results

All 12 parser tests passing, including:
- INNER, LEFT, RIGHT, FULL, CROSS JOIN variants
- ON and USING conditions
- Multiple JOINs in sequence
- Error recovery for missing table refs and conditions

### Breaking Changes

**Removed comma-separated FROM syntax:**
- Old: `FROM users, orders`
- New: `FROM users CROSS JOIN orders`
- Justification: Aligns with design doc requirement for explicit JOIN syntax

---

## ✅ Phase 9: Basic Incremental Materialization (COMPLETED)

**Completed**: December 27, 2024

### What Was Implemented

- **Backend trait enhancements** for incremental updates
  - `MaterializationStrategy` enum (FullRefresh | Incremental)
  - `PartitionSpec` type (column + values for DELETE clause)
  - `execute_model_incremental()` method with strategy parameter
  - `delete_partitions()` and `insert_into_from_query()` primitives

- **DuckDB backend** incremental support
  - DELETE by partition using IN clause with SQL escaping
  - INSERT INTO ... SELECT pattern
  - Auto-creates table on first run (graceful degradation)
  - Spark backend updated with stub implementations

- **SQL model examples** demonstrating materialization strategies
  - `examples/models/transactions.sql` - Source model with timestamped events
  - `examples/models/daily_revenue.sql` - Daily aggregation using incremental materialization
  - Configuration in `examples/smelt.yml` with incremental settings
  - Source data setup with 30 days of transaction data (setup_sources.sql)
  - sources.yml updated with transactions table schema

- **Removed** `smelt-examples` Rust crate
  - Not the right pattern for this project
  - Replaced with SQL model examples in examples/ workspace

- **CLI integration** for incremental execution
  - CLI flags: `--event-time-start` and `--event-time-end` for time range specification
  - Time range parsing and validation (ISO 8601 YYYY-MM-DD format)
  - SQL transformation via `inject_time_filter()` to add WHERE clause filtering
  - Partition date generation from time ranges
  - End-to-end orchestration in `main.rs` (incremental vs full refresh path)

### Implementation Details

**New types** (`crates/smelt-backend/src/types.rs`):
- `PartitionSpec { column: String, values: Vec<String> }` - Specifies which partitions to update
- `MaterializationStrategy::FullRefresh` - DROP + CREATE (existing behavior)
- `MaterializationStrategy::Incremental { partition }` - DELETE + INSERT by partition

**Backend trait** (`crates/smelt-backend/src/lib.rs`):
- `execute_model_incremental()` - Strategy-aware model execution with default implementation
- `delete_partitions()` - DELETE WHERE column IN (values) - trait method, backends implement
- `insert_into_from_query()` - INSERT INTO ... SELECT - trait method, backends implement

**DuckDB backend** (`crates/smelt-backend-duckdb/src/lib.rs`):
- Implements delete_partitions using IN clause with SQL escaping (single quote escape)
- Implements insert_into_from_query using standard SQL
- Auto-creates table on first run if it doesn't exist

**SQL Examples** (`examples/`):
- `models/daily_revenue.sql` - Aggregates transactions by date and user
- `smelt.yml` - Configures incremental: { enabled: true, partition_column: revenue_date }
- `sources.yml` - Defines transactions table schema
- `setup_sources.sql` - Populates 30 days of sample transaction data

**CLI Integration** (`crates/smelt-cli/src/`):
- `main.rs` - Orchestrates incremental vs full refresh execution
  - Parses `--event-time-start` and `--event-time-end` CLI arguments
  - Loads incremental config from `smelt.yml` per model
  - Determines execution strategy (incremental if both config + time range present)
  - Calls `inject_time_filter()` to transform SQL with WHERE clause
  - Generates partition dates using `generate_partition_dates()`
  - Invokes `executor::execute_model_incremental()` with partition spec
- `transformer.rs` - AST-based SQL transformation
  - `inject_time_filter()` adds time range WHERE clause to source queries
  - Uses Rowan parser for precise text replacement
  - Preserves existing WHERE clauses (appends with AND)
- `config.rs` - Incremental configuration types
  - `IncrementalConfig` with `event_time_column` and `partition_column`
  - `Config::get_incremental()` method for per-model settings

### Design Decisions

**DELETE+INSERT vs MERGE**:
- Chose DELETE+INSERT for universal backend support
- MERGE support varies (DuckDB: yes, Spark: Delta only, PostgreSQL: 15+ only)
- DELETE+INSERT works everywhere and is easy to reason about

**Partition specification**:
- Simple string-based partition values (not typed)
- Supports multiple partitions in one operation (IN clause)
- Future: Could add partition expressions, range specifications

**First run handling**:
- Auto-creates table if it doesn't exist (check with table_exists)
- Avoids separate schema management
- Graceful degradation to full refresh on first run

**Configuration in YAML, not SQL comments**:
- Incremental settings in smelt.yml, not annotation parsing
- Avoids need to implement annotation parser (Phase deferred indefinitely)
- Still demonstrates the intent and validates the backend API

### Future Work (Phase 10+)

Phase 9 includes complete end-to-end incremental materialization with CLI integration. Future enhancements could include:

- **Watermark tracking** - Automatically track last processed timestamp and resume from watermark, eliminating need to manually specify time ranges each run
- **Non-date partition support** - Support hourly timestamps, string categories, integer ranges (currently limited to daily date partitions)
- **Auto-detection** - Infer when incremental is safe from SQL semantics
- **Partition inference** - Extract partition column from WHERE clauses automatically
- **Multi-column partitions** - Support composite partition keys (e.g., date + region)
- **MERGE support** - Use MERGE/UPSERT for backends that support it (instead of DELETE+INSERT)

### Test Results

- `cargo clippy --all-targets` passes with no warnings
- Backend trait compiles successfully
- DuckDB backend implements all new methods
- Spark backend updated with stub implementations
- SQL models parse correctly

---

## ✅ Phase 10: Expression Enhancements (COMPLETED)

**Completed**: December 29, 2024

### What Was Implemented

- **CASE expressions** - Both searched and simple forms
  - `CASE WHEN condition THEN result ... ELSE default END` (searched)
  - `CASE expr WHEN value THEN result ... ELSE default END` (simple)
  - Multiple WHEN clauses supported
  - Optional ELSE clause

- **CAST expressions** - Standard SQL and PostgreSQL syntax
  - `CAST(expr AS type)` - Standard SQL syntax
  - `expr::type` - PostgreSQL double-colon operator
  - Type specifications with parameters: `VARCHAR(255)`, `DECIMAL(10,2)`

- **Subqueries** - In SELECT list and FROM clause
  - Scalar subqueries in SELECT: `(SELECT COUNT(*) FROM orders)`
  - Derived tables in FROM: `FROM (SELECT ...) AS alias`
  - Proper SELECT statement parsing within parentheses

- **BETWEEN expressions**
  - `expr BETWEEN low AND high` syntax
  - Expression-based bounds (not just literals)

- **IN expressions** - Both value lists and subqueries
  - Value lists: `status IN ('active', 'pending')`
  - Subqueries: `id IN (SELECT user_id FROM orders)`

- **EXISTS expressions**
  - `EXISTS (SELECT ... FROM ...)` syntax
  - Subquery validation

- **Unary operators** - Negative numbers and NOT
  - Unary minus: `-1`, `-amount`
  - Recursive unary chaining: `--x`
  - NOT operator for boolean negation

### Implementation Details

**Lexer updates** (`crates/smelt-parser/src/lexer.rs`):
- Added 11 new keywords: CASE, WHEN, THEN, ELSE, END, CAST, BETWEEN, IN, EXISTS, ANY, SOME
- Added DOUBLE_COLON (`::`) operator for PostgreSQL casts
- Added MINUS operator (previously missing, causing `-1` to fail)

**Parser enhancements** (`crates/smelt-parser/src/parser.rs`):
- `parse_case_expr()` - Handles both simple and searched CASE forms
- `parse_when_clause()` - Parses WHEN...THEN clauses
- `parse_cast_expr()` - Standard CAST(... AS ...) syntax
- `parse_type_spec()` - Type names with optional parameters
- `parse_subquery()` - SELECT statements in parentheses
- `parse_exists_expr()` - EXISTS (subquery) syntax
- `parse_between_expr()` - BETWEEN low AND high
- `parse_in_expr()` - IN (values/subquery) with discrimination
- `parse_unary_expr()` - Unary minus and NOT operators
- Updated `parse_primary_expr()` to detect CASE, CAST, EXISTS, subqueries, and `::` casts
- Updated `parse_comparison_expr()` to handle BETWEEN and IN
- Updated `parse_table_ref()` to support subqueries in FROM clause
- Updated `at_expression_start()` to include new expression keywords

**AST wrappers** (`crates/smelt-parser/src/ast.rs`):
- `CaseExpr` - with `case_value()`, `when_clauses()`, `else_expr()` methods
- `WhenClause` - with `condition()`, `result()` methods
- `CastExpr` - with `expression()`, `type_spec()`, `is_double_colon_cast()` methods
- `TypeSpec` - with `type_name()`, `full_text()` methods
- `Subquery` - with `select_stmt()` method
- `BetweenExpr` - with `lower_bound()`, `upper_bound()` methods
- `InExpr` - with `is_subquery()`, `subquery()`, `values()` methods
- `ExistsExpr` - with `subquery()` method
- Updated `Expr` with `as_case()`, `as_cast()`, `as_subquery()`, `as_between()`, `as_in()`, `as_exists()` methods

### Test Results

All 29 parser tests passing, including 15 new tests for Phase 10:
- `test_case_searched` - Searched CASE with multiple WHENs
- `test_case_simple` - Simple CASE matching values
- `test_case_no_else` - CASE without ELSE clause
- `test_cast_standard` - CAST(price AS INTEGER)
- `test_cast_postgres_double_colon` - price::INTEGER
- `test_cast_with_params` - CAST(name AS VARCHAR(255))
- `test_cast_decimal` - CAST(amount AS DECIMAL(10, 2))
- `test_subquery_in_select` - Scalar subquery in SELECT list
- `test_subquery_in_from` - Derived table in FROM clause
- `test_between` - price BETWEEN 10 AND 100
- `test_between_with_expressions` - BETWEEN with column references
- `test_in_values` - IN with string literals
- `test_in_numbers` - IN with numeric literals
- `test_in_subquery` - IN with subquery
- `test_exists` - EXISTS with correlated subquery
- `test_complex_nested_expressions` - Combined CASE, cast, IN
- `test_unary_minus` - Negative number literals

### Bug Fixes

- **Fixed missing MINUS operator** - The lexer was not handling `-` as a standalone token, causing it to fall through to ERROR. This made unary minus and negative numbers fail to parse.
- **Fixed expression precedence** - Used `parse_comparison_expr()` in WHEN/THEN clauses instead of `parse_expression()` to avoid consuming keywords like WHEN, ELSE, END.

---

## ✅ Phase 11: Core SQL Clauses (COMPLETED)

**Completed**: December 29, 2024

### What Was Implemented

- **ORDER BY clause** - Comprehensive sorting support
  - Multiple sort expressions: `ORDER BY col1 DESC, col2 ASC`
  - Sort direction: `ASC` / `DESC` (optional, defaults to ASC)
  - Null ordering: `NULLS FIRST` / `NULLS LAST`
  - Expression-based ordering (not just column references)

- **LIMIT clause** - Result set size control
  - Numeric limits: `LIMIT 10`
  - `LIMIT ALL` for explicit unlimited results
  - `OFFSET n` for pagination: `LIMIT 10 OFFSET 20`

- **HAVING clause** - Post-aggregation filtering
  - `HAVING COUNT(*) > 5` after GROUP BY
  - Full expression support (same as WHERE)
  - Proper ordering requirement (must follow GROUP BY)

- **DISTINCT keyword** - Duplicate elimination
  - `SELECT DISTINCT city FROM users`
  - `SELECT ALL` also supported (explicit default)
  - Parsed after SELECT, before column list

- **SELECT without FROM** - Constant expressions
  - `SELECT 1 + 1 AS result`
  - FROM clause now optional in parser
  - Enables calculations and function testing

### Implementation Details

**Lexer updates** (`crates/smelt-parser/src/lexer.rs`):
- Added 11 new keywords: ORDER, LIMIT, OFFSET, HAVING, DISTINCT, ALL, ASC, DESC, NULLS, FIRST, LAST
- All keywords recognized case-insensitively

**Parser enhancements** (`crates/smelt-parser/src/parser.rs`):
- `parse_having_clause()` - HAVING expression parsing
- `parse_order_by_clause()` - Comma-separated ORDER BY items
- `parse_order_by_item()` - Single sort specification with direction and null ordering
- `parse_limit_clause()` - LIMIT value (number/ALL) with optional OFFSET
- Updated `parse_select_stmt()` to handle DISTINCT/ALL and all new clauses
- Updated `at_keyword_that_ends_table_ref()` to include new keywords
- Made FROM clause optional (SELECT without FROM now valid)
- Proper clause ordering enforced: SELECT [DISTINCT] ... [FROM] ... [WHERE] ... [GROUP BY] ... [HAVING] ... [ORDER BY] ... [LIMIT]

**AST wrappers** (`crates/smelt-parser/src/ast.rs`):
- `HavingClause` - with `expression()` method
- `OrderByClause` - with `items()` iterator
- `OrderByItem` - with `expression()`, `direction()`, `null_ordering()` methods
- `SortDirection` enum (Asc, Desc)
- `NullOrdering` enum (First, Last)
- `LimitClause` - with `limit_value()`, `offset_value()` methods
- `LimitValue` enum (Number, All)
- Updated `SelectStmt` with:
  - `having_clause()` method
  - `order_by_clause()` method
  - `limit_clause()` method
  - `is_distinct()` method

**SyntaxKind updates** (`crates/smelt-parser/src/syntax_kind.rs`):
- Added 11 new keyword tokens
- Added 4 new composite node types: HAVING_CLAUSE, ORDER_BY_CLAUSE, ORDER_BY_ITEM, LIMIT_CLAUSE
- Updated `is_keyword()` to include all new keywords

### Test Results

All 43 parser tests passing, including 14 new tests for Phase 11:
- `test_order_by_basic` - Simple ascending sort
- `test_order_by_multiple` - Multiple sort columns
- `test_order_by_nulls` - DESC NULLS LAST
- `test_order_by_nulls_first` - ASC NULLS FIRST
- `test_order_by_expression` - Complex expression ordering (CASE)
- `test_limit_offset` - LIMIT 10 OFFSET 20
- `test_limit_only` - LIMIT without OFFSET
- `test_limit_all` - LIMIT ALL
- `test_having_clause` - Simple HAVING with COUNT
- `test_having_complex_expression` - HAVING with AND
- `test_distinct` - SELECT DISTINCT
- `test_select_all` - SELECT ALL
- `test_complete_query` - All clauses combined
- `test_select_without_from` - SELECT 1 + 1

Cargo clippy passes with no warnings.

### Design Decisions

**FROM clause made optional**:
- Aligns with PostgreSQL and DuckDB behavior
- Enables `SELECT 1 + 1` for testing expressions
- Useful for constant value generation

**HAVING requires GROUP BY semantically but not syntactically**:
- Parser accepts HAVING without GROUP BY (for error recovery)
- Semantic validation should flag this as an error (future work)
- Matches SQL standard error handling approach

**LIMIT ALL vs no LIMIT**:
- Both are valid and equivalent
- LIMIT ALL is explicit about intent
- Useful when overriding default limits

**Expression-based ORDER BY**:
- Supports arbitrary expressions, not just column references
- Enables sorting by CASE expressions, computations, etc.
- Consistent with WHERE and HAVING expression support

---

### Phase 12: Column Schema Tracking (Future)

**Value**: Enable smarter LSP features (autocomplete, validation)

**Work**:
- Track column schemas in smelt-db
- Infer output columns from SELECT
- Validate column references
- LSP autocomplete for column names

**Effort**: Medium-High

---

## Deferred Indefinitely

These features require significant architectural work and are not prioritized:

### Metrics DSL (Spec lines 132-153)
- YAML/declarative metric definitions
- Metric registry and resolution
- Temporal semantics (trailing windows, decomposability)
- Parameter validation

### Type System (Spec lines 183-230)
- Strict type checking
- NULL tracking in types
- LSP quick-fixes for type errors
- Inference within models, explicit at boundaries

### Configuration Annotations (Spec lines 437-464)
- Parse `@materialize`, `@partition_by` annotations
- Store config metadata in AST/database
- Validate configuration options

### Rewrite Rules Framework (Spec lines 284-346)
- Rule framework (Egg or similar)
- Engine-specific translations
- Cost-based optimization

### Learning/Optimization (Spec Phase 6)
- Historical run data
- Optimization suggestions
- Cost modeling

---

## Parser & LSP Status

### ✅ Implemented (Phases 1-11, December 2024)

**Core Features:**
- `smelt.ref()` parsing and validation
- Named parameters (`filter => expr`, `limit => 100`)
- LSP diagnostics for undefined refs
- Go-to-definition for model references
- Incremental compilation via Salsa
- Error recovery in parser

**SQL Syntax (Phases 8, 10, 11):**
- All JOIN types (INNER, LEFT, RIGHT, FULL, CROSS)
- ON and USING conditions
- CASE expressions (both searched and simple forms)
- CAST expressions (standard and PostgreSQL `::` syntax)
- Subqueries (in SELECT and FROM clauses)
- BETWEEN, IN, EXISTS expressions
- Unary operators (-, NOT)
- ORDER BY clause with ASC/DESC and NULLS FIRST/LAST
- LIMIT and OFFSET clauses
- HAVING clause for post-aggregation filtering
- DISTINCT and ALL keywords
- SELECT without FROM (constant expressions)

### ⏸️ Deferred

- `smelt.metric()` support (awaiting metrics design)
- Configuration annotations (`@materialize`, etc.)
- Column-level schema tracking
- Additional SQL syntax (window functions, CTEs, UNION/INTERSECT/EXCEPT)

---

## Contributing

When working on the next phase:

1. **Before starting**: Review the spec in [DESIGN.md](DESIGN.md) for requirements
2. **During development**: Update this roadmap with progress
3. **After completion**: Mark phase as complete with date
4. **Add tests**: Ensure new features have test coverage
5. **Update docs**: Keep CLAUDE.md and comments up to date

See [CLAUDE.md](../CLAUDE.md) for development workflow and architecture notes.
