use dirsync::client::client_main;
use dirsync::server::server_main;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Turn debugging information on
    #[arg(short, long, action = clap::ArgAction::Count)]
    debug: u8,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    Sync {
        #[arg(short, long, value_name = "SERVER")]
        server: String,

        #[arg(short, long, default_value_t = String::from("."), value_name = "DIR")]
        dir: String,

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
    },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Sync {
            server,
            dir,
            dry_run,
            verbose,
        }) => {
            client_main(server.as_str(), dir.as_str(), dry_run, verbose)
                .await
                .unwrap();
        }
        Some(Commands::Server { listen, dir }) => {
            server_main(listen.as_str(), dir.as_str()).await.unwrap();
        }
        None => {
            println!("no command");
        }
    }
}
