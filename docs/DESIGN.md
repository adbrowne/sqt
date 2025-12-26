# smelt Design Specification

## Overview

smelt is a data transformation framework that separates **logical transformation definitions** from **physical execution planning**. Unlike traditional tools like dbt that use SQL templates, smelt parses and understands the semantics of transformations, enabling advanced capabilities like automatic refactoring, cross-engine deployment, and intelligent incrementalization.

### Core Philosophy

1. **Analysts define what** - Pure logical models expressing business intent
2. **Engineers define how** - Rewrite rules and execution configuration
3. **Framework mediates** - Validates, optimizes, and deploys to target engines

### Key Differentiators from dbt

| Aspect | dbt | smelt |
|--------|-----|-----|
| Model definition | Jinja templates | Parsed semantic models |
| Type checking | None (runtime errors) | Static analysis with LSP support |
| Cross-engine | One target per project | Split work across engines |
| Optimization | Manual | Rule-based with learning |
| Incrementalization | Manual `is_incremental()` | Semantic analysis of what's safe |

---

## Target Execution Engines

Initial targets (in priority order):
1. **DuckDB** - Local development, small-medium datasets
2. **PostgreSQL** - Traditional warehouse workloads
3. **Databricks/Spark** - Large-scale distributed processing
4. **DataFusion** - Direct logical plan emission (skip SQL)

Future considerations:
- Flink (streaming)
- Snowflake, BigQuery (cloud warehouses)

---

## Language Design

### Decision: SQL-Based with Extensions

The logical model language is **SQL with smelt-specific extensions**. This choice prioritizes:
- Familiarity for data engineers
- Lower adoption barrier
- Incremental migration from existing SQL

#### Alternatives Considered (Not Chosen)

| Alternative | Pros | Cons | Status |
|-------------|------|------|--------|
| PRQL | Pipeline syntax, less verbose | New syntax to learn, smaller ecosystem | Deferred - could add as frontend later |
| Malloy | Clean semantics, symmetric aggregates | Different execution model, no orchestration | Inspiration only |
| KQL/Kusto | Pipeline syntax, popular for logs | Microsoft-specific heritage | Not pursued |
| Custom DSL | Full control | High investment, adoption friction | Not pursued |

### Extension Syntax: `smelt.*` Functions

Model and metric references use a function-like syntax with the `smelt.` namespace prefix:

```sql
-- Model references
SELECT * FROM smelt.ref('upstream_model')

-- With parameters using => for named arguments (SQL:2003 standard)
SELECT * FROM smelt.ref('upstream_model', filter => event_date > '2024-01-01')

-- Metric references
SELECT
  user_id,
  smelt.metric('monthly_active_users', at => event_date) as mau
FROM events
```

#### Why This Syntax

- **Namespaced**: `smelt.` prefix avoids collision with real UDFs
- **Function-like**: Natural parameter passing with `=>` (standard SQL named parameters)
- **Extensible**: Easy to add `smelt.param()`, `smelt.config()`, etc.
- **Parseable**: Can be handled by extending standard SQL parser

#### Alternatives Considered (Not Chosen)

| Syntax | Example | Reason Not Chosen |
|--------|---------|-------------------|
| Jinja templates | `{{ ref('model') }}` | No static analysis, poor error messages |
| Schema namespace | `smelt.models.upstream` | Less natural for parameters |
| `@` prefix | `@ref('model')` | Potential collision with SQL variables |
| `$` prefix | `$metric.revenue` | Less familiar, edge cases in shells |

### Reference Parameters

```sql
smelt.ref('model',
  -- Data filtering
  filter => <expr>,              -- Pushdown predicate
  partitions => ['2024-01'],     -- Explicit partition list
  sample => 0.1,                 -- Sampling ratio

  -- Freshness/versioning
  max_staleness => '1 hour',     -- Acceptable data age
  as_of => '2024-01-01',         -- Time travel
  version => 'v2',               -- Explicit model version

  -- Optimizer hints
  prefer_materialized => true,   -- Use cache if available
  allow_approximate => true,     -- OK to use approximations
  inline => true                 -- Don't materialize, inline SQL
)

smelt.metric('metric_name',
  at => event_date,              -- Point-in-time evaluation
  for => user_id,                -- Entity to compute for
  grain => 'daily',              -- Rollup level
  allow_approximate => true      -- HLL acceptable
)
```

---

## Two-Layer DSL Architecture

### Layer 1: Metrics DSL (Declarative, Non-SQL)

Captures semantic intent for reusable business metrics. Carries metadata about temporal behavior, statefulness, and decomposability.

```yaml
# Proposed syntax (exact format TBD)
metric monthly_active_users:
  entity: user
  measure: count_distinct(user_id)
  time_grain: day
  period_type: trailing(28 days)
  decomposable: false  # Cannot be computed incrementally per-partition

metric revenue:
  entity: order
  measure: sum(amount)
  dimensions:
    - currency
  decomposable: true  # SUM can be merged across partitions

metric first_touch_attribution:
  entity: user
  event: conversion
  attribute_to: first_in_period(campaign_touch, period: 30 days)
  requires: session_state
```

