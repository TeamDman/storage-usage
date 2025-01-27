mod pdh_error;

use clap::Parser;
use eyre::WrapErr;
use pdh_error::interpret_pdh_error;
use std::thread::sleep;
use std::time::Duration;
use windows::core::w;
use windows::Win32::System::Performance::PdhAddCounterW;
use windows::Win32::System::Performance::PdhCloseQuery;
use windows::Win32::System::Performance::PdhCollectQueryData;
use windows::Win32::System::Performance::PdhGetFormattedCounterArrayW;
use windows::Win32::System::Performance::PdhOpenQueryW;
use windows::Win32::System::Performance::PDH_FMT_COUNTERVALUE_ITEM_W;
use windows::Win32::System::Performance::PDH_FMT_DOUBLE;
use windows::Win32::System::Performance::PDH_HCOUNTER;
use windows::Win32::System::Performance::PDH_HQUERY;
use windows::Win32::System::Performance::PDH_MORE_DATA;

/// Our CLI arguments
#[derive(Parser, Debug)]
#[command(version)]
struct Args {
    /// If set, enable debug logging
    #[arg(long)]
    debug: bool,
}

fn main() -> eyre::Result<()> {
    color_eyre::install()?;
    let args = Args::parse();

    // Setup logging
    let log_level = if args.debug {
        tracing::Level::DEBUG
    } else {
        tracing::Level::INFO
    };
    let env_filter = tracing_subscriber::EnvFilter::builder()
        .with_default_directive(log_level.into())
        .from_env_lossy();
    tracing_subscriber::fmt().with_env_filter(env_filter).init();

    println!("Measuring for three seconds...");

    let results = get_disk_usage_after_3s()?;
    for (name, usage) in &results {
        println!("{}: {:.2}%", name, usage);
    }

    Ok(())
}

/// Opens a PDH query on `\\PhysicalDisk(*)\\% Disk Time`,
/// does two collects (3 seconds apart), and returns usage for all disk instances.
fn get_disk_usage_after_3s() -> eyre::Result<Vec<(String, f64)>> {
    unsafe {
        // Open PDH query
        let mut query = PDH_HQUERY::default();
        interpret_pdh_error(PdhOpenQueryW(None, 0, &mut query)).wrap_err("PdhOpenQueryW failed")?;

        // Add wildcard counter
        let counter_path = w!("\\PhysicalDisk(*)\\% Disk Time");
        let mut counter = PDH_HCOUNTER::default();
        interpret_pdh_error(PdhAddCounterW(query, counter_path, 0, &mut counter))
            .wrap_err("PdhAddCounterW failed")?;

        // First collect => baseline
        interpret_pdh_error(PdhCollectQueryData(query))
            .wrap_err("First PdhCollectQueryData failed")?;

        // Wait 3 seconds
        sleep(Duration::from_secs(3));

        // Second collect => “live” sample
        interpret_pdh_error(PdhCollectQueryData(query))
            .wrap_err("Second PdhCollectQueryData failed")?;

        // We want to retrieve all instance values, so we use PdhGetFormattedCounterArrayW.
        let mut buffer_size: u32 = 0;
        let mut item_count: u32 = 0;

        // First call: pass null buffer to get PDH_MORE_DATA and the needed sizes.
        let mut status = PdhGetFormattedCounterArrayW(
            counter,
            PDH_FMT_DOUBLE,
            &mut buffer_size,
            &mut item_count,
            None,
        );

        // PDH_MORE_DATA means "OK, but you need a bigger buffer"
        if status != PDH_MORE_DATA {
            interpret_pdh_error(status)?;
        }

        // Allocate enough room for 'item_count' items
        let mut items: Vec<PDH_FMT_COUNTERVALUE_ITEM_W> = Vec::with_capacity(item_count as usize);

        // Second call: supply the actual buffer
        status = PdhGetFormattedCounterArrayW(
            counter,
            PDH_FMT_DOUBLE,
            &mut buffer_size,
            &mut item_count,
            Some(items.as_mut_ptr()),
        );
        interpret_pdh_error(status).wrap_err("PdhGetFormattedCounterArrayW failed")?;

        // Now that call has written `item_count` items into items[]. So we do:
        items.set_len(item_count as usize);

        // Convert them to f64 percentages
        let mut results = Vec::with_capacity(item_count as usize);
        for item in &items {
            // `FmtValue.Anonymous.doubleValue` is our usage percentage
            let usage = item.FmtValue.Anonymous.doubleValue;
            let name = item.szName.to_string()?;
            results.push((name, usage));
        }

        // Clean up
        interpret_pdh_error(PdhCloseQuery(query)).wrap_err("PdhCloseQuery failed")?;

        Ok(results)
    }
}
