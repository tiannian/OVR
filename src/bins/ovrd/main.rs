use clap::{Parser, Subcommand};
use ovr::DaemonCfg;
use ruc::*;

mod daemon;
mod pack;

#[derive(Parser, Debug)]
#[clap(about, version, author)]
pub struct Cfg {
    #[clap(subcommand)]
    pub commands: Commands,
}

#[derive(Debug, Subcommand)]
#[allow(clippy::large_enum_variant)]
pub enum Commands {
    #[clap(about = "Run ovr in daemon mode")]
    Daemon(DaemonCfg),
    #[clap(about = "Pack tendermint into the ovrd binary")]
    Pack,
    #[clap(about = "Unpack tendermint from the ovrd binary")]
    Unpack,
}

fn main() {
    let config = Cfg::parse();

    match config.commands {
        Commands::Daemon(cfg) => {
            pnk!(pack::unpack());
            pnk!(daemon::exec(cfg));
        }
        Commands::Pack => {
            pnk!(pack::pack());
        }
        Commands::Unpack => {
            pnk!(pack::unpack());
        }
    }
}
