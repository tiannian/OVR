use clap::Parser;
use ovr::{Cfg, Commands};
use ruc::*;

mod btm;
mod client;
mod daemon;
mod dev;

fn main() {
    let cfg = Cfg::parse();

    match cfg.commands {
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
        Commands::Btm(_cfg) => {
            todo!()
        }
    }
}
