//! Example: Generate realistic behavioral event data and load into DuckDB
//!
//! This example demonstrates how to use the smelt-testdata crate to generate
//! deterministic, realistic test data and load it into a DuckDB database.
//!
//! Run with: cargo run -p smelt-testdata --example generate_events

use smelt_backend::Backend;
use smelt_backend_duckdb::DuckDbBackend;
use smelt_testdata::{TestDataBuilder, TestDataGenerator, TestDataLoader};
use std::path::Path;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("=== smelt-testdata DuckDB Example ===\n");

    // 1. Generate test data
    println!("1. Generating test data...\n");

    let config = TestDataBuilder::new()
        .seed(42) // Reproducible - same seed = same data
        .visitors(100000000) // 100,000,000 unique visitors
        .last_n_days(30) // 30 days of data
        .ecommerce_events() // E-commerce event types
        .mobile_first() // Mobile-heavy platform distribution
        .db_schema("analytics") // Schema name in DuckDB
        .build();

    let generator = TestDataGenerator::new(config);
    let data = generator.generate();

    println!("   {}", data.summary());
    println!();

    // 2. Create DuckDB database and load data
    println!("2. Loading data into DuckDB...\n");

    let db_path = Path::new("testdata_example.duckdb");
    let backend = DuckDbBackend::new(db_path, "analytics").await?;

    let result = backend.load_test_data("analytics", &data).await?;
    println!("   {}", result);
    println!();

    // 3. Run analytics queries
    println!("3. Running analytics queries...\n");

    // Query: Events by platform
    println!("   Events by platform:");
    let results = backend
        .execute_sql(
            "SELECT platform, COUNT(*) as count
             FROM analytics.events
             GROUP BY platform
             ORDER BY count DESC",
        )
        .await?;
    print_results(&results);

    // Query: Top event types
    println!("\n   Top 5 event types:");
    let results = backend
        .execute_sql(
            "SELECT event_type, COUNT(*) as count
             FROM analytics.events
             GROUP BY event_type
             ORDER BY count DESC
             LIMIT 5",
        )
        .await?;
    print_results(&results);

    // Query: Sessions per visitor distribution
    println!("\n   Visitor engagement (sessions per visitor):");
    let results = backend
        .execute_sql(
            "WITH visitor_sessions AS (
                SELECT visitor_id, COUNT(*) as session_count
                FROM analytics.sessions
                GROUP BY visitor_id
             )
             SELECT
                CASE
                    WHEN session_count = 1 THEN '1 (one-time)'
                    WHEN session_count BETWEEN 2 AND 4 THEN '2-4 (regular)'
                    ELSE '5+ (power user)'
                END as engagement_tier,
                COUNT(*) as visitor_count
             FROM visitor_sessions
             GROUP BY 1
             ORDER BY 1",
        )
        .await?;
    print_results(&results);

    // Query: Events per day
    println!("\n   Events per day (last 7 days):");
    let results = backend
        .execute_sql(
            "SELECT
                DATE_TRUNC('day', timestamp) as day,
                COUNT(*) as events
             FROM analytics.events
             GROUP BY 1
             ORDER BY 1 DESC
             LIMIT 7",
        )
        .await?;
    print_results(&results);

    // Query: Conversion funnel
    println!("\n   E-commerce funnel:");
    let results = backend
        .execute_sql(
            "SELECT
                event_type,
                COUNT(DISTINCT visitor_id) as unique_visitors,
                COUNT(*) as total_events
             FROM analytics.events
             WHERE event_type IN ('page_view', 'product_view', 'add_to_cart', 'checkout_start', 'purchase')
             GROUP BY event_type
             ORDER BY total_events DESC",
        )
        .await?;
    print_results(&results);

    // Query: Average session duration
    println!("\n   Session statistics:");
    let results = backend
        .execute_sql(
            "SELECT
                ROUND(AVG(duration_minutes), 1) as avg_duration_min,
                ROUND(MIN(duration_minutes), 1) as min_duration_min,
                ROUND(MAX(duration_minutes), 1) as max_duration_min,
                COUNT(*) as total_sessions
             FROM analytics.sessions",
        )
        .await?;
    print_results(&results);

    // 4. Show table row counts
    println!("\n4. Table summary:\n");
    let visitors_count = backend.get_row_count("analytics", "visitors").await?;
    let sessions_count = backend.get_row_count("analytics", "sessions").await?;
    let events_count = backend.get_row_count("analytics", "events").await?;

    println!("   analytics.visitors: {} rows", visitors_count);
    println!("   analytics.sessions: {} rows", sessions_count);
    println!("   analytics.events:   {} rows", events_count);

    // 5. Preview tables
    println!("\n5. Sample data from events table:\n");
    let preview = backend.get_preview("analytics", "events", 5).await?;
    print_results(&preview);

    println!("\n=== Done! Database saved to: {} ===", db_path.display());

    Ok(())
}

fn print_results(batches: &[arrow::array::RecordBatch]) {
    use arrow::util::pretty::pretty_format_batches;

    if batches.is_empty() || batches.iter().all(|b| b.num_rows() == 0) {
        println!("   (no results)");
        return;
    }

    match pretty_format_batches(batches) {
        Ok(formatted) => {
            for line in formatted.to_string().lines() {
                println!("   {}", line);
            }
        }
        Err(e) => println!("   Error formatting results: {}", e),
    }
}
