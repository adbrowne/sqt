# Optimization Rule API Design

## Goals

Design an API that allows data engineers to write optimization rules that:
1. **Detect patterns** in the logical plan (common subexpressions, inefficient operations)
2. **Transform plans** into more efficient physical plans
3. **Preserve correctness** or explicitly mark when transformations are lossy
4. **Work with incremental compilation** (Salsa-compatible)
5. **Remain valid** as models change over time

## Learnings from Examples

### Example 1: Common Intermediate Aggregation
- **Pattern**: Multiple models compute same intermediate result
- **Transformation**: Create shared materialization, update consumers
- **Correctness**: Preserves exact results (transparent optimization)
- **Applicability**: Automatic when pattern detected and cost-beneficial

### Example 2: Split Large GROUP BY
- **Pattern**: Multiple single-dimension models from same source
- **Transformation**: Combine into UNION ALL query
- **Correctness**: Lossy (changes schema), only valid for specific use cases
- **Applicability**: Requires user consent or explicit opt-in

## Key Insight: Two Types of Optimizations

### Type 1: Transparent (Physical)
- Changes HOW query executes, not WHAT it computes
- Always safe to apply if cost-beneficial
- Examples: shared materialization, predicate pushdown, index selection
- **API requirement**: Auto-apply with cost threshold

### Type 2: Semantic (Logical)
- Changes WHAT query computes
- Only safe when user intent matches transformation
- Examples: dimensional splitting, approximation algorithms
- **API requirement**: User consent or explicit opt-in

## Proposed API

### Core Trait: OptimizationRule

```rust
use salsa::Database;

/// An optimization rule that can be applied to a pipeline
pub trait OptimizationRule: Send + Sync {
    /// Unique name for this rule
    fn name(&self) -> &str;

    /// Type of optimization (transparent vs semantic)
    fn optimization_type(&self) -> OptimizationType;

    /// Detect if this rule is applicable to the given models
    /// Returns None if not applicable, Some(match_info) if applicable
    fn matches(&self, ctx: &RuleContext) -> Option<RuleMatch>;

    /// Estimate the cost benefit of applying this rule
    /// Returns (current_cost, optimized_cost, confidence)
    fn estimate_benefit(&self, ctx: &RuleContext, match_info: &RuleMatch) -> CostEstimate;

    /// Apply the transformation
    /// Returns the rewritten plan or an error
    fn apply(&self, ctx: &mut RuleContext, match_info: &RuleMatch) -> Result<Rewrite, Error>;

    /// Check if this rule is still valid after models changed
    /// Used by Salsa to invalidate optimizations when dependencies change
    fn is_valid(&self, ctx: &RuleContext, rewrite: &Rewrite) -> bool;
}

pub enum OptimizationType {
    /// Transparent: preserves exact results, can auto-apply
    Transparent,

    /// Semantic: changes results, requires user consent
    Semantic { requires_approval: bool },

    /// Experimental: might not preserve correctness, opt-in only
    Experimental,
}

pub struct CostEstimate {
    pub current_cost: Cost,
    pub optimized_cost: Cost,
    pub confidence: f64,  // 0.0 to 1.0
    pub explanation: String,
}

pub struct Cost {
    pub shuffle_bytes: u64,
    pub compute_time_ms: u64,
    pub memory_mb: u64,
    pub estimated_dollars: f64,
}
```

### Rule Context (Salsa Integration)

