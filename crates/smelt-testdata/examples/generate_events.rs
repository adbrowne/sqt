//! Example: Generate realistic behavioral event data and load into DuckDB
//!
//! This example demonstrates how to use the smelt-testdata crate to generate
//! deterministic, realistic test data and stream it into a DuckDB database.
//!
//! Run with: cargo run -p smelt-testdata --example generate_events

use smelt_backend::Backend;
use smelt_backend_duckdb::DuckDbBackend;
use smelt_testdata::{TestDataBuilder, TestDataGenerator, TestDataLoader};
use std::path::Path;
use std::time::Instant;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("=== smelt-testdata Streaming DuckDB Example ===\n");

    // 1. Configure test data generation
    println!("1. Configuring test data generation...\n");

    let config = TestDataBuilder::new()
        .seed(42) // Reproducible - same seed = same data
        .visitors(100_000_000) // 100M unique visitors
        .last_n_days(30) // 30 days of data
        .ecommerce_events() // E-commerce event types
        .mobile_first() // Mobile-heavy platform distribution
        .db_schema("analytics") // Schema name in DuckDB
        .build();

    let generator = TestDataGenerator::new(config);

    // 2. Create DuckDB database and tables
    println!("2. Creating DuckDB database and tables...\n");

    let db_path = Path::new("testdata_example.duckdb");
    let backend = DuckDbBackend::new(db_path, "analytics").await?;

    // Create tables upfront (only once, before streaming)
    backend.create_test_tables("analytics").await?;
    println!("   Tables created in schema 'analytics'\n");

    // 3. Stream data in batches
    println!("3. Streaming data into DuckDB...\n");

    let start = Instant::now();
    let mut total_visitors = 0usize;
    let mut total_sessions = 0usize;
    let mut total_events = 0usize;

    // Process 10,000 visitors per batch - adjust based on memory constraints
    let batch_size = 10_000;
    let batch_iterator = generator.stream_batches(batch_size);

    for batch in batch_iterator {
        let batch_start = Instant::now();

        // Load this batch into DuckDB
        let result = backend.load_batch("analytics", &batch).await?;

        total_visitors += result.visitors_loaded;
        total_sessions += result.sessions_loaded;
        total_events += result.events_loaded;

        println!(
            "   {} ({} rows) in {:.2}s",
            result,
            result.total_rows(),
            batch_start.elapsed().as_secs_f64()
        );
    }

    let elapsed = start.elapsed();
    println!(
        "\n   Done! Loaded {} visitors, {} sessions, {} events in {:.2}s",
        total_visitors,
        total_sessions,
        total_events,
        elapsed.as_secs_f64()
    );
    println!(
        "   Rate: {:.0} events/second\n",
        total_events as f64 / elapsed.as_secs_f64()
    );

    // 4. Run analytics queries
    println!("4. Running analytics queries...\n");

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

    // Query: E-commerce funnel
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

    // 5. Show table row counts
    println!("\n5. Final table summary:\n");
    let visitors_count = backend.get_row_count("analytics", "visitors").await?;
    let sessions_count = backend.get_row_count("analytics", "sessions").await?;
    let events_count = backend.get_row_count("analytics", "events").await?;

    println!("   analytics.visitors: {} rows", visitors_count);
    println!("   analytics.sessions: {} rows", sessions_count);
    println!("   analytics.events:   {} rows", events_count);

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
