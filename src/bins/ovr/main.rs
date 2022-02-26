use clap::Parser;
use ovr::{Cfg, Commands};
use ruc::*;

mod client;
mod daemon;
mod dev;

#[cfg(target_os = "linux")]
mod snapshot;

fn main() {
    let config = Cfg::parse();

    match config.commands {
        Commands::Cli(cfg) => {
            pnk!(client::exec(cfg));
        }
        Commands::Daemon(cfg) => {
            pnk!(daemon::exec(cfg));
        }
        Commands::Dev(cfg) => {
            pnk!(dev::EnvCfg::from(cfg).exec());
        }

        #[cfg(target_os = "linux")]
        Commands::Snap(cfg) => {
            pnk!(snapshot::exec(cfg));
        }
    }
}
