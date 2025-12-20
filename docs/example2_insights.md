# Example 2: Split Large GROUP BY - Insights

## Problem Statement

A single query with multi-dimensional GROUP BY creates massive shuffle overhead in distributed engines (Spark/Databricks):

```sql
SELECT user_id, country, device_type, event_date,
       COUNT(*), SUM(revenue), AVG(revenue)
FROM events
GROUP BY user_id, country, device_type, event_date
```

**Problem**: With high-cardinality dimensions (1000 users × 8 countries × 3 devices × 30 dates), this creates ~7200 intermediate groups to shuffle, even though the data is sparse.

**Optimized approach**: Split into independent single-dimension queries and UNION ALL.

## Results

**Naive approach:**
- 7,200 intermediate shuffle rows
- Single large shuffle blocks entire job
- High memory pressure on shuffle phase

**Optimized approach:**
- 1,041 total shuffle rows (across 4 queries)
- 4 independent shuffles (can run in parallel)
- 85% reduction in shuffle data volume

## Key Differences from Example 1

### Example 1: Correctness-Preserving
- **Transformation**: Extract common computation, reuse it
- **Result**: Identical to naive version (bitwise equivalent)
- **Safety**: Can apply automatically
- **Pattern**: Shared intermediate (common subexpression elimination)

### Example 2: Lossy Transformation
- **Transformation**: Split multi-dimensional GROUP BY into single dimensions
- **Result**: DIFFERENT from naive version (schema and data changed!)
- **Safety**: CANNOT apply automatically - requires user consent
- **Pattern**: Computational optimization at the cost of information

### Critical Insight

Example 2 reveals that **not all optimizations preserve results**. Some optimizations are **semantic transformations** that change what the query computes.

```sql
-- Naive: Returns all dimension combinations
SELECT user_id, country, COUNT(*) FROM events
GROUP BY user_id, country

-- Result: [(user=1, country='US', count=5), (user=1, country='UK', count=3), ...]
```

```sql
-- Optimized: Returns ONLY single dimensions
SELECT user_id, NULL, COUNT(*) FROM events GROUP BY user_id
UNION ALL
SELECT NULL, country, COUNT(*) FROM events GROUP BY country

-- Result: [(user=1, NULL, count=8), (NULL, country='US', count=20), ...]
```

**These are not equivalent!** The optimized version loses cross-dimensional information.

## When Is This Optimization Valid?

### Valid Use Case: Separate Dimensional Aggregates

User writes THREE separate models:

```sql
-- models/users_aggregates.sql
SELECT user_id, COUNT(*), SUM(revenue) FROM events GROUP BY user_id

-- models/country_aggregates.sql
SELECT country, COUNT(*), SUM(revenue) FROM events GROUP BY country

-- models/device_aggregates.sql
SELECT device_type, COUNT(*), SUM(revenue) FROM events GROUP BY device_type
```

Optimizer detects:
- All three models scan the same source table (`events`)
- They use the same filters (if any)
- They compute decomposable aggregates (COUNT, SUM)
- They could be computed in a single pass with UNION ALL

**This is safe!** User explicitly wanted separate dimensional aggregates.

### Invalid Use Case: Cross-Dimensional Analysis

User writes:

```sql
-- models/user_country_matrix.sql
SELECT user_id, country, COUNT(*) FROM events GROUP BY user_id, country
```

Optimizer **CANNOT** split this into:
```sql
SELECT user_id, COUNT(*) FROM events GROUP BY user_id
UNION ALL
SELECT country, COUNT(*) FROM events GROUP BY country
```

Because the user wants the cross-dimensional information (which users are in which countries).

## Pattern Detection Requirements

The optimizer needs to:

1. **Detect separate dimensional models**
   - Multiple models querying the same source
   - Each model groups by a different dimension
   - Same or compatible filters

2. **Verify decomposability**
   - Aggregates must be decomposable (SUM, COUNT, MIN, MAX, AVG)
   - NOT: MEDIAN, PERCENTILE, STRING_AGG, ARRAY_AGG

3. **Estimate cost savings**
   - Compare: single multi-dimensional query vs. multiple single-dimension queries
   - Factor in: cardinality, data volume, shuffle cost
   - Only apply if significant savings (e.g., >50% reduction)

