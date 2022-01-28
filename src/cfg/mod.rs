use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[clap(about, version, author)]
pub struct Cfg {
    #[clap(
        short = 'a',
        long,
        help = "Addresses served by the daemon, defalt to '[::]' and '0.0.0.0'"
    )]
    serv_addr_list: Option<Vec<String>>,
    #[clap(
        short = 'h',
        long,
        help = "A port used for http service, default to 30000"
    )]
    serv_http_port: Option<u16>,
    #[clap(
        short = 'w',
        long,
        help = "A port used for websocket service, default to 30001"
    )]
    serv_ws_port: Option<u16>,
    #[clap(
        short = 'm',
        long,
        help = "An udp port used for system managements, default to 9527"
    )]
    serv_mgmt_port: Option<u16>,

    #[clap(subcommand)]
    pub commands: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    #[clap(about = "Run ovr in client mode, default option")]
    Client,
    #[clap(about = "Run ovr in daemon mode, aka run a node")]
    Daemon(DaemonCfg),
    #[clap(about = "Use debug utils, eg, create a local env")]
    Debug(DebugCfg),
}

#[derive(Clone, Debug, Parser)]
pub struct DaemonCfg {
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
        help = "A path where all data will be stored in, default to '~/.vsdb'"
    )]
    pub(crate) vsdb_base_dir: Option<String>,
    #[clap(long, help = "A field for EIP1559")]
    pub(crate) block_base_fee_per_gas: Option<u128>,
}

#[derive(Parser, Debug)]
pub struct DebugCfg {
    #[clap(short, long)]
    pub(crate) env_name: u64,
}