### Layer 2: SQL Models (Expressive, Familiar)

Use standard SQL with smelt extensions to compose metrics and build complex transformations.

```sql
-- Model: daily_user_stats
-- @materialize: daily
-- @partition_by: event_date

SELECT
  event_date,
  user_id,
  smelt.metric('revenue', at => event_date) as daily_revenue,
  smelt.metric('monthly_active_users', at => event_date) as mau_at_date
FROM smelt.ref('events')
GROUP BY 1, 2
```

### Why Two Layers

1. **Metrics are reusable** - Same definition used across many models
2. **Metrics carry semantics** - Framework knows MAU is trailing-window, revenue is decomposable
3. **SQL stays familiar** - Engineers don't need to learn everything new
4. **Clear optimization boundary** - Metrics heavily optimized, SQL more pass-through

---

## Type System

### Design: Strict with LSP Quick-Fixes

The type system is strict (inspired by Haskell) but the LSP provides quick-fixes to reduce friction. The goal: committed code is strict, authoring experience is fluid.

```sql
-- User writes:
SELECT a + b FROM t  -- Error: a is VARCHAR, b is INT

-- LSP offers quick-fix, user accepts:
SELECT CAST(a AS INT) + b FROM t  -- Explicit, correct
```

### Key Type System Features

1. **NULL tracking in types**
   - `DECIMAL NOT NULL` vs `DECIMAL NULL`
   - LEFT JOIN automatically promotes to nullable
   - LSP suggests COALESCE when needed

2. **Inference within models, explicit at boundaries**
   - Types inferred for intermediate expressions
   - Input/output schemas must be explicit
   - Similar to Rust: inference in functions, signatures explicit

3. **Superset types with backend validation**
   - IR can represent types not supported everywhere (e.g., HUGEINT)
   - Error raised only when deploying to a backend that doesn't support it

4. **Literal handling**
   - Numeric literals polymorphic within numeric tower
   - String-to-number coercion always explicit

### SQL Mistakes to Avoid

| SQL Problem | smelt Approach |
|-------------|--------------|
| NULL = NULL is NULL | Require explicit IS NULL checks |
| Implicit type coercion | Require explicit CAST |
| UNION positional matching | UNION BY NAME, error on mismatch |
| SELECT * | Disallow or require explicit opt-in |
| Ambiguous column resolution | Always error, require qualification |
| Non-deterministic GROUP BY (MySQL) | Error on non-aggregated, non-grouped columns |
| ORDER BY in subqueries | Warn or error (meaningless) |
| Implicit CROSS JOIN | Require explicit CROSS JOIN |
| Timestamp timezone ambiguity | Only naive datetime and instant (with tz) |
| Integer division ambiguity | Explicit integer vs decimal division |

---

## Intermediate Representation (IR)

### Computation Requirements

The IR tracks what each computation *requires* semantically, not how to execute it:

```rust
enum ComputationRequirement {
    Stateless,           // Pure function of current row
    PartitionLocal,      // Independent per partition key
    RequiresOrdering,    // Needs rows in specific order within partition
    Windowed {           // Needs N prior/future rows
        preceding: WindowBound,
        following: WindowBound,
    },
    Sessionized {        // Gap-based grouping
        key: Vec<Column>,
        gap: Duration,
    },
    Cumulative,          // Depends on all prior rows
}
```

### Backend Capability Declaration

Each backend declares what it supports:

```rust
struct BackendCapabilities {
    supports_stateless: bool,
    supports_partition_local: bool,
    supports_ordering: bool,
    supports_windowed: bool,
    supports_sessionized: bool,      // Spark: native, DuckDB: via rewrite
    supports_cumulative: bool,
    supports_types: HashSet<DataType>,
    // ... engine-specific capabilities
}
```

### Validation Flow

1. Parse SQL + extensions → AST
2. Resolve references (models, metrics) → Typed AST
3. Analyze computation requirements → Annotated IR
4. Match against target backend → Error or rewrite plan

---

## Rewrite Rules

### Design: Engine-Specific Translations

Rewrite rules translate semantic concepts to engine-specific implementations. They live in the framework, not in user models.

```python
# Example: Sessionization
@rule
def sessionization_spark(node: SessionizedComputation, target: SparkBackend):
    """Native session_window in Spark"""
    return SparkSessionWindow(
        keys=node.keys,
        gap=node.gap,
        timestamp_col=node.timestamp
    )

@rule(complexity="high")
def sessionization_duckdb(node: SessionizedComputation, target: DuckDBBackend):
    """Implement via window functions"""
    return WindowBasedSessionization(
        flag_expr=f"""
            CASE WHEN {node.timestamp} - LAG({node.timestamp}) OVER (
                PARTITION BY {', '.join(node.keys)} ORDER BY {node.timestamp}
            ) > INTERVAL '{node.gap}'
            THEN 1 ELSE 0 END
        """,
        session_id_expr="SUM(flag) OVER (PARTITION BY ... ORDER BY ...)"
    )
```

