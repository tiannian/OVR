use abci::ServerBuilder;
use ovr::{cfg::DaemonCfg, App};
use ruc::*;
use std::net::SocketAddr;

const MB: usize = 1 << 20;
const BUF_SIZ: usize = 128 * MB;

pub fn start(cfg: DaemonCfg) -> Result<()> {
    let app = App::load_or_create(cfg).c(d!())?;

    start_web3_http_server(&app).c(d!())?;
    start_web3_http_ws(&app).c(d!())?;

    let addr_list = app
        .cfg
        .serv_addr_list
        .split(',')
        .map(|addr| {
            format!("{}:{}", addr, app.cfg.serv_abci_port)
                .parse::<SocketAddr>()
                .c(d!())
        })
        .collect::<Result<Vec<_>>>()?;
    if addr_list.is_empty() {
        return Err(eg!("no listening address found"));
    }

    let s = ServerBuilder::new(BUF_SIZ);
    let s = s.bind(addr_list.as_slice(), app).c(d!())?;
    s.listen().c(d!())
}

fn start_web3_http_server(_app: &App) -> Result<()> {
    // TODO
    Ok(())
}

fn start_web3_http_ws(_app: &App) -> Result<()> {
    // TODO
    Ok(())
}
