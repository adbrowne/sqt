# smelt Development Roadmap

This document tracks the implementation status of smelt, aligned with the spec in [DESIGN.md](DESIGN.md).

## Current Status

**Multi-Backend Architecture Complete**: Parser, LSP, and multi-backend CLI with DuckDB and Spark (stub) implementations.

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

### Phase 9: Column Schema Tracking

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

### ✅ Implemented (Phases 1-3, December 2025)

- `smelt.ref()` parsing and validation
- Named parameters (`filter => expr`)
- LSP diagnostics for undefined refs
- Go-to-definition for model references
- Incremental compilation via Salsa
- Error recovery in parser

### ⏸️ Deferred

- `smelt.metric()` support (awaiting metrics design)
- JOIN syntax parsing
- Configuration annotations (`@materialize`, etc.)
- Column-level schema tracking

---

## Contributing

When working on the next phase:

1. **Before starting**: Review the spec in [DESIGN.md](DESIGN.md) for requirements
2. **During development**: Update this roadmap with progress
3. **After completion**: Mark phase as complete with date
4. **Add tests**: Ensure new features have test coverage
5. **Update docs**: Keep CLAUDE.md and comments up to date

See [CLAUDE.md](../CLAUDE.md) for development workflow and architecture notes.
