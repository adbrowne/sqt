/// Example 2: Split Large GROUP BY (Optimized Version)
///
/// Solution: Split multi-dimensional GROUP BY into independent single-dimension aggregations
///
/// Instead of:
///   GROUP BY user_id, country, device_type, date  (huge shuffle)
///
/// Do:
///   GROUP BY user_id    (smaller shuffle)
///   UNION ALL
///   GROUP BY country    (smaller shuffle)
///   UNION ALL
///   GROUP BY device_type (smaller shuffle)
///   UNION ALL
///   GROUP BY date       (smaller shuffle)
///
/// Result: 4 small shuffles instead of 1 massive shuffle
///
/// This is similar to computing a CUBE or ROLLUP, but more efficient because
/// we only compute the dimensions we actually need (not all combinations).
use anyhow::Result;
use duckdb::Connection;
use smelt_examples::utils::{create_duckdb_connection, execute_and_print};

fn setup_events_data(conn: &Connection) -> Result<()> {
    // Same data setup as naive version
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

        INSERT INTO events
        WITH users AS (
            SELECT UNNEST(RANGE(1, 1001)) AS user_id
        ),
        countries AS (
            SELECT UNNEST(['US', 'UK', 'CA', 'DE', 'FR', 'JP', 'AU', 'BR']) AS country
        ),
        devices AS (
            SELECT UNNEST(['iOS', 'Android', 'Web']) AS device_type
        ),
        dates AS (
            SELECT DATE '2024-01-01' + INTERVAL (d) DAY AS event_date
            FROM UNNEST(RANGE(0, 30)) AS t(d)
        )
        SELECT
            ROW_NUMBER() OVER () AS event_id,
            user_id,
            country,
            device_type,
            event_date,
            (RANDOM() * 100)::DECIMAL(10,2) AS revenue
        FROM users
        CROSS JOIN countries
        CROSS JOIN devices
        CROSS JOIN dates
        WHERE RANDOM() < 0.01
        LIMIT 10000;
        "
    )?;

    Ok(())
}

fn split_groupby_by_user(conn: &Connection) -> Result<()> {
    let sql = "
    SELECT
        user_id,
        NULL::VARCHAR AS country,
        NULL::VARCHAR AS device_type,
        NULL::DATE AS event_date,
        'by_user' AS dimension,
        COUNT(*) AS event_count,
        SUM(revenue) AS total_revenue,
        AVG(revenue) AS avg_revenue
    FROM events
    GROUP BY user_id
    ORDER BY total_revenue DESC
    LIMIT 10
    ";

    execute_and_print(conn, sql, "Optimized: GROUP BY user_id only")?;
    Ok(())
}

fn split_groupby_by_country(conn: &Connection) -> Result<()> {
    let sql = "
    SELECT
        NULL::INTEGER AS user_id,
        country,
        NULL::VARCHAR AS device_type,
        NULL::DATE AS event_date,
        'by_country' AS dimension,
        COUNT(*) AS event_count,
        SUM(revenue) AS total_revenue,
        AVG(revenue) AS avg_revenue
    FROM events
    GROUP BY country
    ORDER BY total_revenue DESC
    ";

    execute_and_print(conn, sql, "Optimized: GROUP BY country only")?;
    Ok(())
}

fn split_groupby_by_device(conn: &Connection) -> Result<()> {
    let sql = "
    SELECT
        NULL::INTEGER AS user_id,
        NULL::VARCHAR AS country,
        device_type,
        NULL::DATE AS event_date,
        'by_device' AS dimension,
        COUNT(*) AS event_count,
        SUM(revenue) AS total_revenue,
        AVG(revenue) AS avg_revenue
    FROM events
    GROUP BY device_type
    ORDER BY total_revenue DESC
    ";

    execute_and_print(conn, sql, "Optimized: GROUP BY device_type only")?;
    Ok(())
}

fn split_groupby_by_date(conn: &Connection) -> Result<()> {
    let sql = "
    SELECT
        NULL::INTEGER AS user_id,
        NULL::VARCHAR AS country,
        NULL::VARCHAR AS device_type,
        event_date,
        'by_date' AS dimension,
        COUNT(*) AS event_count,
        SUM(revenue) AS total_revenue,
        AVG(revenue) AS avg_revenue
    FROM events
    GROUP BY event_date
    ORDER BY total_revenue DESC
    LIMIT 10
    ";

    execute_and_print(conn, sql, "Optimized: GROUP BY event_date only")?;
    Ok(())
}

fn combined_split_groupby(conn: &Connection) -> Result<()> {
    // Show all dimensions in a single UNION ALL query
    let sql = "
    SELECT 'by_user' as dimension, user_id, NULL as country, NULL as device_type, NULL as event_date,
           COUNT(*) as event_count, SUM(revenue) as total_revenue
    FROM events GROUP BY user_id

    UNION ALL

    SELECT 'by_country', NULL, country, NULL, NULL,
           COUNT(*), SUM(revenue)
    FROM events GROUP BY country

    UNION ALL

    SELECT 'by_device', NULL, NULL, device_type, NULL,
           COUNT(*), SUM(revenue)
    FROM events GROUP BY device_type

    UNION ALL

    SELECT 'by_date', NULL, NULL, NULL, event_date::VARCHAR,
           COUNT(*), SUM(revenue)
    FROM events GROUP BY event_date

    ORDER BY total_revenue DESC
    LIMIT 20
    ";

    execute_and_print(conn, sql, "Combined: All Dimensions via UNION ALL")?;
    Ok(())
}

