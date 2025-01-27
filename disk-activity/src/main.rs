pub mod pdh_error;

use clap::Parser;
use eyre::bail;
use pdh_error::interpret_pdh_error;
use windows::core::w;
use windows::Win32::Foundation::ERROR_SUCCESS;
use windows::Win32::System::Performance::PdhAddCounterW;
use windows::Win32::System::Performance::PdhCollectQueryData;
use windows::Win32::System::Performance::PdhGetFormattedCounterValue;
use windows::Win32::System::Performance::PdhOpenQueryW;
use windows::Win32::System::Performance::PDH_FMT_COUNTERVALUE;
use windows::Win32::System::Performance::PDH_FMT_DOUBLE;
use windows::Win32::System::Performance::PDH_HCOUNTER;
use windows::Win32::System::Performance::PDH_HQUERY;
use windows::Win32::System::Performance::PDH_INVALID_HANDLE;
use windows::Win32::System::Performance::PDH_NO_DATA;

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
    match get_disk_active_time() {
        Ok(value) => println!("Disk active time: {:.2}%", value),
        Err(e) => eprintln!("Error: {}", e),
    }
    Ok(())
}

fn get_disk_active_time() -> eyre::Result<f32> {
    unsafe {
        // Open a PDH query
        let mut query = PDH_HQUERY::default();
        interpret_pdh_error(PdhOpenQueryW(None, 0, &mut query))?;

        // Add the counter for % Disk Time
        let counter_path = w!("\\PhysicalDisk(*)\\% Disk Time");
        let mut counter = PDH_HCOUNTER::default();

        // https://learn.microsoft.com/en-us/windows/win32/api/pdh/nf-pdh-pdhaddcounterw
        interpret_pdh_error(PdhAddCounterW(query, counter_path, 0, &mut counter))?;

        // Collect data
        // https://learn.microsoft.com/en-us/windows/win32/api/pdh/nf-pdh-pdhcollectquerydata
        match PdhCollectQueryData(query) {
            x if x == ERROR_SUCCESS.0 => {}
            PDH_INVALID_HANDLE => bail!("The query handle is not valid."),
            PDH_NO_DATA => bail!("No data is available."),
            x => unreachable!("Unexpected error: {x:?}"),
        }

        // Get formatted counter value
        let mut value = PDH_FMT_COUNTERVALUE::default();
        // https://learn.microsoft.com/en-us/windows/win32/api/pdh/nf-pdh-pdhgetformattedcountervalue
        interpret_pdh_error(PdhGetFormattedCounterValue(
            counter,
            PDH_FMT_DOUBLE,
            None,
            &mut value,
        ))?;

        Ok(value.Anonymous.doubleValue as f32)
    }
}
