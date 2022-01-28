use clap::Parser;
use ovr::{App, Cfg, Commands};
use ruc::*;

fn main() {
    let cfg = Cfg::parse();

    if let Commands::Daemon(cfg) = cfg.commands {
        let _app = pnk!(App::new(cfg));
    } else {
        // TODO
        // client operations
    }
}