fn compare_shuffle_sizes(conn: &Connection) -> Result<()> {
    println!("\n╔════════════════════════════════════════════════════════════╗");
    println!("║  Shuffle Size Comparison                                   ║");
    println!("╚════════════════════════════════════════════════════════════╝\n");

    // Count rows in each split query
    let sql = "
    SELECT
        'Naive (all dimensions)' AS approach,
        COUNT(*) AS shuffle_row_count
    FROM (
        SELECT user_id, country, device_type, event_date
        FROM events
        GROUP BY user_id, country, device_type, event_date
    )

    UNION ALL

    SELECT 'Optimized (by user)', COUNT(*)
    FROM (SELECT user_id FROM events GROUP BY user_id)

    UNION ALL

    SELECT 'Optimized (by country)', COUNT(*)
    FROM (SELECT country FROM events GROUP BY country)

    UNION ALL

    SELECT 'Optimized (by device)', COUNT(*)
    FROM (SELECT device_type FROM events GROUP BY device_type)

    UNION ALL

    SELECT 'Optimized (by date)', COUNT(*)
    FROM (SELECT event_date FROM events GROUP BY event_date)

    UNION ALL

    SELECT 'Optimized (total)', (
        (SELECT COUNT(DISTINCT user_id) FROM events) +
        (SELECT COUNT(DISTINCT country) FROM events) +
        (SELECT COUNT(DISTINCT device_type) FROM events) +
        (SELECT COUNT(DISTINCT event_date) FROM events)
    )
    ";

    execute_and_print(conn, sql, "Row Counts: Naive vs Optimized")?;

    println!("\n📊 Analysis:");
    println!("   • Naive approach: ~7000 intermediate rows in shuffle");
    println!("   • Optimized approach: ~1000 total rows across 4 shuffles");
    println!("   • Reduction: ~85% fewer rows shuffled");
    println!("   • Each optimized shuffle is independent (can parallelize)\n");

    println!("💰 Cost Implications (Spark/Databricks):");
    println!("   • Shuffle cost ∝ data volume × network hops");
    println!("   • Naive: 1 large shuffle blocks entire job");
    println!("   • Optimized: 4 small shuffles run in parallel");
    println!("   • Expected speedup: 3-10x for large datasets");
    println!("   • Cost reduction: 50-80% (less shuffle I/O)\n");

    Ok(())
}

fn main() -> Result<()> {
    println!("\n╔════════════════════════════════════════════════════════════╗");
    println!("║  Example 2: Split Large GROUP BY (OPTIMIZED)              ║");
    println!("╚════════════════════════════════════════════════════════════╝\n");

    println!("Optimization: Split multi-dimensional GROUP BY into independent queries\n");

    let conn = create_duckdb_connection()?;

    // Setup data
    setup_events_data(&conn)?;

    // Show each split query
    split_groupby_by_user(&conn)?;
    split_groupby_by_country(&conn)?;
    split_groupby_by_device(&conn)?;
    split_groupby_by_date(&conn)?;

    // Combined view
    combined_split_groupby(&conn)?;

    // Compare shuffle sizes
    compare_shuffle_sizes(&conn)?;

    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║  Key Insights for Optimizer API Design                    ║");
    println!("╚════════════════════════════════════════════════════════════╝\n");

    println!("1. Pattern Detection:");
    println!("   - Detect: GROUP BY with multiple high-cardinality dimensions");
    println!("   - Check: Are aggregates decomposable? (SUM, COUNT, MIN, MAX: yes; MEDIAN: no)");
    println!("   - Estimate: Shuffle cost (rows × dimensions × key_size)\n");

    println!("2. When to Apply:");
    println!("   - High cardinality in multiple dimensions");
    println!("   - Sparse combinations (most groups don't exist)");
    println!("   - Decomposable aggregates only");
    println!("   - NOT beneficial for: low cardinality, dense data, or MEDIAN/PERCENTILE\n");

    println!("3. Rewrite Strategy:");
    println!("   - Split into N queries (one per dimension)");
    println!("   - Add NULL for non-grouped dimensions");
    println!("   - Add 'dimension' tag for identification");
    println!("   - UNION ALL the results\n");

    println!("4. Correctness Considerations:");
    println!("   - Results are NOT equivalent to naive (different schema!)");
    println!("   - This is a LOSSY optimization (loses cross-dimension info)");
    println!("   - Only valid if user wants dimension-level aggregates");
    println!("   - Optimizer must ASK user or detect intent from usage\n");

    println!("5. API Design Question:");
    println!("   - User must SPECIFY this is acceptable (not automatic!)");
    println!("   - Example hint: @optimize(split_dimensions)");
    println!("   - Or: Define separate models for each dimension explicitly\n");

    Ok(())
}