4. **Check for sparse combinations**
   - If data is dense (most combinations exist), splitting may not help
   - Sparse data benefits most (most combinations don't appear)

## Rewrite Strategy

### Pattern Match
```
Find:
  - Models M1, M2, ..., Mn
  - Same source table S
  - M1 groups by dimension D1
  - M2 groups by dimension D2
  - ...
  - Same filters F (or compatible)
  - Decomposable aggregates A
```

### Rewrite
```
Create single unified query:
  SELECT D1, NULL as D2, ..., NULL as Dn, 'by_D1' as _dimension, A
  FROM S WHERE F GROUP BY D1

  UNION ALL

  SELECT NULL, D2, ..., NULL, 'by_D2', A
  FROM S WHERE F GROUP BY D2

  ...

Split results to respective models based on _dimension tag
```

### Benefits
- Single table scan instead of N scans
- Smaller individual shuffles
- Parallelizable (each dimension independent)
- Reduced total shuffle volume

## Correctness Considerations

### This is NOT a transparent optimization!

**Example 1** (shared session aggregation):
- User writes: `SELECT ... FROM sessions`
- Optimizer: Creates shared `session_summary` table
- Result: **Exact same answer**, just faster

**Example 2** (split GROUP BY):
- User writes: THREE separate models for different dimensions
- Optimizer: Combines into one UNION ALL query
- Result: **Same answer** (user wanted separate dimensions anyway)
- BUT: If user had written ONE model with multiple dimensions, **cannot split**

### Key Distinction

- **Example 1**: Optimization at physical level (how to execute)
- **Example 2**: Optimization at logical level (what to compute)

The optimizer can only apply Example 2 when it detects that the user's **logical intent** matches the optimization.

## API Design Implications

### Option A: Explicit Opt-In (Safer)

User explicitly marks models as candidates for dimensional splitting:

```rust
model! {
    name: "user_aggregates",
    sql: "SELECT user_id, COUNT(*), SUM(revenue) FROM events GROUP BY user_id",
    hints: vec![OptimizeHint::DimensionalSplit {
        combine_with: vec!["country_aggregates", "device_aggregates"],
        dimension: "user_id"
    }]
}
```

**Pros**: Explicit, safe, no surprises
**Cons**: Verbose, requires user to identify optimization opportunities

### Option B: Automatic Detection with Safeguards

Optimizer automatically detects the pattern but only applies if:
1. Models are in a specific directory (e.g., `models/dimensions/`)
2. Models follow naming convention (e.g., `dim_*.sql`)
3. User has not disabled the optimization
4. Cost savings exceed threshold (e.g., 50%)

**Pros**: Less verbose, good defaults
**Cons**: Implicit behavior, potential confusion

### Option C: Hybrid (Recommended)

Optimizer detects opportunities and **suggests** them to the user via LSP:

```
💡 Optimization available: Combine dimensional aggregates
   Models: user_aggregates, country_aggregates, device_aggregates
   Expected savings: 85% shuffle reduction, ~$15/day cost savings

   [Apply] [Ignore] [Never for this project]
```

User clicks "Apply" → Optimizer generates optimized plan and updates model annotations

**Pros**: Best of both worlds - automatic detection + user control
**Cons**: Requires LSP integration

## Differences Between Examples 1 & 2

| Aspect | Example 1 (Shared Aggregation) | Example 2 (Split GROUP BY) |
|--------|-------------------------------|---------------------------|
| **Correctness** | Preserves exact results | Changes result structure |
| **Automation** | Can apply automatically | Requires user consent |
| **Pattern** | Common subexpression | Separate dimensional models |
| **Optimization Level** | Physical (execution) | Logical (computation) |
| **Safety** | Always safe | Conditionally safe |
| **Detection** | Structural equivalence | Semantic intent analysis |
| **Benefit** | Avoid redundant work | Reduce shuffle overhead |

## Unified Optimization Rule API Requirements

Based on both examples, the API must support:

1. **Pattern Matching**
   - Structural: Same SQL text/AST (Example 1)
   - Semantic: Same intent/computation (Example 2)

2. **Applicability Conditions**
   - Cost-based: Apply if savings > threshold
   - Safety-based: Apply only if correctness preserved
   - User-consent: Apply only if user approves (for lossy optimizations)

3. **Rewrite Operations**
   - Extract common computation (Example 1)
   - Combine separate queries (Example 2)
   - Insert materialization points
   - Modify query structure

4. **Cost Estimation**
   - Data volume
   - Shuffle cost
   - Backend capabilities
   - Parallelism opportunities

5. **User Interaction**
   - Automatic (safe optimizations)
   - Suggested (lossy optimizations)
   - Opt-in (experimental optimizations)

## Next Steps

1. **Design Rule API** that supports both patterns
2. **Implement cost model** for shuffle estimation
3. **Build LSP integration** for suggesting optimizations
4. **Create rule library** with common patterns
5. **Add tests** for correctness preservation vs. transformation

The key insight: **Not all optimizations are created equal.** Some are transparent (Example 1), others require user understanding and consent (Example 2). The API must distinguish between these categories.
