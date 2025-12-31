use anyhow::{Context, Result};
use arrow::util::pretty;
use chrono::{Duration, NaiveDate};
use clap::{Parser, Subcommand};
use smelt_backend::{Backend, PartitionSpec};
use smelt_backend_duckdb::DuckDbBackend;
use smelt_cli::{
    executor, find_project_root, inject_time_filter, BackendType, Config, DependencyGraph,
    ModelDiscovery, SourceConfig, SqlCompiler, TimeRange,
};
use std::path::PathBuf;

#[cfg(feature = "spark")]
use smelt_backend_spark::SparkBackend;

#[derive(Parser)]
#[command(name = "smelt")]
#[command(about = "Modern data transformation framework", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run models and materialize them in the target database
    Run(RunArgs),
}

#[derive(Parser)]
struct RunArgs {
    /// Path to smelt project root
    #[arg(long, default_value = ".")]
    project_dir: PathBuf,

    /// DuckDB database file path
    #[arg(long)]
    database: Option<PathBuf>,

    /// Target environment from smelt.yml
    #[arg(long, default_value = "dev")]
    target: String,

    /// Display query results after execution
    #[arg(long)]
    show_results: bool,

    /// Show compiled SQL for each model
    #[arg(long, short)]
    verbose: bool,

    /// Parse and validate without executing
    #[arg(long)]
    dry_run: bool,

    /// Start of event time range for incremental models (ISO 8601: YYYY-MM-DD)
    #[arg(long = "event-time-start", requires = "event_time_end")]
    event_time_start: Option<String>,

    /// End of event time range for incremental models (exclusive, ISO 8601: YYYY-MM-DD)
    #[arg(long = "event-time-end", requires = "event_time_start")]
    event_time_end: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Run(args) => run(args).await,
    }
}

