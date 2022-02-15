use abci::ServerBuilder;
use ovr::{
    cfg::DaemonCfg,
    rpc::{self, HttpServer, WsServer},
    App,
};
use ruc::*;
use std::net::SocketAddr;

const MB: usize = 1 << 20;
const BUF_SIZ: usize = 128 * MB;

pub fn start(cfg: DaemonCfg) -> Result<()> {
    let app = App::load_or_create(cfg).c(d!())?;

    start_web3_service(&app).c(d!())?;

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

fn start_web3_service(app: &App) -> Result<(Vec<HttpServer>, Vec<WsServer>)> {
    let (http_serv_list, ws_serv_list): (Vec<_>, Vec<_>) = app
        .cfg
        .serv_addr_list
        .split(',')
        .map(|addr| {
            (
                format!("{}:{}", addr, app.cfg.serv_http_port),
                format!("{}:{}", addr, app.cfg.serv_ws_port),
            )
        })
        .unzip();

    let http_serv_list = http_serv_list
        .into_iter()
        .map(|addr| addr.parse::<SocketAddr>().c(d!()))
        .collect::<Result<Vec<_>>>()?;
    let ws_serv_list = ws_serv_list
        .iter()
        .map(|addr| addr.parse::<SocketAddr>().c(d!()))
        .collect::<Result<Vec<_>>>()?;

    rpc::start_web3_service(&http_serv_list, &ws_serv_list).c(d!())
}
