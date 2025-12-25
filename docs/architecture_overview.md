# Architecture Overview: Putting It All Together

## The Big Picture

smelt combines three powerful ideas:

1. **Logical/Physical Separation**: Users write WHAT to compute, optimizer decides HOW
2. **Cross-Model Optimization**: Detect and optimize patterns across multiple models
3. **First-Class Editor Support**: LSP + Salsa for instant feedback and incremental compilation

## How They Work Together

```
┌─────────────────────────────────────────────────────────────────┐
│                        Developer Experience                      │
│  ┌────────────┐                                                  │
│  │   Editor   │ ← Real-time feedback via LSP                    │
│  │  (VSCode)  │ ← Diagnostics, completions, refactoring         │
│  └──────┬─────┘                                                  │
│         │                                                        │
│         │ File changes                                           │
│         ▼                                                        │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │              LSP Server + Salsa Database                 │   │
│  │  • Incremental parsing (Rowan CST)                       │   │
│  │  • Incremental semantic analysis                         │   │
│  │  • Incremental optimization                              │   │
│  └──────┬───────────────────────────────────────────────────┘   │
└─────────┼──────────────────────────────────────────────────────┘
          │
          │ Model definitions + dependencies
          ▼
┌─────────────────────────────────────────────────────────────────┐
│                      Compilation Pipeline                        │
│  ┌────────┐    ┌────────┐    ┌──────────┐    ┌──────────────┐  │
│  │ Parser │ →  │  AST   │ →  │ Semantic │ →  │ Logical IR   │  │
│  │ (Rowan)│    │        │    │ Analysis │    │ (DataFusion) │  │
│  └────────┘    └────────┘    └──────────┘    └──────┬───────┘  │
│                                                       │          │
│                                                       │          │
│  ┌────────────────────────────────────────────────────┘          │
│  │                                                               │
│  ▼                                                               │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │              Optimizer (User-Defined Rules)              │   │
│  │                                                           │   │
│  │  Example Rule: Common Intermediate Aggregation           │   │
│  │  ┌────────────────────────────────────────────────────┐  │   │
│  │  │ 1. Detect: Find models with same session logic    │  │   │
│  │  │ 2. Analyze: Compute union of required dimensions  │  │   │
│  │  │ 3. Rewrite: Create shared materialization         │  │   │
│  │  │ 4. Update: Point models to shared table           │  │   │
│  │  └────────────────────────────────────────────────────┘  │   │
│  │                                                           │   │
│  └──────────────────────────┬────────────────────────────────┘   │
│                             │                                    │
│                             ▼                                    │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │           Physical IR (Execution Plan)                   │   │
│  │  • Which models to materialize                           │   │
│  │  • Which backend for each operation                      │   │
│  │  • Execution order                                       │   │
│  └──────────────────────────┬────────────────────────────────┘   │
└─────────────────────────────┼──────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                         Code Generation                          │
│  ┌────────────┐    ┌────────────┐    ┌────────────────────┐    │
│  │  DuckDB    │    │ Databricks │    │  Spark / BigQuery  │    │
│  │  Backend   │    │  Backend   │    │     (Future)       │    │
│  └────────────┘    └────────────┘    └────────────────────┘    │
└─────────────────────────────────────────────────────────────────┘
```

## Example: Developer Workflow

### 1. Developer Writes Models

```sql
-- models/user_sessions.sql
{{ config(description="User sessions with 30-min timeout") }}

SELECT
    user_id,
    session_id,
    COUNT(*) as events
FROM {{ ref('raw_events') }}
GROUP BY user_id, session_id
```

### 2. LSP Provides Instant Feedback

As they type:
- ✅ `ref('raw_events')` → autocomplete suggests available models
- ✅ Hover shows `raw_events` schema
- ✅ Diagnostic: "Column `session_id` not found in `raw_events`"
- ❌ Developer fixes: needs to compute sessions first

### 3. Developer Refactors (LSP-Assisted)

```sql
-- Updated version
WITH sessions AS (
    SELECT
        user_id,
        SUM(CASE WHEN ... THEN 1 ELSE 0 END) OVER (...) as session_id
    FROM {{ ref('raw_events') }}
)
SELECT user_id, session_id, COUNT(*) as events
FROM sessions
GROUP BY user_id, session_id
```

LSP updates in real-time:
- ✅ No more diagnostic errors
- ✅ Schema inferred correctly
- ℹ️ Inlay hint: "3 other models use similar session logic"

### 4. Developer Adds More Models

```sql
-- models/sessions_by_country.sql
SELECT country, COUNT(*) FROM {{ ref('user_sessions') }} ...

-- models/sessions_by_hour.sql
SELECT hour, COUNT(*) FROM {{ ref('user_sessions') }} ...
```

### 5. Optimizer Detects Pattern (Via Salsa)

