use clap::CommandFactory;
use clap::FromArgMatches;
use storage_usage_v2::cli::Cli;
use storage_usage_v2::init_tracing::init_tracing;

fn main() -> eyre::Result<()> {
    color_eyre::install()?;
    let cli = Cli::command();
    let cli = Cli::from_arg_matches(&cli.get_matches())?;
    init_tracing(cli.global_args.log_level());

    cli.run()?;
    Ok(())
}
