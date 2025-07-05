use clap::CommandFactory;
use clap::FromArgMatches;
use storage_usage_v2::cli::Cli;
use storage_usage_v2::console_reuse::reuse_console_if_requested;
use storage_usage_v2::init_tracing::init_tracing;

fn main() -> eyre::Result<()> {
    color_eyre::install()?;
    let cli = Cli::command();
    let cli = Cli::from_arg_matches(&cli.get_matches())?;

    reuse_console_if_requested(&cli.global_args);
    init_tracing(cli.global_args.log_level());

    cli.run()?;
    Ok(())
}
