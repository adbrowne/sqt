# sqt Development Roadmap

This document tracks the implementation status of the sqt parser and LSP, aligned with the spec in [README.md](README.md).

## Current Status

**Phases 1 & 2 Complete**: Basic `sqt.ref()` syntax with named parameters fully implemented.

```sql
-- ✅ Supported syntax
SELECT * FROM sqt.ref('model_name')
SELECT * FROM sqt.ref('events', filter => date > '2024-01-01')
SELECT * FROM sqt.ref('orders', filter => status = 'active', limit => 100)
```

---

## ✅ Phase 1: Basic sqt.ref() Support (COMPLETED)

**Commit**: `084ef71` - "Implement sqt.ref() namespaced syntax per new spec"

### What Was Implemented

- Parser recognizes `sqt.ref('model')` pattern in FROM and expressions
- AST extracts namespace and function name separately
- RefCall strictly validates namespace is "sqt"
- sqt-db and LSP work automatically through AST
- Test workspace updated to `sqt.ref()` syntax

### Key Changes

- **Parser** (`crates/sqt-parser/src/parser.rs`): Handle `IDENT.IDENT()` pattern
- **AST** (`crates/sqt-parser/src/ast.rs`): Add `FunctionCall::namespace()` method
- **RefCall**: Validate `sqt.` namespace prefix
- **Test workspace**: All models use new syntax

---

## ✅ Phase 2: Named Parameters Support (COMPLETED)

**Commit**: `290f39b` - "Implement named parameters with => syntax (Phase 2)"

### What Was Implemented

- ARROW (`=>`) token in lexer
- NAMED_PARAM node type in parser
- `parse_argument()` handles `name => expr` pattern
- NamedParam AST type with `name()` and `value_text()` methods
- `FunctionCall::named_params()` and `RefCall::named_params()` iterators
- Test workspace demonstrates usage

### Key Changes

- **Lexer** (`crates/sqt-parser/src/lexer.rs`): Add `=>` token
- **SyntaxKind** (`crates/sqt-parser/src/syntax_kind.rs`): Add ARROW and NAMED_PARAM
- **Parser** (`crates/sqt-parser/src/parser.rs`): Parse named arguments
- **AST** (`crates/sqt-parser/src/ast.rs`): Add NamedParam type
- **Test workspace**: Example with `filter =>` parameter

---

## ⏸️ Phase 3: sqt.metric() Support (DEFERRED)

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
3. Add `metric_refs()` query to sqt-db
4. Add LSP diagnostics for undefined metrics

---

## Next Steps (Choose One)

### Option A: Improve Parameter Validation ⭐ Recommended

**Value**: Make named parameters more useful immediately

**Work**:
1. Define allowed parameters for `sqt.ref()` per spec:
   - `filter`, `partitions`, `sample`, `max_staleness`, `as_of`, `version`, etc.
2. Add LSP diagnostics for unknown parameters
3. Add LSP autocomplete for parameter names
4. Add hover documentation for each parameter

**Effort**: Low-Medium (LSP-focused, no parser changes)

**Files**: `crates/sqt-lsp/src/main.rs`, `crates/sqt-db/src/lib.rs`

---

### Option B: Column Schema Tracking

**Value**: Enable smarter LSP features (autocomplete, validation)

**Work**:
1. Track column schemas in sqt-db
2. Infer output columns from SELECT statements
3. Validate column references in expressions
4. Add LSP autocomplete for column names

**Effort**: Medium (requires SQL analysis logic)

**Files**: `crates/sqt-db/src/lib.rs`, `crates/sqt-lsp/src/main.rs`

---

### Option C: Testing Framework

**Value**: Confidence in parser/AST correctness

**Work**:
1. Add parser tests for various SQL patterns
2. Add AST tests for ref/parameter extraction
3. Add integration tests for LSP features
4. Test error recovery scenarios

**Effort**: Low-Medium (infrastructure setup)

**Files**: New test files in `crates/sqt-parser/tests/`, `crates/sqt-lsp/tests/`

---

### Option D: VSCode Extension Improvements

**Value**: Better developer experience

**Work**:
1. Add syntax highlighting for `sqt.ref()` and parameters
2. Add snippets for common patterns
3. Improve error message formatting
4. Add "Find All References" support

**Effort**: Low (VSCode extension work)

**Files**: `editors/vscode/syntaxes/`, `editors/vscode/package.json`

---

### Option E: Documentation & Examples

**Value**: Help future users/contributors

**Work**:
1. Document parser architecture
2. Document AST traversal patterns
3. Add more test workspace examples
4. Write getting-started guide

**Effort**: Low (documentation)

**Files**: New docs in `docs/`, expanded examples in `test-workspace/`

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

1. **Before starting**: Review the spec in [README.md](README.md) for requirements
2. **During development**: Update this roadmap with progress
3. **After completion**: Mark phase as complete with commit hash
4. **Add tests**: Ensure new features have test coverage
5. **Update docs**: Keep CLAUDE.md and comments up to date

See [CLAUDE.md](CLAUDE.md) for development workflow and architecture notes.
