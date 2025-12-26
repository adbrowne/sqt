# smelt Development Roadmap

This document tracks the implementation status of smelt, aligned with the spec in [DESIGN.md](DESIGN.md).

## Current Status

**Phases 1, 2, & 3 Complete**: Parser, LSP, and CLI with DuckDB execution fully implemented.

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
```

---

## ✅ Phase 1: Basic smelt.ref() Support (COMPLETED)

**Completed**: December 20, 2025

### What Was Implemented

- Parser recognizes `smelt.ref('model')` pattern in FROM and expressions
- AST extracts namespace and function name separately
- RefCall strictly validates namespace is "smelt"
- smelt-db and LSP work automatically through AST
- Test workspace updated to `smelt.ref()` syntax

### Key Changes

- **Parser** (`crates/smelt-parser/src/parser.rs`): Handle `IDENT.IDENT()` pattern
- **AST** (`crates/smelt-parser/src/ast.rs`): Add `FunctionCall::namespace()` method
- **RefCall**: Validate `sqt.` namespace prefix
- **Test workspace**: All models use new syntax

---

## ✅ Phase 2: Named Parameters Support (COMPLETED)

**Completed**: December 20, 2025

### What Was Implemented

- ARROW (`=>`) token in lexer
- NAMED_PARAM node type in parser
- `parse_argument()` handles `name => expr` pattern
- NamedParam AST type with `name()` and `value_text()` methods
- `FunctionCall::named_params()` and `RefCall::named_params()` iterators
- Test workspace demonstrates usage

### Key Changes

- **Lexer** (`crates/smelt-parser/src/lexer.rs`): Add `=>` token
- **SyntaxKind** (`crates/smelt-parser/src/syntax_kind.rs`): Add ARROW and NAMED_PARAM
- **Parser** (`crates/smelt-parser/src/parser.rs`): Parse named arguments
- **AST** (`crates/smelt-parser/src/ast.rs`): Add NamedParam type
- **Test workspace**: Example with `filter =>` parameter

---

## ✅ Phase 3: CLI and DuckDB Execution (COMPLETED)

**Completed**: December 26, 2025

### What Was Implemented

- New `smelt-cli` crate with `sqt run` command
- DuckDB-based model execution with file-based database persistence
- YAML configuration (`smelt.yml` and `sources.yml`)
- Model discovery from `models/` directory
- Dependency graph construction with topological sort
- SQL compilation (replacing `smelt.ref()` with table references)
- Table and view materialization strategies
- Source table validation
- Named parameter detection with clear error messages
- Test workspace configuration for end-to-end testing

### Key Features

- **CLI**: Full-featured command-line interface using `clap`
  - `sqt run` - Execute models and materialize in DuckDB
  - `--project-dir` - Specify project root
  - `--database` - Custom database file path
  - `--show-results` - Preview query results
  - `--verbose` - Show compiled SQL
  - `--dry-run` - Validate without executing

- **Configuration**: YAML-based project configuration
  - `smelt.yml` - Project settings, targets, model materialization
  - `sources.yml` - External source table definitions

- **Execution Engine**:
  - Dependency resolution with cycle detection
  - Topological sort for correct execution order
  - Both CREATE TABLE AS and CREATE VIEW support
  - Row counts and timing statistics
  - Pretty-printed result preview using Arrow

- **Error Handling**:
  - Clear error messages with file/line/column positions
  - Named parameter detection with helpful error messages
  - Undefined reference validation
  - Circular dependency detection
  - Source table validation

### Implementation Details

**New Files**:
- `crates/smelt-cli/Cargo.toml` - CLI crate definition
- `crates/smelt-cli/src/main.rs` - Entry point and orchestration
- `crates/smelt-cli/src/lib.rs` - Public API
- `crates/smelt-cli/src/config.rs` - YAML configuration parsing
- `crates/smelt-cli/src/discovery.rs` - Model file discovery
- `crates/smelt-cli/src/graph.rs` - Dependency graph and topological sort
- `crates/smelt-cli/src/compiler.rs` - SQL compilation (ref replacement)
- `crates/smelt-cli/src/executor.rs` - DuckDB execution engine
- `crates/smelt-cli/src/errors.rs` - Custom error types

**Test Configuration**:
- `test-workspace/smelt.yml` - Example project configuration
- `test-workspace/sources.yml` - Source table definitions
- `test-workspace/setup_sources.sql` - Sample data generation

### Limitations (By Design)

- Named parameters in `smelt.ref()` are detected but not yet compiled
  - Gives clear error: "named parameters which are not yet supported"
  - Can be implemented in future phase
- JOIN syntax not yet supported in parser
  - Parser currently only handles comma-separated table references
  - Use `FROM table1, table2 WHERE ...` instead of `FROM table1 JOIN table2 ON ...`
  - Can be implemented in future phase
- No incremental materialization (always full refresh)
- Single-threaded execution (no parallel model execution)
- DuckDB only (multi-backend support deferred)

### Test Results

Successfully executed test-workspace models:
- `raw_events` - 100 rows (table)
- `user_sessions` - 33 rows (view)
- `user_stats` - 10 rows (table)

All executed in ~8ms with correct dependency resolution.

---

## ⏸️ Phase 4: smelt.metric() Support (DEFERRED)

**Status**: Deferred until metrics DSL design is finalized

### Why Deferred

Requires architectural decisions about:
- Metric definition format (YAML? SQL? Other?)
- Metric storage and resolution strategy
- Parameter validation semantics
- Temporal computation model (trailing windows, decomposability)

### When Ready

Can follow similar pattern to RefCall implementation:
1. Add `MetricCall` AST type
2. Add `File::metrics()` iterator
3. Add `metric_refs()` query to smelt-db
4. Add LSP diagnostics for undefined metrics

---

## 🔄 Phase 4: Spark Backend (NEXT)

**Status**: Next priority

### Why Spark Next

Adding a second backend now validates the multi-backend architecture before building more features on top. Key architectural questions to answer:

1. **Backend abstraction**: How to abstract SQL dialect differences?
2. **Connection management**: How to configure and manage connections?
3. **Execution model**: How to handle remote execution vs local?
4. **Type mapping**: How to map types between backends?

### What to Implement

1. **Backend trait** - Abstract interface for execution backends
   ```rust
   trait Backend {
       fn execute(&self, sql: &str) -> Result<ExecutionResult>;
       fn create_table(&self, name: &str, sql: &str) -> Result<()>;
       fn supports(&self, capability: Capability) -> bool;
   }
   ```

2. **Spark executor** - Databricks/Spark SQL execution
   - Connect via Databricks SQL connector or Spark Connect
   - Handle Spark-specific SQL dialect differences
   - Map Arrow types between systems

3. **Target configuration** - Per-model or global target selection
   ```yaml
   # smelt.yml
   targets:
     dev:
       backend: duckdb
       database: dev.db
     prod:
       backend: spark
       host: my-workspace.databricks.com
       warehouse_id: abc123
   ```

4. **CLI target selection**
   ```bash
   smelt run --target prod
   smelt run --target dev  # default
   ```

### Dialect Differences to Handle

| Feature | DuckDB | Spark SQL |
|---------|--------|-----------|
| String concat | `\|\|` | `CONCAT()` or `\|\|` |
| Date functions | `DATE '2024-01-01'` | `DATE('2024-01-01')` |
| QUALIFY | ✅ Native | ❌ Subquery rewrite |
| MERGE | ✅ Native | ✅ Delta Lake |
| Array syntax | `[1, 2, 3]` | `ARRAY(1, 2, 3)` |

### Effort

Medium-High - requires new crate, connection handling, dialect abstraction

### Files

- `crates/smelt-backend/` - New crate for backend abstraction
- `crates/smelt-cli/src/executor.rs` - Refactor to use backend trait
- `crates/smelt-cli/src/config.rs` - Add target configuration

---

## Other Options (Deferred)

These are valuable but deferred until multi-backend architecture is validated:

### Named Parameter Compilation

**Value**: Make named parameters functional in CLI execution

**Work**: Implement `filter =>` parameter compilation - parse filter expression, inject as WHERE clause.

**Effort**: Medium

---

### Incremental Materialization

**Value**: Faster execution by only recomputing changed data

**Work**: Track model state, detect incremental-safe models, generate incremental SQL. See [DESIGN.md](DESIGN.md#incremental-table-builds) for full design.

**Effort**: Medium-High

---

### JOIN Syntax Support

**Value**: Enable standard SQL JOIN syntax

**Work**: Add JOIN keywords to lexer, update parser, add AST support.

**Effort**: Medium

---

### Column Schema Tracking

**Value**: Enable smarter LSP features (autocomplete, validation)

**Work**: Track column schemas, infer output columns, validate references.

**Effort**: Medium

---

## Future Work (Not Prioritized)

These features require significant architectural work and are deferred:

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

### Multi-Backend Support (Spec lines 256-272)
- Backend capability declarations
- Computation requirement tracking
- Backend-specific rewrites

### Rewrite Rules (Spec lines 284-346)
- Rule framework (possibly using Egg or similar)
- Engine-specific translations
- Cost-based optimization

### Incrementalization (Spec Phase 5)
- Batch safety proofs
- Incremental materialization
- State management

### Learning/Optimization (Spec Phase 6)
- Historical run data
- Optimization suggestions
- Cost modeling

---

## Contributing

When working on the next phase:

1. **Before starting**: Review the spec in [DESIGN.md](DESIGN.md) for requirements
2. **During development**: Update this roadmap with progress
3. **After completion**: Mark phase as complete with date
4. **Add tests**: Ensure new features have test coverage
5. **Update docs**: Keep CLAUDE.md and comments up to date

See [CLAUDE.md](../CLAUDE.md) for development workflow and architecture notes.
