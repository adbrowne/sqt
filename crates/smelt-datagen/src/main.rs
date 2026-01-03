//! CLI for deterministic data generation.

use anyhow::Result;
use chrono::NaiveDate;
use clap::Parser;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

#[derive(Parser, Debug)]
#[command(name = "smelt-datagen")]
#[command(about = "Deterministic data generation for smelt")]
struct Args {
    /// Output directory for Hive-partitioned Parquet files
    #[arg(short, long, default_value = "output")]
    output: PathBuf,

    /// Random seed for deterministic generation
    #[arg(short, long, default_value = "42")]
    seed: u64,

    /// Number of sessions to generate
    #[arg(short, long, default_value = "100000000")]
    num_sessions: usize,

    /// Number of days to spread sessions across
    #[arg(short, long, default_value = "30")]
    days: u32,

    /// Start date (YYYY-MM-DD)
    #[arg(long, default_value = "2024-01-01")]
    start_date: String,

    /// Quiet mode (no progress output)
    #[arg(short, long)]
    quiet: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let start_date = NaiveDate::parse_from_str(&args.start_date, "%Y-%m-%d")
        .map_err(|e| anyhow::anyhow!("Invalid date format: {}", e))?;

    if !args.quiet {
        println!(
            "Generating {} sessions over {} days",
            args.num_sessions, args.days
        );
        println!("Output: {:?}", args.output);
        println!("Seed: {}", args.seed);
        println!();
    }

    let start_time = Instant::now();
    let last_print = AtomicU64::new(0);

    let progress_fn = |current: usize, total: usize| {
        let elapsed = start_time.elapsed().as_secs();
        let last = last_print.load(Ordering::Relaxed);

        // Print at most every second
        if elapsed > last {
            last_print.store(elapsed, Ordering::Relaxed);
            let pct = (current as f64 / total as f64) * 100.0;
            let rate = current as f64 / elapsed.max(1) as f64;
            let eta = if rate > 0.0 && current < total {
                ((total - current) as f64 / rate) as u64
            } else {
                0
            };
            eprint!(
                "\rProgress: {:.1}% ({}/{}) - {:.0} rows/sec - ETA: {}s    ",
                pct, current, total, rate, eta
            );
        }
    };

    let progress: Option<&(dyn Fn(usize, usize) + Sync)> =
        if args.quiet { None } else { Some(&progress_fn) };

    let count = smelt_datagen::parquet::write_sessions_to_parquet(
        &args.output,
        args.seed,
        args.num_sessions,
        args.days,
        start_date,
        progress,
    )?;

    let elapsed = start_time.elapsed();

    if !args.quiet {
        eprintln!();
        println!();
        println!(
            "Generated {} sessions in {:.2}s",
            count,
            elapsed.as_secs_f64()
        );
        println!("Rate: {:.0} rows/sec", count as f64 / elapsed.as_secs_f64());
    }

    Ok(())
}
