//! Example: Generate realistic behavioral event data and load into DuckDB
//!
//! This example demonstrates how to use the smelt-testdata crate to generate
//! deterministic, realistic test data and stream it into a DuckDB database.
//!
//! Uses DuckDB's Arrow Appender API for fast bulk loading (~100K+ events/sec).
//!
//! Run with: cargo run -p smelt-testdata --example generate_events --release

use smelt_backend::Backend;
use smelt_backend_duckdb::{BulkLoadOptions, DuckDbBackend};
use smelt_testdata::{ArrowOutput, TestDataBuilder, TestDataGenerator, TestDataLoader};
use std::path::Path;
use std::time::Instant;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("=== smelt-testdata Streaming DuckDB Example ===\n");

    // 1. Configure test data generation
    println!("1. Configuring test data generation...\n");

    let config = TestDataBuilder::new()
        .seed(42) // Reproducible - same seed = same data
        .visitors(1_000_000) // 1M visitors (use 100_000_000 for large test)
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

    // 3. Stream data in batches using Arrow bulk loading
    println!("3. Streaming data into DuckDB (Arrow bulk load)...\n");

    let start = Instant::now();
    let mut total_visitors = 0usize;
    let mut total_sessions = 0usize;
    let mut total_events = 0usize;

    let arrow_output = ArrowOutput::new();

    // Process 100 visitors per batch (~500 events) to avoid DuckDB Arrow bug with large batches
    let batch_size = 100;
    let batch_iterator = generator.stream_batches(batch_size);

    // Checkpoint every N batches to prevent WAL buildup
    let checkpoint_interval = 10;

    for batch in batch_iterator {
        let batch_start = Instant::now();

        // Convert to Arrow RecordBatches
        let visitors_batch = arrow_output.visitors_to_batch(&batch.visitors);
        let sessions_batch = arrow_output.sessions_to_batch(&batch.sessions);
        let events_batch = arrow_output.events_to_batch(&batch.events);

        // Use DuckDB's fast Arrow Appender API
        let should_checkpoint = (batch.batch_index + 1) % checkpoint_interval == 0;
        let options = BulkLoadOptions {
            checkpoint: should_checkpoint,
        };

        backend
            .bulk_load_arrow("analytics", "visitors", &[visitors_batch], options.clone())
            .await?;
        backend
            .bulk_load_arrow("analytics", "sessions", &[sessions_batch], options.clone())
            .await?;
        backend
            .bulk_load_arrow("analytics", "events", &[events_batch], options)
            .await?;

        total_visitors += batch.visitors.len();
        total_sessions += batch.sessions.len();
        total_events += batch.events.len();

        let batch_elapsed = batch_start.elapsed();
        let events_per_sec = batch.events.len() as f64 / batch_elapsed.as_secs_f64();
        let checkpoint_marker = if should_checkpoint {
            " [checkpoint]"
        } else {
            ""
        };

        println!(
            "   Batch {}/{}: {} visitors, {} sessions, {} events in {:.2}s ({:.0} events/s){}",
            batch.batch_index + 1,
            batch.total_batches,
            batch.visitors.len(),
            batch.sessions.len(),
            batch.events.len(),
            batch_elapsed.as_secs_f64(),
            events_per_sec,
            checkpoint_marker
        );
    }

    // Final checkpoint
    backend.checkpoint().await?;

    let elapsed = start.elapsed();
    println!(
        "\n   Done! Loaded {} visitors, {} sessions, {} events in {:.2}s",
        total_visitors,
        total_sessions,
        total_events,
        elapsed.as_secs_f64()
    );
    println!(
        "   Overall rate: {:.0} events/second\n",
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