```rust
use salsa::Database;

/// Context provided to rules, powered by Salsa for incremental queries
pub struct RuleContext<'db> {
    db: &'db dyn OptimizerDatabase,
}

impl<'db> RuleContext<'db> {
    /// Get all models in the pipeline
    pub fn all_models(&self) -> Arc<HashMap<ModelId, Model>> {
        self.db.all_models()  // Salsa query
    }

    /// Get the logical plan for a model
    pub fn logical_plan(&self, model: ModelId) -> Arc<LogicalPlan> {
        self.db.logical_plan(model)  // Salsa query
    }

    /// Get dependency graph
    pub fn dependency_graph(&self) -> Arc<DepGraph> {
        self.db.dependency_graph()  // Salsa query
    }

    /// Find models that match a pattern
    pub fn find_models<F>(&self, predicate: F) -> Vec<ModelId>
    where
        F: Fn(&Model) -> bool
    {
        self.all_models()
            .iter()
            .filter(|(_, m)| predicate(m))
            .map(|(id, _)| *id)
            .collect()
    }

    /// Compute shared columns across models
    pub fn union_required_columns(&self, models: &[ModelId]) -> Vec<Column> {
        // Implementation: analyze all models, compute union of columns
        todo!()
    }

    /// Check if two logical plans are structurally equivalent
    pub fn plans_equivalent(&self, plan1: &LogicalPlan, plan2: &LogicalPlan) -> bool {
        // Deep structural comparison
        todo!()
    }

    /// Estimate cardinality of a plan
    pub fn estimate_cardinality(&self, plan: &LogicalPlan) -> u64 {
        self.db.estimate_cardinality(plan)  // Salsa query
    }
}
```

### Rule Match and Rewrite

```rust
/// Information about a pattern match
pub struct RuleMatch {
    pub matched_models: Vec<ModelId>,
    pub pattern_type: String,
    pub metadata: HashMap<String, Value>,
}

/// Description of a rewrite transformation
pub struct Rewrite {
    pub rule_name: String,
    pub original_models: Vec<ModelId>,
    pub transformations: Vec<Transformation>,
}

pub enum Transformation {
    /// Create a new materialized table
    CreateMaterialization {
        name: String,
        plan: LogicalPlan,
        materialization_type: MaterializationType,
    },

    /// Update a model to reference a different source
    UpdateModelSource {
        model: ModelId,
        new_source: String,
    },

    /// Combine multiple models into one physical query
    CombineModels {
        models: Vec<ModelId>,
        combined_plan: LogicalPlan,
        split_results: HashMap<ModelId, ResultSelector>,
    },

    /// Add a hint to the physical plan
    AddHint {
        model: ModelId,
        hint: PhysicalHint,
    },
}

pub enum MaterializationType {
    View,
    Table,
    TempTable,
    IncrementalTable,
}
```

## Example Implementations

### Example 1: Common Intermediate Aggregation Rule

```rust
pub struct SharedAggregationRule;

impl OptimizationRule for SharedAggregationRule {
    fn name(&self) -> &str {
        "shared_intermediate_aggregation"
    }

    fn optimization_type(&self) -> OptimizationType {
        OptimizationType::Transparent
    }

    fn matches(&self, ctx: &RuleContext) -> Option<RuleMatch> {
        // Find models with common CTE patterns
        let models = ctx.all_models();

        // Group models by their first CTE (simplified)
        let mut groups: HashMap<String, Vec<ModelId>> = HashMap::new();

        for (id, model) in models.iter() {
            let plan = ctx.logical_plan(*id);

            // Extract common CTE pattern (e.g., session computation)
            if let Some(cte_pattern) = extract_common_cte(&plan) {
                groups.entry(cte_pattern).or_default().push(*id);
            }
        }

        // Find groups with 2+ models
        for (pattern, model_ids) in groups {
            if model_ids.len() >= 2 {
                return Some(RuleMatch {
                    matched_models: model_ids,
                    pattern_type: "common_cte".to_string(),
                    metadata: hashmap! {
                        "cte_pattern" => json!(pattern),
                    },
                });
            }
        }

        None
    }

    fn estimate_benefit(&self, ctx: &RuleContext, match_info: &RuleMatch) -> CostEstimate {
        let num_consumers = match_info.matched_models.len();

        // Rough estimate: avoid (N-1) redundant computations
        let plan = ctx.logical_plan(match_info.matched_models[0]);
        let single_cost = estimate_plan_cost(&plan);

        CostEstimate {
            current_cost: Cost {
                compute_time_ms: single_cost.compute_time_ms * num_consumers as u64,
                ..single_cost
            },
            optimized_cost: single_cost,  // Compute once
            confidence: 0.8,
            explanation: format!(
                "Compute shared intermediate once instead of {} times",
                num_consumers
            ),
        }
    }

    fn apply(&self, ctx: &mut RuleContext, match_info: &RuleMatch) -> Result<Rewrite, Error> {
        let models = &match_info.matched_models;

        // Extract the common CTE
        let common_plan = extract_common_subplan(ctx, models)?;

        // Compute union of required columns
        let required_columns = ctx.union_required_columns(models);

        // Create materialization
        let mat_name = format!("_shared_{}", self.name());
        let mat_plan = add_required_columns(common_plan, required_columns);

        let mut transformations = vec![
            Transformation::CreateMaterialization {
                name: mat_name.clone(),
                plan: mat_plan,
                materialization_type: MaterializationType::TempTable,
            }
        ];

        // Update each consumer to use the materialization
        for model in models {
            transformations.push(Transformation::UpdateModelSource {
                model: *model,
                new_source: mat_name.clone(),
            });
        }

        Ok(Rewrite {
            rule_name: self.name().to_string(),
            original_models: models.clone(),
            transformations,
        })
    }

    fn is_valid(&self, ctx: &RuleContext, rewrite: &Rewrite) -> bool {
        // Check if all original models still exist and have compatible patterns
        for model_id in &rewrite.original_models {
            if !ctx.all_models().contains_key(model_id) {
                return false;  // Model was deleted
            }

            // Could also check if model was modified in incompatible way
        }
        true
    }
}
```