### Common Rewrites Needed

| Concept | Native Support | Rewrite For Others |
|---------|----------------|-------------------|
| Session windows | Spark, Flink | Window function pattern |
| QUALIFY | DuckDB, Snowflake, Databricks | Subquery with WHERE |
| PIVOT/UNPIVOT | Snowflake, Databricks | CASE expression expansion |
| MERGE/upsert | Most modern engines | DELETE + INSERT |
| Approx count distinct | BigQuery, Spark | HyperLogLog UDF or exact |
| HUGEINT (128-bit) | DuckDB | NUMERIC/DECIMAL elsewhere |
| Recursive CTEs | Postgres, DuckDB, Spark 3.x | Iterative unrolling (limited) |

### Prior Art for Rewrite Systems

| System | Relevance | Key Ideas |
|--------|-----------|-----------|
| **Egg** (e-graphs) | Rule framework | Equality saturation, avoid ordering issues |
| **MLIR** | Multi-level IR | Progressive lowering, dialects |
| **Apache Calcite** | Query optimization | RelOptRule, cost-based selection |
| **DataFusion optimizer** | Rust-native | Simple OptimizerRule trait |
| **Substrait** | Cross-engine IR | Portable plan representation |

### Rule Interface (Proposed)

Rust core with Python bindings for rule authoring:

```python
@rule
def my_rule(node: CountDistinct, ctx: RuleContext) -> Optional[Rewrite]:
    if ctx.target.supports(ApproxCountDistinct) and ctx.has_annotation('approximate_ok'):
        return HyperLogLog(node.column, precision=14)
    return None  # No rewrite, use default
```

---

## Execution Planning

### ETL Optimization Context

Unlike ad-hoc query optimization, ETL has different constraints:

| Ad-hoc Query | ETL Pipeline |
|--------------|--------------|
| Optimize in ms | Can afford hours of analysis |
| No prior knowledge | Historical run data available |
| Run once | Run daily for years |
| User waiting | Scheduled, unattended |

### Features Enabled by This Context

1. **Pre-run analysis**
   ```bash
   smelt optimize --model daily_stats --budget 4h --sample-data s3://...
   # Outputs learned configuration to .smelt/optimizations/
   ```

2. **Learning from history**
   - Record row counts, shuffle sizes, spill events per run
   - Use historical stats instead of gathering fresh ones
   - Detect patterns (consistent spill → suggest rule)

3. **Human-in-the-loop**
   - Expensive pipelines may warrant manual tuning
   - Framework suggests, engineer confirms

4. **Stored optimization decisions**
   - Persist learned configs across runs
   - Version alongside model definitions

### Batch Processing Intelligence

The framework can prove when batching is safe for backfills:

```sql
-- If model is partition-independent:
--   - All window functions partitioned by batch key
--   - No self-joins across batch boundaries
--   - Aggregations are batch-local or decomposable
-- Then: 90-day backfill = 1 query, not 90 queries
```

Can also transform queries to *make* them batch-safe:

```sql
-- Original (not batch-safe)
ROW_NUMBER() OVER (PARTITION BY user_id ORDER BY ts)

-- Rewritten for batch (if semantics allow)
ROW_NUMBER() OVER (PARTITION BY user_id, batch_date ORDER BY ts)
```

---

## Configuration Layers

### Separation of Concerns

```
┌─────────────────────────────────────────┐
│  Logical Model (analyst)                │
│  - Pure business logic                  │
│  - SQL + smelt.ref/smelt.metric         │
│  - No execution hints                   │
└─────────────────────────────────────────┘
                    │
                    ▼
┌─────────────────────────────────────────┐
│  Execution Config (engineer)            │
│  - Materialization strategy             │
│  - Backend hints                        │
│  - Optimization budget                  │
└─────────────────────────────────────────┘
                    │
                    ▼
┌─────────────────────────────────────────┐
│  Learned Optimizations (framework)      │
│  - Historical statistics                │
│  - Successful rule applications         │
│  - Performance baselines                │
└─────────────────────────────────────────┘
```

### Configuration Syntax (Proposed)

**Option A: Annotations in SQL comments**
```sql
-- @materialize: daily
-- @partition_by: event_date
-- @backend_hint(spark): { coalesce_partitions: 200 }
SELECT ...
```

**Option B: Separate config file**
```yaml
# daily_stats.config.yaml
model: daily_stats
materialize:
  grain: daily
  partition_by: event_date
backend_hints:
  spark:
    coalesce_partitions: 200
    avoid_cube: true
optimization:
  budget: 2h
  learn_from_history: true
```

**Option C: Both** (annotations for simple, file for complex)

Recommendation: Start with Option A for simplicity, add Option B when needed.

---

## State Management for Computations

### Design: Framework Does NOT Manage State

smelt generates artifacts for target engines to manage state natively. It does NOT implement its own state storage.

| Pattern | Databricks/Spark | Flink | Postgres |
|---------|------------------|-------|----------|
| Incremental | MERGE with partition overwrite | Changelog stream | UPSERT |
| Running totals | Batch recompute or Delta | Managed state + checkpoints | Materialized view |
| Sessions | session_window() | Session windows | Window function rewrite |

