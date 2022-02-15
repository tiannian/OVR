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
        Commands::Daemon(cfg) => {
            pnk!(daemon::start(cfg));
        }
        Commands::Client(_cfg) => {
            todo!()
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
