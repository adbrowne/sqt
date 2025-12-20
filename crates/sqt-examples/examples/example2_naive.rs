/// Example 2: Split Large GROUP BY (Naive Version)
///
/// Problem: Large multi-dimensional GROUP BY creates huge intermediate shuffle in Spark/Databricks
///
/// Scenario: Computing aggregates across multiple high-cardinality dimensions
/// - user_id (millions of users)
/// - country (hundreds of countries)
/// - device_type (tens of device types)
/// - date (365+ days)
///
/// Naive approach: Single GROUP BY with all dimensions
/// Result: Cartesian explosion in shuffle (millions × hundreds × tens × 365)
///
/// This example demonstrates the problem and sets up for the optimization.

use anyhow::Result;
use duckdb::Connection;
use sqt_examples::utils::{create_duckdb_connection, execute_and_print};

fn setup_events_data(conn: &Connection) -> Result<()> {
    // Create a realistic events table with high cardinality dimensions
    conn.execute_batch(
        "
        CREATE TABLE events (
            event_id INTEGER,
            user_id INTEGER,
            country VARCHAR,
            device_type VARCHAR,
            event_date DATE,
            revenue DECIMAL(10,2)
        );

        -- Insert sample data representing a large dataset
        -- In reality this would be millions of rows
        INSERT INTO events
        WITH users AS (
            SELECT UNNEST(RANGE(1, 1001)) AS user_id  -- 1000 users
        ),
        countries AS (
            SELECT UNNEST(['US', 'UK', 'CA', 'DE', 'FR', 'JP', 'AU', 'BR']) AS country
        ),
        devices AS (
            SELECT UNNEST(['iOS', 'Android', 'Web']) AS device_type
        ),
        dates AS (
            SELECT DATE '2024-01-01' + INTERVAL (d) DAY AS event_date
            FROM UNNEST(RANGE(0, 30)) AS t(d)  -- 30 days
        )
        SELECT
            ROW_NUMBER() OVER () AS event_id,
            user_id,
            country,
            device_type,
            event_date,
            (RANDOM() * 100)::DECIMAL(10,2) AS revenue
        FROM users, countries, devices, dates
        WHERE RANDOM() < 0.01  -- Sparse: only 1% of combinations have events
        LIMIT 10000;  -- Cap at 10k rows for demo
        "
    )?;

    execute_and_print(
        conn,
        "SELECT COUNT(*) as total_events,
                COUNT(DISTINCT user_id) as unique_users,
                COUNT(DISTINCT country) as unique_countries,
                COUNT(DISTINCT device_type) as unique_devices,
                COUNT(DISTINCT event_date) as unique_dates
         FROM events",
        "Events Dataset Summary"
    )?;

    Ok(())
}

fn large_multidimensional_groupby(conn: &Connection) -> Result<()> {
    // Naive approach: GROUP BY all dimensions at once
    // This creates a huge intermediate result set

    let sql = "
    SELECT
        user_id,
        country,
        device_type,
        event_date,
        COUNT(*) AS event_count,
        SUM(revenue) AS total_revenue,
        AVG(revenue) AS avg_revenue
    FROM events
    GROUP BY user_id, country, device_type, event_date
    ORDER BY total_revenue DESC
    LIMIT 20
    ";

    execute_and_print(conn, sql, "Naive: Large Multi-Dimensional GROUP BY")?;

    // Show the problem: count intermediate rows
    let count_sql = "
    SELECT COUNT(*) AS intermediate_row_count
    FROM (
        SELECT user_id, country, device_type, event_date
        FROM events
        GROUP BY user_id, country, device_type, event_date
    )
    ";

    execute_and_print(conn, count_sql, "Problem: Intermediate Shuffle Size")?;

    Ok(())
}

fn analyze_shuffle_cost(conn: &Connection) -> Result<()> {
    println!("\n╔════════════════════════════════════════════════════════════╗");
    println!("║  Why This Is Expensive in Spark/Databricks                ║");
    println!("╚════════════════════════════════════════════════════════════╝\n");

    // Show cardinality of each dimension
    execute_and_print(
        conn,
        "SELECT
            (SELECT COUNT(DISTINCT user_id) FROM events) as user_cardinality,
            (SELECT COUNT(DISTINCT country) FROM events) as country_cardinality,
            (SELECT COUNT(DISTINCT device_type) FROM events) as device_cardinality,
            (SELECT COUNT(DISTINCT event_date) FROM events) as date_cardinality,
            (SELECT COUNT(DISTINCT user_id) FROM events) *
            (SELECT COUNT(DISTINCT country) FROM events) *
            (SELECT COUNT(DISTINCT device_type) FROM events) *
            (SELECT COUNT(DISTINCT event_date) FROM events) as theoretical_max_groups",
        "Dimension Cardinalities"
    )?;

    println!("\n📊 Analysis:");
    println!("   • Each dimension has high cardinality");
    println!("   • GROUP BY creates shuffle partitions for each unique combination");
    println!("   • In Spark: All data shuffled across cluster before aggregation");
    println!("   • Shuffle size = rows × (key_size + value_size)");
    println!("   • With millions of rows, this becomes the bottleneck\n");

    println!("💡 The Insight:");
    println!("   • Most dimension combinations are sparse (don't appear)");
    println!("   • We're shuffling data for combinations that may not exist");
    println!("   • Better: Compute each dimension independently, union results");
    println!("   • Benefit: N smaller shuffles instead of 1 massive shuffle\n");

    Ok(())
}

fn main() -> Result<()> {
    println!("\n╔════════════════════════════════════════════════════════════╗");
    println!("║  Example 2: Split Large GROUP BY (NAIVE)                  ║");
    println!("╚════════════════════════════════════════════════════════════╝\n");

    println!("Scenario: OLAP-style aggregation across multiple dimensions");
    println!("Common in data warehousing, analytics cubes, reporting\n");

    let conn = create_duckdb_connection()?;

    // Setup sample data
    setup_events_data(&conn)?;

    // Show the naive approach
    large_multidimensional_groupby(&conn)?;

    // Analyze why it's expensive
    analyze_shuffle_cost(&conn)?;

    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║  Next: See example2_optimized.rs for the solution         ║");
    println!("╚════════════════════════════════════════════════════════════╝\n");

    Ok(())
}
