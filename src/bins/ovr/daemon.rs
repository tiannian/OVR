use abci::ServerBuilder;
use ovr::{cfg::DaemonCfg, rpc::Web3ServerBuilder, App};
use ruc::*;
use std::net::SocketAddr;

const MB: usize = 1 << 20;
const BUF_SIZ: usize = 128 * MB;

pub fn exec(cfg: Box<DaemonCfg>) -> Result<()> {
    let cfg = cfg.as_ref().clone();
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

fn start_web3_service(app: &App) -> Result<()> {
    let ((http_serv_list, ws_serv_list), td_addr_list): ((Vec<_>, Vec<_>), Vec<_>) = app
        .cfg
        .serv_addr_list
        .split(',')
        .map(|addr| {
            (
                (
                    format!("{}:{}", addr, app.cfg.serv_http_port),
                    format!("{}:{}", addr, app.cfg.serv_ws_port),
                ),
                format!("{}:{}", addr, app.cfg.tendermint_rpc_port),
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
    let td_addr_list = td_addr_list
        .iter()
        .map(|addr| addr.parse::<SocketAddr>().c(d!()))
        .collect::<Result<Vec<_>>>()?;

    let builder = Web3ServerBuilder {
        upstream: td_addr_list,
        http: http_serv_list,
        ws: ws_serv_list,
        state: app.ledger.state.clone(),
    };

    let server = builder.build();

    server.start();
    Ok(())
}
