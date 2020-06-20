use clap::{crate_version, App, AppSettings, Arg, ArgMatches, SubCommand};

fn subcommand() -> SubCommand {
    let cmd_create = SubCommand::with_name("slf")
        .about("pack or unpack slf files")
        .subcommand(
            SubCommand::with_name("pack")
                .about("pack slf files")
                .arg(
                    Arg::with_name("directories")
                        .help("Which directories to pack")
                        .long("ja2-asset-tool will create one slf file per directory")
                        .takes_value(true)
                        .multiple(true)
                        .required(true)
            )
        )
        .arg(
            Arg::with_name("directory")
                .help("Manually specify a directory to scan")
                .long("directory")
                .takes_value(true)
                .required(true),
        );
}