### Example 2: Dimensional Split Rule

```rust
pub struct DimensionalSplitRule;

impl OptimizationRule for DimensionalSplitRule {
    fn name(&self) -> &str {
        "split_dimensional_aggregates"
    }

    fn optimization_type(&self) -> OptimizationType {
        OptimizationType::Semantic {
            requires_approval: true,  // Lossy optimization!
        }
    }

    fn matches(&self, ctx: &RuleContext) -> Option<RuleMatch> {
        // Find models that:
        // 1. Query the same source table
        // 2. Each groups by a single dimension
        // 3. Use compatible filters
        // 4. Use decomposable aggregates

        let models = ctx.all_models();

        // Group by source table
        let mut by_source: HashMap<String, Vec<ModelId>> = HashMap::new();

        for (id, model) in models.iter() {
            let plan = ctx.logical_plan(*id);

            if let Some(source) = extract_source_table(&plan) {
                // Check: single GROUP BY dimension + decomposable aggregates
                if is_single_dimension_aggregate(&plan) {
                    by_source.entry(source).or_default().push(*id);
                }
            }
        }

        // Find groups with 2+ models
        for (source, model_ids) in by_source {
            if model_ids.len() >= 2 {
                return Some(RuleMatch {
                    matched_models: model_ids,
                    pattern_type: "dimensional_split".to_string(),
                    metadata: hashmap! {
                        "source_table" => json!(source),
                    },
                });
            }
        }

        None
    }

    fn estimate_benefit(&self, ctx: &RuleContext, match_info: &RuleMatch) -> CostEstimate {
        let models = &match_info.matched_models;

        // Current: N separate table scans
        let current_cost = models.iter()
            .map(|id| estimate_plan_cost(&ctx.logical_plan(*id)))
            .sum();

        // Optimized: 1 table scan + UNION ALL (cheaper shuffle)
        let combined_shuffle_cost = estimate_union_all_cost(ctx, models);

        CostEstimate {
            current_cost,
            optimized_cost: combined_shuffle_cost,
            confidence: 0.7,
            explanation: format!(
                "Combine {} dimensional scans into single query with UNION ALL",
                models.len()
            ),
        }
    }

    fn apply(&self, ctx: &mut RuleContext, match_info: &RuleMatch) -> Result<Rewrite, Error> {
        let models = &match_info.matched_models;

        // Build UNION ALL query
        let combined_plan = build_union_all_plan(ctx, models)?;

        Ok(Rewrite {
            rule_name: self.name().to_string(),
            original_models: models.clone(),
            transformations: vec![
                Transformation::CombineModels {
                    models: models.clone(),
                    combined_plan,
                    split_results: build_result_selectors(models),
                }
            ],
        })
    }

    fn is_valid(&self, ctx: &RuleContext, rewrite: &Rewrite) -> bool {
        // Check if models still have compatible patterns
        // If one model adds a cross-dimensional GROUP BY, invalidate
        for model_id in &rewrite.original_models {
            let plan = ctx.logical_plan(*model_id);
            if !is_single_dimension_aggregate(&plan) {
                return false;  // Pattern changed, optimization no longer valid
            }
        }
        true
    }
}
```