### Framework Responsibilities

1. **Validate** - Target engine supports required semantics
2. **Generate** - Correct artifacts for target's state model
3. **Error clearly** - "Model X requires session semantics, postgres_batch doesn't support this"

### Migration Path

If a model is deployed to Spark batch today and moves to Flink streaming tomorrow:
- Logical model unchanged
- Execution config changes target
- Framework generates new artifacts

---

## LSP and Developer Experience

### Quick-Fix Driven Strictness

```
┌────────────────────────────────────────────────────────────┐
│  SELECT a + b FROM t                                       │
│          ~~~                                               │
│  Error: Cannot add VARCHAR and INT                         │
│                                                            │
│  Quick fixes:                                              │
│    • Cast 'a' to INT: CAST(a AS INT) + b                  │
│    • Cast 'b' to VARCHAR: a + CAST(b AS VARCHAR)          │
└────────────────────────────────────────────────────────────┘
```

### LSP Features

- **Autocomplete**: Model names, metric names, column names from upstream
- **Hover**: Show inferred types, metric definitions, upstream schema
- **Go to definition**: Jump to model/metric definition
- **Find references**: Where is this model/metric used?
- **Diagnostics**: Type errors, unknown references, deprecated features
- **Quick fixes**: Add casts, qualify ambiguous columns, add COALESCE

### Error Quality

```
Error: Model 'daily_stats' cannot be deployed to 'duckdb_batch'

Reason: Model requires 'Sessionized' computation (line 15: session_window(...))
        but 'duckdb_batch' does not support native sessions.

Options:
  1. Deploy to 'spark_streaming' (supports sessions natively)
  2. Add '@allow_complex_rewrite' to enable window-function emulation
  3. Refactor model to remove session dependency
```

---

## Comparison with Related Tools

### vs dbt

| Aspect | dbt | smelt |
|--------|-----|-----|
| Model definition | Jinja + SQL templates | Parsed SQL with extensions |
| Ref resolution | Runtime template expansion | Static analysis |
| Type safety | None | Full type system |
| Incrementalization | Manual `is_incremental()` | Automatic semantic analysis |
| Backfill batching | Run N times | Prove safety, run once |
| Cross-engine | No | Yes |
| Optimization | Manual | Rule-based + learning |

### vs Malloy

| Aspect | Malloy | smelt |
|--------|--------|-----|
| Primary user | Analyst exploring data | Engineer building pipelines |
| Execution | Query-time SQL generation | Planned materialization |
| Orchestration | None | Built-in |
| Cross-engine | Single target | Can split across engines |
| Incrementalization | Not in scope | Core feature |
| State management | None | Via target engine |

Malloy is a better query language for analysts. smelt is infrastructure for data engineers.

### vs Substrait

Substrait defines a cross-engine plan representation. smelt could potentially:
- Use Substrait as an IR layer
- Emit Substrait plans for DataFusion backend
- Benefit from Substrait's type system work

### vs Apache Calcite

Calcite is a query optimizer framework. smelt differs:
- Calcite optimizes single queries; smelt optimizes pipeline DAGs
- Calcite focuses on join ordering; smelt focuses on materialization/incrementalization
- smelt delegates low-level optimization to target engines

---

## Incremental Table Builds

### Philosophy: Build with smelt, Not Bake into smelt

**Key Insight**: Incremental table builds should be **implementable using smelt's DSL**, not baked into the framework as special logic. If smelt's framework is sufficiently powerful and flexible, users should be able to express incremental patterns themselves.

This approach:
1. **Tests the framework's expressiveness** - If users can build complex patterns like incrementalization, the framework is powerful
2. **Enables innovation** - Users can create custom materialization strategies beyond what we imagined
3. **Reduces framework complexity** - Less special-casing, more general primitives
4. **Improves transparency** - Users see exactly how incrementalization works, can modify it

### What This Requires from smelt

For users to build incremental materialization themselves, smelt needs these capabilities:

#### 1. Multi-Statement Models

Users need to define models that execute multiple SQL statements:

```sql
-- models/daily_revenue.sql
-- Current: Single SELECT
SELECT order_date, customer_id, SUM(amount) as total
FROM smelt.ref('orders')
GROUP BY 1, 2
```

```sql
-- Needed: Multiple statements with control flow
BEGIN TRANSACTION;

-- Delete affected range
DELETE FROM {{ target_table }}
WHERE order_date >= {{ smelt.batch_start() }}
  AND order_date < {{ smelt.batch_end() }};

-- Insert fresh data
INSERT INTO {{ target_table }}
SELECT order_date, customer_id, SUM(amount) as total
FROM smelt.ref('orders')
WHERE order_date >= {{ smelt.batch_start() }}
  AND order_date < {{ smelt.batch_end() }}
GROUP BY 1, 2;

COMMIT;
```

#### 2. Run Context Access

Users need access to runtime information via built-in functions:

