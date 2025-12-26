# smelt

Modern data transformation framework.

Andrew's Christmas 2025 holiday project.

## What is smelt?

smelt is a data transformation framework that separates **logical transformation definitions** from **physical execution planning**. Unlike dbt which uses Jinja templates, smelt parses and understands the semantics of your SQL, enabling:

- **Automatic incrementalization** - Framework analyzes what's safe to run incrementally
- **Cross-engine deployment** - Split work across DuckDB, Spark, Postgres, etc.
- **Static analysis** - Type checking, LSP support, and semantic validation
- **Rule-based optimization** - Automatic rewrites with learning from history

## Quick Example

```sql
-- models/daily_revenue.sql
-- @incremental: enabled
-- @incremental.time_column: order_date

SELECT
  order_date,
  customer_id,
  SUM(amount) as daily_revenue
FROM smelt.ref('orders')
GROUP BY 1, 2
```

```bash
# Run all models
smelt run

# Run incrementally (only process new data)
smelt run --incremental

# Preview what would run
smelt run --dry-run --verbose
```

## Current Status

**Phases 1-3 Complete**: Parser, LSP, and CLI with DuckDB execution.

See [docs/ROADMAP.md](docs/ROADMAP.md) for implementation status.

## Documentation

- **[docs/DESIGN.md](docs/DESIGN.md)** - Full language specification and architecture
- **[docs/ROADMAP.md](docs/ROADMAP.md)** - Implementation status and next steps
- **[docs/lsp_quickstart.md](docs/lsp_quickstart.md)** - Getting started with the LSP
- **[docs/architecture_overview.md](docs/architecture_overview.md)** - System design

## Getting Started

```bash
# Build
cargo build

# Run tests
cargo test

# Run the CLI
cargo run -p smelt-cli -- run --project-dir test-workspace

# Run the LSP server
cargo run -p smelt-lsp
```

## Key Differentiators from dbt

| Aspect | dbt | smelt |
|--------|-----|-------|
| Model definition | Jinja templates | Parsed semantic models |
| Incrementalization | Manual `is_incremental()` | Automatic semantic analysis |
| Type checking | None (runtime errors) | Static analysis with LSP |
| Cross-engine | One target per project | Split work across engines |
| Optimization | Manual | Rule-based with learning |

## License

MIT License - Copyright (c) 2025 Andrew Browne