## Salsa Integration

### Query Definitions

```rust
#[salsa::query_group(OptimizerDatabaseStorage)]
pub trait OptimizerDatabase: SourceDatabase {
    /// Find all applicable optimization rules for current pipeline
    fn applicable_rules(&self) -> Arc<Vec<ApplicableRule>>;

    /// Get the optimized physical plan for a model
    fn optimized_plan(&self, model: ModelId) -> Arc<PhysicalPlan>;

    /// Check if a specific optimization is still valid
    fn optimization_valid(&self, rewrite_id: RewriteId) -> bool;
}

pub struct ApplicableRule {
    pub rule: Arc<dyn OptimizationRule>,
    pub match_info: RuleMatch,
    pub cost_estimate: CostEstimate,
}
```

### Query Implementation

```rust
fn applicable_rules(db: &dyn OptimizerDatabase) -> Arc<Vec<ApplicableRule>> {
    let rules = db.registered_rules();  // Input query
    let mut applicable = Vec::new();

    for rule in rules.iter() {
        let ctx = RuleContext { db };

        if let Some(match_info) = rule.matches(&ctx) {
            let cost_estimate = rule.estimate_benefit(&ctx, &match_info);

            // Only include if beneficial
            if cost_estimate.optimized_cost < cost_estimate.current_cost {
                applicable.push(ApplicableRule {
                    rule: rule.clone(),
                    match_info,
                    cost_estimate,
                });
            }
        }
    }

    Arc::new(applicable)
}
```

### Incremental Recomputation

When a model changes:
1. Salsa marks `file_text()` as dirty
2. Cascades to `logical_plan(model_id)`
3. `applicable_rules()` depends on `logical_plan()`, so gets recomputed
4. `optimization_valid()` is checked for existing rewrites
5. If invalid, rewrite is removed and new optimizations discovered

## LSP Integration for User Consent

For semantic optimizations that require approval:

```rust
// In LSP server
async fn code_action(&self, params: CodeActionParams) -> Result<Vec<CodeAction>> {
    let applicable = self.db.applicable_rules();

    let mut actions = Vec::new();

    for rule in applicable.iter() {
        if matches!(rule.rule.optimization_type(), OptimizationType::Semantic { .. }) {
            // Suggest to user
            actions.push(CodeAction {
                title: format!(
                    "Optimize: {} (saves ~{})",
                    rule.rule.name(),
                    rule.cost_estimate.optimized_cost.estimated_dollars
                ),
                kind: Some(CodeActionKind::REFACTOR),
                command: Some(Command {
                    command: "sqt.applyOptimization".to_string(),
                    arguments: vec![json!(rule.match_info)],
                }),
                ..Default::default()
            });
        }
    }

    Ok(actions)
}
```

## Summary

The API design supports:

✅ **Two types of optimizations** (transparent vs semantic)
✅ **Salsa integration** for incremental recomputation
✅ **Pattern matching** via flexible `matches()` method
✅ **Cost estimation** for decision-making
✅ **Correctness tracking** via `is_valid()`
✅ **User consent** for lossy transformations via LSP
✅ **Extensibility** - users can write custom rules

Next steps:
1. Implement core traits and context types
2. Build Example 1 & 2 rules using this API
3. Test with Salsa database
4. Integrate with LSP for suggestions
