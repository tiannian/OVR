use clap::Parser;
use ovr::{App, Cfg, Commands};
use ruc::*;

fn main() {
    let cfg = Cfg::parse();

    match cfg.commands {
        Commands::Daemon(cfg) => {
            let _app = pnk!(App::load_or_create(cfg));
        }
        Commands::Client(_cfg) => {
            todo!()
        }
        Commands::Debug(_cfg) => {
            todo!()
        }
    }
}