async fn run(args: RunArgs) -> Result<()> {
    // 1. Find project root
    let project_dir = find_project_root(&args.project_dir)
        .with_context(|| format!("Failed to find project root from {:?}", args.project_dir))?;

    println!("Project directory: {}", project_dir.display());

    // 2. Load configuration
    let config =
        Config::load(&project_dir).with_context(|| "Failed to load smelt.yml configuration")?;

    println!("Project: {} (version {})", config.name, config.version);

    // Get target config
    let target_config = config.targets.get(&args.target).ok_or_else(|| {
        anyhow::anyhow!(
            "Target '{}' not found in smelt.yml. Available targets: {}",
            args.target,
            config
                .targets
                .keys()
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        )
    })?;

    // Load source configuration (optional)
    let sources = SourceConfig::load(&project_dir).ok();

    if let Some(ref source_config) = sources {
        let source_count: usize = source_config.sources.values().map(|s| s.tables.len()).sum();
        println!("Loaded {} source tables", source_count);
    }

    // 3. Discover models
    let discovery = ModelDiscovery::new(project_dir.clone(), config.model_paths.clone());
    let models = discovery
        .discover_models()
        .with_context(|| "Failed to discover models")?;

    println!("Found {} models", models.len());

    // Report any parse errors
    for model in &models {
        if !model.parse_errors.is_empty() {
            eprintln!("\nWarning: Parse errors in {}:", model.name);
            for error in &model.parse_errors {
                eprintln!("  - {} at {:?}", error.message, error.range);
            }
        }
    }

    // 4. Build dependency graph
    let graph = DependencyGraph::build(models, sources.as_ref())
        .with_context(|| "Failed to build dependency graph")?;

    graph
        .validate()
        .with_context(|| "Dependency validation failed")?;

    // 5. Determine execution order
    let execution_order = graph
        .execution_order()
        .with_context(|| "Failed to determine execution order")?;

    println!(
        "\nExecution order: {}",
        execution_order
            .iter()
            .enumerate()
            .map(|(i, name)| format!("{}. {}", i + 1, name))
            .collect::<Vec<_>>()
            .join(" → ")
    );

    if args.dry_run {
        println!("\n[DRY RUN] Skipping execution");
        return Ok(());
    }

    // 6. Create backend based on target type
    let backend: Box<dyn Backend> = match target_config.backend_type() {
        BackendType::DuckDB => {
            let database = target_config
                .database
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("DuckDB target requires 'database' field"))?;

            let db_path = args.database.unwrap_or_else(|| project_dir.join(database));
            println!("\nBackend: DuckDB");
            println!("Database: {}", db_path.display());

            Box::new(
                DuckDbBackend::new(&db_path, &target_config.schema)
                    .await
                    .with_context(|| format!("Failed to initialize DuckDB at {:?}", db_path))?,
            )
        }
        BackendType::Spark => {
            #[cfg(feature = "spark")]
            {
                let connect_url = target_config
                    .connect_url
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("Spark target requires 'connect_url' field"))?;

                let default_catalog = "spark_catalog".to_string();
                let catalog = target_config.catalog.as_ref().unwrap_or(&default_catalog);

                println!("\nBackend: Spark");
                println!("Connect URL: {}", connect_url);
                println!("Catalog: {}", catalog);

                Box::new(
                    SparkBackend::new(connect_url, catalog, &target_config.schema)
                        .await
                        .with_context(|| {
                            format!("Failed to connect to Spark at {}", connect_url)
                        })?,
                )
            }
            #[cfg(not(feature = "spark"))]
            {
                return Err(anyhow::anyhow!(
                    "Spark backend not available. Rebuild with --features spark"
                ));
            }
        }
    };

    // 7. Validate sources exist (if sources.yml present)
    if let Some(ref source_config) = sources {
        executor::validate_sources(backend.as_ref(), source_config)
            .await
            .with_context(|| "Source validation failed")?;
    }

    // 8. Parse time range if provided (for incremental processing)
    let time_range = match (&args.event_time_start, &args.event_time_end) {
        (Some(start), Some(end)) => {
            // Validate date format
            NaiveDate::parse_from_str(start, "%Y-%m-%d").with_context(|| {
                format!("Invalid start date format: {}. Expected YYYY-MM-DD", start)
            })?;
            NaiveDate::parse_from_str(end, "%Y-%m-%d").with_context(|| {
                format!("Invalid end date format: {}. Expected YYYY-MM-DD", end)
            })?;

            println!("\nTime range: {} to {} (exclusive)", start, end);
            Some(TimeRange {
                start: start.clone(),
                end: end.clone(),
            })
        }
        _ => None,
    };

    // 9. Compile and execute each model
    let compiler = SqlCompiler::new(config.clone());

    println!("\n{}", "=".repeat(60));
    println!("Executing models...");
    println!("{}", "=".repeat(60));

    let mut results = Vec::new();

    for model_name in &execution_order {
        let model = graph.get_model(model_name)?;

        // Check if this model should be run incrementally
        // SQL metadata takes precedence over smelt.yml
        let inc_config = config
            .get_incremental_with_metadata(model_name, model.metadata.as_ref().map(|b| b.as_ref()));
        let is_incremental = time_range.is_some() && inc_config.is_some();

        if is_incremental {
            let range = time_range.as_ref().unwrap();
            let inc = inc_config.unwrap();

            println!("\n▶ Running model: {} (incremental)", model_name);

            // Transform SQL to filter by time range
            let transformed_sql = inject_time_filter(&model.content, &inc.event_time_column, range)
                .with_context(|| format!("Failed to transform SQL for model: {}", model_name))?;

            // Compile with transformed SQL
            let compiled = compiler
                .compile_with_sql(model, &target_config.schema, &transformed_sql)
                .with_context(|| format!("Failed to compile model: {}", model_name))?;

            if args.verbose {
                println!("\n  Transformed SQL:");
                println!("  {}", "─".repeat(58));
                for line in compiled.sql.lines() {
                    println!("  {}", line);
                }
                println!("  {}", "─".repeat(58));
            }

            // Generate partition values for DELETE
            let partition_values = generate_partition_dates(&range.start, &range.end)?;
            println!(
                "  Partitions to update: {} ({} days)",
                if partition_values.len() <= 3 {
                    partition_values.join(", ")
                } else {
                    format!(
                        "{}, ..., {}",
                        partition_values.first().unwrap(),
                        partition_values.last().unwrap()
                    )
                },
                partition_values.len()
            );

            let partition = PartitionSpec {
                column: inc.partition_column.clone(),
                values: partition_values,
            };

            // Execute incrementally
            let result = executor::execute_model_incremental(
                backend.as_ref(),
                &compiled,
                &target_config.schema,
                partition,
                args.show_results,
            )
            .await
            .with_context(|| format!("Failed to execute model: {}", model_name))?;

            println!(
                "  ✓ {} ({} rows, {:?})",
                result.model_name, result.row_count, result.duration
            );

            // Show preview if requested
            if let Some(ref batches) = result.preview {
                println!("\n  Preview:");
                pretty::print_batches(batches).with_context(|| "Failed to print result preview")?;
                println!();
            }

            results.push(result);
        } else {
            // Standard full refresh path
            if time_range.is_some() && inc_config.is_none() {
                println!(
                    "\n▶ Running model: {} (full refresh - not configured for incremental)",
                    model_name
                );
            } else {
                println!("\n▶ Running model: {}", model_name);
            }

            // Compile
            let compiled = compiler
                .compile(model, &target_config.schema)
                .with_context(|| format!("Failed to compile model: {}", model_name))?;

            if args.verbose {
                println!("\n  Compiled SQL:");
                println!("  {}", "─".repeat(58));
                for line in compiled.sql.lines() {
                    println!("  {}", line);
                }
                println!("  {}", "─".repeat(58));
            }

            // Execute
            let result = executor::execute_model(
                backend.as_ref(),
                &compiled,
                &target_config.schema,
                args.show_results,
            )
            .await
            .with_context(|| format!("Failed to execute model: {}", model_name))?;

            println!(
                "  ✓ {} ({} rows, {:?})",
                result.model_name, result.row_count, result.duration
            );

            // Show preview if requested
            if let Some(ref batches) = result.preview {
                println!("\n  Preview:");
                pretty::print_batches(batches).with_context(|| "Failed to print result preview")?;
                println!();
            }

            results.push(result);
        }
    }

    // 9. Summary
    println!("\n{}", "=".repeat(60));
    println!("Summary");
    println!("{}", "=".repeat(60));
    println!("✓ Executed {} models successfully", results.len());

    let total_duration: std::time::Duration = results.iter().map(|r| r.duration).sum();
    println!("  Total time: {:?}", total_duration);

    Ok(())
}

/// Generate partition date values from a time range.
/// Returns a list of date strings in YYYY-MM-DD format.
fn generate_partition_dates(start: &str, end: &str) -> Result<Vec<String>> {
    let start_date = NaiveDate::parse_from_str(start, "%Y-%m-%d")
        .with_context(|| format!("Invalid start date: {}", start))?;
    let end_date = NaiveDate::parse_from_str(end, "%Y-%m-%d")
        .with_context(|| format!("Invalid end date: {}", end))?;

    if start_date >= end_date {
        return Err(anyhow::anyhow!(
            "Start date ({}) must be before end date ({})",
            start,
            end
        ));
    }

    let mut dates = Vec::new();
    let mut current = start_date;
    while current < end_date {
        dates.push(current.format("%Y-%m-%d").to_string());
        current += Duration::days(1);
    }

    Ok(dates)
}
