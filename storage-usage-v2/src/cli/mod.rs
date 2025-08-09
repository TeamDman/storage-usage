use crate::cli::action::Action;
use crate::cli::global_args::GlobalArgs;
use crate::to_args::Invocable;
use crate::to_args::ToArgs;
use arbitrary::Arbitrary;
use clap::Parser;
use std::ffi::OsString;

pub mod action;
pub mod config_action;
pub mod drive_letter_pattern;
pub mod elevation_action;
pub mod elevation_check_action;
pub mod elevation_test_action;
pub mod global_args;
pub mod mft_action;
pub mod mft_diff_action;
pub mod mft_dump_action;
pub mod mft_query_action;
pub mod mft_show_action;
pub mod mft_sync_action;

#[derive(Parser, Arbitrary, PartialEq, Debug)]
#[clap(version)]
pub struct Cli {
    #[clap(flatten)]
    pub global_args: GlobalArgs,
    #[clap(subcommand)]
    pub action: Action,
}

impl Cli {
    pub fn run(self) -> eyre::Result<()> {
        self.action.run()
    }
}

impl ToArgs for Cli {
    fn to_args(&self) -> Vec<OsString> {
        let mut args = Vec::new();
        args.extend(self.global_args.to_args());
        args.extend(self.action.to_args());
        args
    }
}

impl Invocable for Cli {
    fn executable(&self) -> std::path::PathBuf {
        std::env::current_exe().expect("Failed to get current executable path")
    }

    fn args(&self) -> Vec<OsString> {
        self.to_args()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arbitrary::Arbitrary;
    use clap::Parser;

    #[test]
    fn fuzz_cli_args_roundtrip() {
        // Generate 100 arbitrary CLI instances and test roundtrip conversion
        let mut data = vec![42u8; 1024]; // Create owned data
        let mut rng = arbitrary::Unstructured::new(&data);

        for i in 0..100 {
            // Generate an arbitrary CLI instance
            let cli = match Cli::arbitrary(&mut rng) {
                Ok(cli) => cli,
                Err(_) => {
                    // If we run out of data, refresh with new seed
                    data = vec![i as u8; 1024];
                    rng = arbitrary::Unstructured::new(&data);
                    Cli::arbitrary(&mut rng).expect("Failed to generate CLI instance")
                }
            };

            // Convert CLI to args
            let args = cli.to_args();

            // Create command line with executable name
            let mut full_args = vec!["test-exe".into()];
            full_args.extend(args);

            // Parse back from args
            let parsed_cli = match Cli::try_parse_from(&full_args) {
                Ok(parsed) => parsed,
                Err(e) => {
                    panic!(
                        "Failed to parse CLI args on iteration {}: {}\nOriginal CLI: {:?}\nArgs: {:?}",
                        i, e, cli, full_args
                    );
                }
            };

            // Check equality
            if cli != parsed_cli {
                panic!(
                    "CLI roundtrip failed on iteration {}:\nOriginal: {:?}\nParsed: {:?}\nArgs: {:?}",
                    i, cli, parsed_cli, full_args
                );
            }
        }
    }

    #[test]
    fn fuzz_cli_args_consistency() {
        // Test that the same CLI instance always produces the same args
        let mut data = vec![123u8; 1024]; // Create owned data
        let mut rng = arbitrary::Unstructured::new(&data);

        for i in 0..50 {
            let cli = match Cli::arbitrary(&mut rng) {
                Ok(cli) => cli,
                Err(_) => {
                    data = vec![(i * 2) as u8; 1024];
                    rng = arbitrary::Unstructured::new(&data);
                    Cli::arbitrary(&mut rng).expect("Failed to generate CLI instance")
                }
            };

            let args1 = cli.to_args();
            let args2 = cli.to_args();

            assert_eq!(
                args1, args2,
                "CLI.to_args() should be deterministic for iteration {}",
                i
            );
        }
    }

    #[test]
    fn test_specific_cli_cases() {
        // Test specific cases that should work
        use crate::cli::action::Action;
        use crate::cli::elevation_action::ElevationAction;
        use crate::cli::elevation_action::ElevationArgs;
        use crate::cli::elevation_check_action::ElevationCheckArgs;
        use crate::cli::elevation_test_action::ElevationTestArgs;
        use crate::cli::global_args::GlobalArgs;
        use crate::cli::mft_action::MftAction;
        use crate::cli::mft_action::MftArgs;
        use crate::cli::mft_dump_action::MftDumpArgs;

        let test_cases = vec![
            Cli {
                global_args: GlobalArgs {
                    debug: false,
                    console_pid: None,
                },
                action: Action::Mft(MftArgs {
                    action: MftAction::Dump(MftDumpArgs {
                        drive_letters: "C".to_string(),
                        output_path: "test_output.bin".into(),
                        overwrite_existing: false,
                    }),
                }),
            },
            Cli {
                global_args: GlobalArgs {
                    debug: true,
                    console_pid: Some(1234),
                },
                action: Action::Mft(MftArgs {
                    action: MftAction::Dump(MftDumpArgs {
                        drive_letters: "D".to_string(),
                        output_path: "another_output.bin".into(),
                        overwrite_existing: true,
                    }),
                }),
            },
            Cli {
                global_args: GlobalArgs {
                    debug: false,
                    console_pid: None,
                },
                action: Action::Elevation(ElevationArgs {
                    action: ElevationAction::Check(ElevationCheckArgs {}),
                }),
            },
            Cli {
                global_args: GlobalArgs {
                    debug: true,
                    console_pid: Some(5678),
                },
                action: Action::Elevation(ElevationArgs {
                    action: ElevationAction::Test(ElevationTestArgs {}),
                }),
            },
        ];

        for (i, cli) in test_cases.into_iter().enumerate() {
            // Convert CLI to args
            let args = cli.to_args();

            // Create command line with executable name
            let mut full_args = vec!["test-exe".into()];
            full_args.extend(args);

            // Parse back from args
            let parsed_cli = Cli::try_parse_from(&full_args)
                .unwrap_or_else(|e| panic!("Failed to parse CLI args for test case {}: {}", i, e));

            // Check equality
            assert_eq!(cli, parsed_cli, "CLI roundtrip failed for test case {}", i);
        }
    }
}
