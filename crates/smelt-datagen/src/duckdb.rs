//! DuckDB bulk writer using the Appender API.

use crate::session::SessionGenerator;
use anyhow::{Context, Result};
use duckdb::{params, Connection};
use std::path::Path;

/// Write sessions to a DuckDB database using the bulk Appender API.
///
/// # Arguments
/// * `db_path` - Path to the DuckDB database file
/// * `table_name` - Name of the table to create and populate
/// * `seed` - Random seed for deterministic generation
/// * `num_sessions` - Total number of sessions to generate
/// * `num_days` - Number of days to spread sessions across
/// * `start_date` - First day of the date range
pub fn write_sessions_to_duckdb(
    db_path: &Path,
    table_name: &str,
    seed: u64,
    num_sessions: usize,
    num_days: u32,
    start_date: chrono::NaiveDate,
    progress_callback: Option<&dyn Fn(usize, usize)>,
) -> Result<usize> {
    let conn = Connection::open(db_path)
        .with_context(|| format!("Failed to open DuckDB database: {:?}", db_path))?;

    // Create table
    let create_sql = format!(
        r#"
        CREATE OR REPLACE TABLE {} (
            visitor_id VARCHAR NOT NULL,
            session_id VARCHAR NOT NULL,
            platform VARCHAR NOT NULL,
            visit_source VARCHAR NOT NULL,
            visit_campaign VARCHAR,
            widget_views INTEGER NOT NULL,
            session_date DATE NOT NULL,
            product_views INTEGER NOT NULL,
            product_category VARCHAR NOT NULL,
            product_revenue INTEGER NOT NULL,
            product_purchase_count INTEGER NOT NULL
        )
        "#,
        table_name
    );

    conn.execute(&create_sql, [])
        .with_context(|| format!("Failed to create table: {}", table_name))?;

    // Generate sessions and insert using Appender
    let generator = SessionGenerator::new(seed, start_date, num_days, num_sessions);

    // Use Appender for bulk inserts (much faster than individual INSERTs)
    let mut appender = conn
        .appender(table_name)
        .with_context(|| format!("Failed to create appender for table: {}", table_name))?;

    let mut count = 0;
    let batch_size = 100_000;

    for session in generator.generate(seed) {
        appender
            .append_row(params![
                session.visitor_id.to_string(),
                session.session_id.to_string(),
                session.platform.as_str(),
                session.visit_source.as_str(),
                session.visit_campaign,
                session.widget_views,
                session.session_date.to_string(),
                session.product_views,
                session.product_category.as_str(),
                session.product_revenue,
                session.product_purchase_count,
            ])
            .with_context(|| format!("Failed to append row {}", count))?;

        count += 1;

        if count % batch_size == 0 {
            if let Some(cb) = progress_callback {
                cb(count, num_sessions);
            }
        }
    }

    // Flush remaining rows
    appender.flush().context("Failed to flush appender")?;

    if let Some(cb) = progress_callback {
        cb(count, num_sessions);
    }

    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use tempfile::TempDir;

    #[test]
    fn test_write_small_dataset() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.duckdb");

        let start_date = NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();
        let count =
            write_sessions_to_duckdb(&db_path, "sessions", 42, 1000, 30, start_date, None).unwrap();

        assert!(count > 0);

        // Verify data was written
        let conn = Connection::open(&db_path).unwrap();
        let row_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM sessions", [], |row| row.get(0))
            .unwrap();

        assert_eq!(row_count as usize, count);
    }

    #[test]
    fn test_deterministic_output() {
        let temp_dir1 = TempDir::new().unwrap();
        let temp_dir2 = TempDir::new().unwrap();
        let db_path1 = temp_dir1.path().join("test1.duckdb");
        let db_path2 = temp_dir2.path().join("test2.duckdb");

        let start_date = NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();

        write_sessions_to_duckdb(&db_path1, "sessions", 42, 100, 30, start_date, None).unwrap();
        write_sessions_to_duckdb(&db_path2, "sessions", 42, 100, 30, start_date, None).unwrap();

        let conn1 = Connection::open(&db_path1).unwrap();
        let conn2 = Connection::open(&db_path2).unwrap();

        // Compare first few rows
        let mut stmt1 = conn1
            .prepare("SELECT visitor_id, session_id FROM sessions ORDER BY session_id LIMIT 10")
            .unwrap();
        let mut stmt2 = conn2
            .prepare("SELECT visitor_id, session_id FROM sessions ORDER BY session_id LIMIT 10")
            .unwrap();

        let rows1: Vec<(String, String)> = stmt1
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();

        let rows2: Vec<(String, String)> = stmt2
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();

        assert_eq!(rows1, rows2);
    }
}
