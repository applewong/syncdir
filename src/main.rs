use dirsync::client::client_main;
use dirsync::server::server_main;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    /// Turn debugging information on
    #[arg(short, long, action = clap::ArgAction::Count)]
    debug: u8,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    Sync {
        #[arg(short, long, value_name = "SERVER")]
        server: String,

        #[arg(short, long, default_value_t = String::from("."), value_name = "DIR")]
        dir: String,

        #[arg(long, default_value_t = String::from("friday"), value_name = "AUTH_KEY")]
        auth_key: String,

        #[arg(long, default_value_t = false, value_name = "DRY RUN")]
        dry_run: bool,

        #[arg(short, long, default_value_t = false, value_name = "VERBOSE")]
        verbose: bool,
    },
    Server {
        #[arg(short, long, default_value_t = String::from(":9022"), value_name = "SERVER")]
        listen: String,

        #[arg(short, long, default_value_t = String::from("."), value_name = "DIR")]
        dir: String,

        #[arg(long, default_value_t = String::from("friday"), value_name = "AUTH_KEY")]
        auth_key: String,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Sync {
            server,
            dir,
            auth_key,
            dry_run,
            verbose,
        }) => client_main(&server, &dir, &auth_key, dry_run, verbose).unwrap(),
        Some(Commands::Server {
            listen,
            dir,
            auth_key,
        }) => {
            server_main(&listen, &dir, &auth_key).unwrap();
        }
        None => {
            println!("no command");
        }
    }
}