When developer saves files, Salsa incrementally:
1. Reparses changed files (~1ms each)
2. Updates dependency graph (~10ms)
3. Runs optimization queries (~50ms)

Optimizer rule detects:
- All three models need same session computation
- Can share intermediate `session_summary` table

### 6. LSP Shows Optimization Opportunity

Inline diagnostic in editor:
```
ℹ️ Optimization available: 3 models share session computation
   → Click to materialize 'session_summary'
```

Code action:
```
💡 Create shared materialization
   • Reduces computation by 67%
   • Estimated cost: $10 → $3.50
```

### 7. Developer Accepts Optimization

LSP/Optimizer generates:
```sql
-- Generated: models/_internal/session_summary.sql
{{ config(materialized='table', internal=true) }}

SELECT
    user_id,
    session_id,
    session_day,
    session_hour,
    session_country,
    COUNT(*) as events,
    SUM(revenue) as revenue
FROM {{ ref('raw_events') }}
GROUP BY ...
```

And updates dependent models:
```sql
-- models/user_sessions.sql (updated)
SELECT user_id, session_id, events
FROM {{ ref('_session_summary') }}
```

### 8. Execution

```bash
$ smelt run --target production

Compiling pipeline...
  ✓ Parsed 47 models (120ms)
  ✓ Resolved dependencies (15ms)
  ✓ Applied 3 optimizations (45ms)
  ✓ Generated SQL (80ms)

Executing on Databricks...
  → session_summary (Spark, 45s, $2.10)
  ↳ user_sessions (DuckDB, 0.2s, $0.00)
  ↳ sessions_by_country (DuckDB, 0.1s, $0.00)
  ↳ sessions_by_hour (DuckDB, 0.1s, $0.00)

Total: 45.4s, $2.10 (saved $7.90 vs naive execution)
```

## Key Technical Decisions

### Salsa for Incremental Compilation

**Why**: Developer changes one model → only recompute affected models
- Parse all 1000 models once: ~1s
- Change one model: recompile only dependents: ~50ms
- Critical for LSP responsiveness

**How**: Salsa tracks query dependencies automatically
```rust
fn dependency_graph(&self) -> Arc<Graph> {
    // Automatically depends on all_models()
    let models = self.all_models();  // Salsa query
    build_graph(models)
}
```

When a file changes:
1. Salsa marks `file_text()` query as dirty
2. Automatically invalidates `parse_file()`, `file_ast()`, `all_models()`, `dependency_graph()`
3. Recomputes only what's needed

### Rowan for Error Recovery

**Why**: Developers type invalid code 99% of the time
- Must provide completions even with syntax errors
- Must show diagnostics without blocking on errors

**How**: Rowan produces CST even with errors
```sql
{{ ref('user_  -- incomplete!
```

Parser produces:
```
REF_EXPR
  ├─ REF_KW("ref")
  ├─ LPAREN("(")
  ├─ STRING("'user_") ← incomplete
  └─ ERROR         ← missing closing paren and }}
```

LSP can still:
- Suggest completions: "user_sessions", "user_events"
- Provide diagnostic: "Unclosed string literal"
- Show go-to-definition (even though ref is incomplete)

### Explicit Optimization Rules

**Why**: Transparency and debuggability
- Developer can see what optimizations are applied
- Can write custom rules for domain-specific patterns
- Can disable rules that cause issues

**How**: Rules are first-class values
```rust
let rule = OptimizationRule {
    name: "share_sessions",
    pattern: |ctx| ctx.find_common_cte("sessions"),
    rewrite: |ctx| ctx.create_shared_table(...),
    enabled: true,
};

optimizer.add_rule(rule);
```

IDE shows:
```
Applied optimizations:
  ✓ share_sessions → session_summary (3 consumers)
  ✗ split_large_groupby → disabled (small dataset)
```

## What's Next

1. **Finish Example 1 Analysis** ✅ (Done!)
   - Documented insights for common intermediate aggregation
   - API design recommendations

2. **Build Example 2**: Split Large GROUP BY
   - Different optimization pattern
   - Validate rule API generalizes

3. **Prototype Salsa + Rowan Parser**
   - Basic template syntax (`{{ ref() }}`, `{{ config() }}`)
   - Error recovery
   - Salsa queries for parsing

4. **Implement Basic LSP**
   - Diagnostics (parse errors)
   - Go-to-definition for `ref()`
   - Model name completions

5. **Design Optimization Rule API**
   - Based on Examples 1 & 2
   - Pattern matching primitives
   - Rewrite operations

6. **Build Optimizer Framework**
   - Rule registration
   - Pattern detection
   - Physical plan generation

This architecture gives us:
- **Fast feedback**: Salsa + LSP = sub-100ms edit-to-diagnostic
- **Powerful optimization**: Cross-model pattern detection
- **Great DX**: Real-time help, no mental overhead
- **Scalability**: Incremental compilation handles 1000s of models
