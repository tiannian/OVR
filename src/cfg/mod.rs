use clap::{Parser, Subcommand};

#[cfg(target_os = "linux")]
use btm::BtmCfg;

#[derive(Parser, Debug)]
#[clap(about, version, author)]
pub struct Cfg {
    #[clap(subcommand)]
    pub commands: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    #[clap(about = "Run ovr in daemon mode, aka run a node")]
    Daemon(DaemonCfg),
    #[clap(about = "Run ovr in client mode, default option")]
    Client(ClientCfg),
    #[clap(about = "Use debug utils, eg, create a local env")]
    Debug(DebugCfg),
}

#[derive(Clone, Debug, Parser)]
pub struct DaemonCfg {
    #[clap(
        short = 'a',
        long,
        default_value_t = ["[::]", "0.0.0.0"].join(","),
        help = "Addresses served by the daemon, seperated by ','"
    )]
    serv_addr_list: String,
    #[clap(
        short = 'p',
        long,
        default_value_t = 30000,
        help = "A port used for http service"
    )]
    serv_http_port: u16,
    #[clap(
        short = 'w',
        long,
        default_value_t = 30001,
        help = "A port used for websocket service"
    )]
    serv_ws_port: u16,
    #[clap(
        short = 'm',
        long,
        default_value_t = 9527,
        help = "An UDP port used for system managements"
    )]
    serv_mgmt_port: u16,

    #[clap(long, help = "The ID of your chain, an unsigned integer")]
    pub(crate) chain_id: u64,
    #[clap(long, help = "A custom name of your chain")]
    pub(crate) chain_name: String,
    #[clap(long, help = "A custom version of your chain")]
    pub(crate) chain_version: String,
    #[clap(long, help = "Basic gas price of the evm transactions")]
    pub(crate) gas_price: Option<u128>,
    #[clap(long, help = "The limitation of the total gas of any block")]
    pub(crate) block_gas_limit: Option<u128>,
    #[clap(
        short = 'd',
        long,
        help = "A path where all data will be stored in [default: ~/.vsdb]"
    )]
    pub(crate) vsdb_base_dir: Option<String>,
    #[clap(long, help = "A field for EIP1559")]
    pub(crate) block_base_fee_per_gas: Option<u128>,
}

#[derive(Parser, Debug)]
pub struct ClientCfg {
    #[clap(
        short = 'a',
        long,
        default_value_t = String::from("localhost"),
        help = "Addresses served by the server end, defalt to 'localhost'"
    )]
    serv_addr: String,
    #[clap(
        short = 'p',
        long,
        default_value_t = 30000,
        help = "A port used for http service"
    )]
    serv_http_port: u16,
    #[clap(
        short = 'w',
        long,
        default_value_t = 30001,
        help = "A port used for websocket service"
    )]
    serv_ws_port: u16,
    #[clap(
        short = 'm',
        long,
        default_value_t = 9527,
        help = "An UDP port used for system managements"
    )]
    serv_mgmt_port: u16,
}

#[derive(Parser, Debug)]
pub struct DebugCfg {
    #[clap(short, long)]
    pub(crate) env_name: u64,
}