```sql
smelt.is_incremental()           -- Boolean: incremental run or full refresh?
smelt.batch_start()              -- Start of current batch time range
smelt.batch_end()                -- End of current batch time range
smelt.watermark('column_name')   -- Last processed value
smelt.target_table()             -- Name of the target table being written to
smelt.config('key', 'default')   -- Read model configuration
```

#### 3. State Management

Users need to read and update persistent state:

```sql
-- Read last watermark
SELECT * FROM source
WHERE updated_at > smelt.watermark('updated_at')

-- Update watermark (framework tracks this automatically from INSERT)
-- Or explicit: smelt.set_watermark('updated_at', MAX(updated_at))
```

#### 4. Conditional Logic

Users need branching logic (similar to dbt's Jinja):

```sql
{% if smelt.is_incremental() %}
  -- Incremental path: DELETE + INSERT
  DELETE FROM {{ smelt.target_table() }}
  WHERE date >= {{ smelt.batch_start() }};

  INSERT INTO {{ smelt.target_table() }}
  SELECT * FROM source WHERE date >= {{ smelt.batch_start() }};
{% else %}
  -- Full refresh path: CREATE OR REPLACE
  SELECT * FROM source;
{% endif %}
```

**Alternative**: Multi-statement models with smelt functions (no templating):

```sql
-- Framework automatically skips DELETE on full refresh
DELETE FROM smelt.target()
WHERE date >= smelt.batch_start()
  AND smelt.is_incremental();  -- Evaluates to WHERE FALSE on full refresh

INSERT INTO smelt.target()
SELECT * FROM source
WHERE date >= smelt.coalesce(smelt.batch_start(), '1970-01-01');
```

#### 5. Model Composition / Macros

Users should be able to create reusable incremental patterns:

```sql
-- macros/incremental_delete_insert.sql
{% macro incremental_delete_insert(time_column, batch_size='1 day') %}
BEGIN TRANSACTION;

DELETE FROM {{ smelt.target_table() }}
WHERE {{ time_column }} >= {{ smelt.batch_start() }}
  AND {{ time_column }} < {{ smelt.batch_end() }}
  AND {{ smelt.is_incremental() }};

INSERT INTO {{ smelt.target_table() }}
{{ caller() }}  -- User's SELECT goes here
WHERE {{ time_column }} >= {{ smelt.batch_start() }}
  AND {{ time_column }} < {{ smelt.batch_end() }};

COMMIT;
{% endmacro %}
```

```sql
-- models/daily_revenue.sql
{% call incremental_delete_insert('order_date') %}
  SELECT order_date, customer_id, SUM(amount) as total
  FROM smelt.ref('orders')
  GROUP BY 1, 2
{% endcall %}
```

### What smelt Provides (Framework Responsibilities)

While users write the incremental logic, smelt provides:

1. **Execution orchestration** - Run models in dependency order, handle batching
2. **State persistence** - Store watermarks, run metadata
3. **Configuration management** - Read batch_size, lookback from config
4. **Batch boundary calculation** - Determine what batches need processing
5. **Transaction management** - Ensure atomic commits
6. **Error recovery** - Resume from failed batches
7. **Semantic analysis** - Validate safety (warn if unsafe patterns detected)

### Example: User-Implemented Incremental Pattern

Here's how a user would implement delete+insert incrementalization:

```sql
-- models/daily_revenue.sql
-- @incremental.time_column: order_date
-- @incremental.batch_size: 1 day
-- @incremental.lookback: 3 days

{% if smelt.is_incremental() %}
  -- Incremental: delete affected range, then insert
  DELETE FROM {{ smelt.target_table() }}
  WHERE order_date >= {{ smelt.batch_start() }}
    AND order_date < {{ smelt.batch_end() }};
{% endif %}

-- Both full refresh and incremental execute this INSERT
INSERT INTO {{ smelt.target_table() }}
SELECT
  order_date,
  customer_id,
  SUM(amount) as total
FROM smelt.ref('orders')
WHERE
  {% if smelt.is_incremental() %}
    order_date >= {{ smelt.batch_start() }}
    AND order_date < {{ smelt.batch_end() }}
  {% else %}
    TRUE  -- Full refresh: process all data
  {% endif %}
GROUP BY 1, 2;
```

### Benefits of This Approach

**Transparency**: Users see exactly what SQL is executed, not framework magic

**Flexibility**: Users can customize incremental strategy per model:
- MERGE instead of DELETE+INSERT
- Custom conflict resolution
- Multi-step transformations
- Complex state management

**Innovation**: Users can create new patterns:
- Incremental with deduplication
- Incremental with slowly changing dimensions
- Custom backfill strategies
- Conditional full refreshes

**Learning**: Framework can suggest optimizations by analyzing user patterns

### Framework-Provided Helpers (Optional)

smelt could provide optional built-in macros for common patterns:

```sql
-- Using built-in helper
{% use smelt.patterns.incremental_delete_insert %}

SELECT order_date, customer_id, SUM(amount) as total
FROM smelt.ref('orders')
GROUP BY 1, 2;
```

This is convenience, not magic - users can view the macro source and customize.

### Semantic Analysis and Safety

smelt can still analyze user-written incremental models:

```
$ smelt run --incremental

Analyzing daily_revenue...
  ⚠️  Warning: Window function ROW_NUMBER() OVER (PARTITION BY user_id ...)
      This may be unsafe for incremental builds.
      Each batch only sees a subset of user_id rows.

      Options:
        1. Add user_id to batch partitioning (makes it safe)
        2. Add lookback to window context
        3. Force full refresh for this model

  ✓  Incremental logic looks safe for delete+insert strategy
  ✓  Time column 'order_date' found in GROUP BY (partition-independent)
```

### Missing Features from Current Design

To enable user-implemented incrementalization, smelt needs:

| Feature | Current Status | Needed For |
|---------|----------------|------------|
| **Multi-statement models** | ❌ Only single SELECT | DELETE + INSERT transactions |
| **Template/macro system** | ❌ No templating | Conditional logic, reusable patterns |
| **Runtime context functions** | ❌ No built-ins | Access to batch boundaries, watermarks |
| **Target table reference** | ❌ Can't reference output | DELETE from target, MERGE into target |
| **State API** | ❌ No state management | Read/write watermarks, custom state |
| **Model composition** | ❌ No macro/include | Share incremental patterns across models |
| **Transaction control** | ❌ Framework-managed only | User-controlled BEGIN/COMMIT |

### Configuration

Users configure their models with metadata that smelt uses for orchestration:

```yaml
# smelt.yml
models:
  daily_revenue:
    incremental:
      time_column: order_date       # Which column defines batches
      batch_size: 1 day              # Logical grain
      lookback: 3 days               # Reprocess recent batches (late arrivals)
```

Or as annotations:
```sql
-- models/daily_revenue.sql
-- @incremental.time_column: order_date
-- @incremental.batch_size: 1 day
-- @incremental.lookback: 3 days

{% if smelt.is_incremental() %}
  DELETE FROM {{ smelt.target_table() }}
  WHERE order_date >= {{ smelt.batch_start() }};
{% endif %}

INSERT INTO {{ smelt.target_table() }}
SELECT order_date, customer_id, SUM(amount)
FROM smelt.ref('orders')
WHERE order_date >= {{ smelt.batch_start() }}
GROUP BY 1, 2;
```

### CLI Interface

```bash
# Full refresh
smelt run

# Incremental run (processes new batches based on watermarks)
smelt run --incremental

# Run specific date range
smelt run --incremental --start-date 2024-01-15 --end-date 2024-01-18

# Force full refresh for specific model
smelt run --full-refresh --select daily_revenue

# Dry run (show what would be processed)
smelt run --incremental --dry-run

# Show watermark state
smelt state show
smelt state show daily_revenue

# Reset watermark (force reprocessing)
smelt state reset daily_revenue --from 2024-01-01
```

### Comparison with dbt Microbatch

**dbt approach** (framework generates incremental logic):
```sql
{{ config(
    materialized='incremental',
    incremental_strategy='microbatch',
    event_time='order_date',
    batch_size='day'
) }}

SELECT order_date, customer_id, SUM(amount)
FROM {{ source('raw', 'orders') }}
{% if is_incremental() %}
WHERE order_date >= '{{ var("start_date") }}'
  AND order_date < '{{ var("end_date") }}'
{% endif %}
GROUP BY 1, 2
```

**smelt approach** (user writes incremental logic, framework orchestrates):
```sql
-- @incremental.time_column: order_date
-- @incremental.batch_size: 1 day

{% if smelt.is_incremental() %}
  DELETE FROM {{ smelt.target_table() }}
  WHERE order_date >= {{ smelt.batch_start() }};
{% endif %}

INSERT INTO {{ smelt.target_table() }}
SELECT order_date, customer_id, SUM(amount)
FROM smelt.ref('orders')
WHERE order_date >= {{ smelt.batch_start() }}
GROUP BY 1, 2;
```

**Key differences:**

| Aspect | dbt | smelt |
|--------|-----|-------|
| **Who writes logic** | Framework magic | User explicit |
| **Transparency** | Hidden DELETE | Visible in model |
| **Flexibility** | Fixed strategies | User-customizable |
| **Semantics** | Parse SQL | Parse + understand |
| **Validation** | Runtime errors | Static analysis warnings |
| **Customization** | Limited config | Full SQL control |
| **Learning curve** | Lower (less to write) | Higher (more powerful) |

smelt prioritizes **transparency and power** over simplicity. Users see exactly what runs, can customize every detail, and the framework provides analysis and orchestration.

### Framework Optimizations (Optional)

Even with user-written incremental logic, smelt can provide intelligent orchestration:

#### Dynamic Batch Grouping

**The problem**: A 90-day backfill with `batch_size: 1 day` naively means 90 query executions.

**smelt's optimization**: The `batch_size` defines logical grain, but smelt can group multiple batches into one execution when safe:

```
$ smelt run --incremental --start-date 2024-01-01 --end-date 2024-03-31

Analyzing models for batch optimization...

daily_revenue:
  Logical batches: 90 days
  Model is partition-independent ✓
  Can process in 13 weekly executions instead of 90 daily
  Estimated speedup: ~6x

user_sessions:
  Logical batches: 90 days
  Model has cross-partition dependencies ✗
  Must process per-day (90 executions)
  Reason: LAG() window function over user_id crosses batch boundaries

Proceed with optimized plan? [Y/n]
```

Users can override:
```bash
# Force specific grouping
smelt run --incremental --batch-group "1 week"

# Force one execution for entire range
smelt run --incremental --batch-group all

# Debug mode: one execution per logical batch
smelt run --incremental --batch-group "1 day"
```

#### Cross-Model Optimization

When models share dependencies, smelt can:
1. Compute shared batches together
2. Parallelize independent batches
3. Skip batches where all downstream models are up-to-date

These are framework optimizations - users write standard incremental logic, smelt optimizes execution.

---

## Schema Evolution

When model definitions change, smelt can efficiently update existing materialized tables instead of requiring full rebuilds.

### The Problem

In dbt, any schema change requires a full refresh:
```sql
-- Before: SELECT a, b FROM source
-- After:  SELECT a, b, c FROM source

-- dbt approach: DROP TABLE and rebuild from scratch
-- Even if the table has 10 billion rows and 'c' is cheap to compute
```

### smelt's Approach

Because smelt tracks schemas and understands SQL semantics, it can generate efficient migrations:

```sql
-- Adding a column
ALTER TABLE daily_revenue ADD COLUMN new_metric DECIMAL;
UPDATE daily_revenue SET new_metric = (
  SELECT SUM(amount) FROM orders WHERE orders.date = daily_revenue.date
);

-- Or for additive columns with defaults
ALTER TABLE daily_revenue ADD COLUMN region VARCHAR DEFAULT 'unknown';
```

### Change Detection

smelt compares the current model definition against the last-deployed schema:

```
$ smelt run

Schema changes detected:

daily_revenue:
  + new_metric DECIMAL     (added column)
  ~ amount DECIMAL(10,2)   (was: DECIMAL - precision change)
  - old_column             (removed column)

Migration strategy:
  • new_metric: ALTER TABLE ADD COLUMN + backfill query
  • amount: Safe widening, no action needed
  • old_column: Will be dropped (data loss)

Proceed? [Y/n]
```

### Evolution Strategies

| Change Type | Strategy | Data Preserved? |
|-------------|----------|-----------------|
| Add column (computable) | ALTER + UPDATE | ✅ Yes |
| Add column (with default) | ALTER + DEFAULT | ✅ Yes |
| Add column (needs source) | Full refresh | ✅ Yes |
| Remove column | ALTER DROP | ⚠️ Column lost |
| Widen type (INT→BIGINT) | No action | ✅ Yes |
| Narrow type (BIGINT→INT) | Validate + ALTER | ⚠️ May fail |
| Change type (incompatible) | Full refresh | ✅ Yes |
| Rename column | ALTER RENAME | ✅ Yes |

### Efficient Backfill for New Columns

When adding a column, smelt analyzes whether it can be computed from existing data:

**Case 1: Column derived from existing columns**
```sql
-- Model adds: total_with_tax AS amount * 1.1
-- smelt generates:
ALTER TABLE orders ADD COLUMN total_with_tax DECIMAL;
UPDATE orders SET total_with_tax = amount * 1.1;
```

**Case 2: Column from upstream model (already materialized)**
```sql
-- Model adds: customer_name from smelt.ref('customers')
-- smelt generates:
ALTER TABLE orders ADD COLUMN customer_name VARCHAR;
UPDATE orders o SET customer_name = (
  SELECT c.name FROM customers c WHERE c.id = o.customer_id
);
```

**Case 3: Column requires source data**
```sql
-- Model adds: new_field from smelt.ref('raw_events')
-- If raw_events is a source (not materialized), full refresh needed
-- smelt warns and offers options:
--   1. Full refresh (safe, slow)
--   2. Set to NULL/default for existing rows (fast, incomplete)
--   3. Incremental backfill over time windows
```

### Cross-Model Evolution

When a model's schema changes, smelt analyzes downstream impact:

```
$ smelt run

Schema change in 'orders':
  + shipping_cost DECIMAL

Downstream impact analysis:

  daily_revenue (depends on orders):
    • No impact - doesn't select shipping_cost

  order_summary (depends on orders):
    • Uses SELECT * - will automatically include new column
    • Downstream schema will change
    • Cascade: customer_report also uses SELECT *

Options:
  1. Update all downstream models (recommended)
  2. Update only direct dependents
  3. Update orders only (downstream will fail on next run)
```

### Configuration

Control evolution behavior per-model or globally:

```yaml
# smelt.yml
schema_evolution:
  strategy: prompt           # prompt, auto, strict
  allow_column_removal: true
  allow_type_narrowing: false

models:
  critical_table:
    schema_evolution:
      strategy: strict       # Never auto-migrate, always prompt
      allow_column_removal: false
```

Or via annotations:
```sql
-- @schema_evolution: strict
-- @schema_evolution.allow_column_removal: false

SELECT ...
```

### CLI Commands

```bash
# Show pending schema changes without applying
smelt diff

# Apply schema migrations
smelt run --migrate

# Force full refresh even when migration is possible
smelt run --full-refresh

# Generate migration SQL without executing
smelt migrate --dry-run --output migrations/

# Validate that schema changes are safe
smelt validate
```

### State Tracking

smelt tracks deployed schemas:

```yaml
# .smelt/state/daily_revenue.state.yaml
model: daily_revenue
schema:
  version: 3
  deployed_at: 2024-01-18T06:00:00Z
  columns:
    - name: order_date
      type: DATE
      nullable: false
    - name: customer_id
      type: INTEGER
      nullable: false
    - name: total
      type: DECIMAL(10,2)
      nullable: true
  history:
    - version: 2
      deployed_at: 2024-01-10T06:00:00Z
      changes: ["added column: total"]
    - version: 1
      deployed_at: 2024-01-01T06:00:00Z
      changes: ["initial deployment"]
```

### Integration with Incremental

Schema evolution works with incremental builds:

```
Scenario: Add new column to incremental model

1. smelt detects schema change (new column added)
2. For existing rows: ALTER TABLE + backfill UPDATE
3. For new rows: Normal incremental INSERT includes new column
4. Result: Complete data, minimal recomputation
```

```sql
-- Combined migration + incremental
BEGIN TRANSACTION;

-- Schema migration
ALTER TABLE daily_revenue ADD COLUMN new_metric DECIMAL;
UPDATE daily_revenue SET new_metric = compute_metric(...)
WHERE TRUE;  -- All existing rows

-- Incremental update (new data)
DELETE FROM daily_revenue WHERE order_date >= '2024-01-18';
INSERT INTO daily_revenue
SELECT order_date, customer_id, total, compute_metric(...) as new_metric
FROM orders
WHERE order_date >= '2024-01-18';

COMMIT;
```

---

## Implementation Phases

### Phase 1: Core Parser and Single Backend
- SQL parser with `smelt.ref()` extension
- Basic type checking
- DuckDB backend
- Simple model dependencies
- No incrementalization

### Phase 2: Type System and LSP
- Full type inference
- NULL tracking
- LSP with diagnostics and quick-fixes
- Multiple models, dependency resolution

### Phase 3: Multi-Backend and Rewrites
- Add Postgres, Spark backends
- Rewrite rule framework
- Backend capability declarations
- Basic rule library (QUALIFY, PIVOT, etc.)

### Phase 4: Metrics DSL
- Metric definition syntax
- `smelt.metric()` resolution
- Temporal semantics metadata
- Metric composition

### Phase 5: Incrementalization
- Computation requirement analysis
- Batch safety proofs
- Incremental rewrite rules
- State requirement validation

### Phase 6: Learning and Optimization
- Run history capture
- Statistics persistence
- Optimization budget system
- Recommendation engine

---

## Open Questions

1. **Metrics DSL syntax**: YAML? Custom DSL? SQL-like?

2. **Config location**: Annotations, separate files, or both?

3. **Rule language**: Pure Rust? Python bindings? Declarative DSL?

4. **Substrait integration**: Use as IR layer? Just for DataFusion?

5. **Testing strategy**: How to verify rewrite correctness across engines?

6. **Lineage/Catalog integration**: How to expose to external catalogs?

7. **Secrets/connections**: How to configure database connections?

---

## Appendix: SQL Extension Grammar (Sketch)

```ebnf
smelt_ref ::= 'smelt.ref' '(' string_literal (',' ref_param)* ')'
ref_param ::= identifier '=>' expr

smelt_metric ::= 'smelt.metric' '(' string_literal (',' metric_param)* ')'
metric_param ::= identifier '=>' expr

-- smelt functions can appear in:
--   FROM clause (smelt.ref)
--   SELECT expressions (smelt.metric)
--   WHERE/HAVING (smelt.metric for filtering)
```

---

## Appendix: Example End-to-End

**Metric definition:**
```yaml
metric revenue:
  measure: sum(amount)
  entity: order
  decomposable: true
```

**Model definition:**
```sql
-- models/daily_revenue.sql
-- @materialize: daily
-- @partition_by: order_date

SELECT
  order_date,
  customer_id,
  smelt.metric('revenue') as daily_revenue
FROM smelt.ref('orders', filter => order_date >= current_date - 90)
GROUP BY 1, 2
```

**Execution config:**
```yaml
# execution/daily_revenue.yaml
model: daily_revenue
targets:
  - name: dev
    backend: duckdb
  - name: prod
    backend: spark
    hints:
      coalesce_partitions: 100
```

**Deploy:**
```bash
smelt deploy --model daily_revenue --target prod
# Framework:
#   1. Parses model, resolves metric
#   2. Checks Spark supports all requirements
#   3. Applies rewrites if needed
#   4. Generates Spark SQL
#   5. Creates incremental merge logic
#   6. Outputs to configured location
```
