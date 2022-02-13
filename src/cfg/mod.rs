use clap::{Parser, Subcommand};

#[cfg(target_os = "linux")]
use {
    crate::common::BlockHeight,
    btm::{BtmCfg as BtmSysCfg, SnapAlgo, SnapMode, ENV_VAR_BTM_VOLUME},
    ruc::*,
    std::env,
};

#[derive(Parser, Debug)]
#[clap(about, version, author)]
pub struct Cfg {
    #[clap(subcommand)]
    pub commands: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    #[clap(about = "Run ovr in daemon mode, aka run a node")]
    Daemon(DaemonCfg),
    #[clap(about = "Run ovr in client mode, default option")]
    Client(ClientCfg),
    #[clap(about = "Development utils, create a local env, .etc")]
    Dev(DevCfg),
    #[cfg(target_os = "linux")]
    #[clap(about = "BTM related operations")]
    Btm(BtmCfg),
}

#[derive(Clone, Debug, Parser)]
pub struct DaemonCfg {
    #[clap(
        long,
        default_value_t = 9527,
        help = "The ID of your chain, an unsigned integer"
    )]
    pub(crate) chain_id: u64,
    #[clap(
        long,
        default_value_t = String::from("NULL"),
        help = "A custom name of your chain"
    )]
    pub(crate) chain_name: String,
    #[clap(
        long,
        default_value_t = String::from("NULL"),
        help = "A custom version of your chain"
    )]
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

    #[clap(
        short = 'A',
        long,
        default_value_t = ["[::]", "0.0.0.0"].join(","),
        help = "Addresses served by the daemon, seperated by ','"
    )]
    pub serv_addr_list: String,
    #[clap(
        short = 'p',
        long,
        default_value_t = 30000,
        help = "A port used for http service"
    )]
    pub serv_http_port: u16,
    #[clap(
        short = 'w',
        long,
        default_value_t = 30001,
        help = "A port used for websocket service"
    )]
    pub serv_ws_port: u16,
    #[clap(
        short = 'a',
        long,
        default_value_t = 26658,
        help = "the listening port of tendermint ABCI process(embed in ovr)"
    )]
    pub serv_abci_port: u16,
    #[clap(
        short = 'r',
        long,
        default_value_t = 26657,
        help = "the listening port of tendermint RPC(embed in tendermint)"
    )]
    pub tm_rpc_port: u16,

    #[cfg(target_os = "linux")]
    #[clap(long, help = "Global switch of btm functions")]
    pub btm_enable: bool,

    #[cfg(target_os = "linux")]
    #[clap(
        short = 'P',
        long,
        help = "Will try to use ${ENV_VAR_BTM_VOLUME} if missing"
    )]
    pub btm_volume: Option<String>,

    #[cfg(target_os = "linux")]
    #[clap(
        short = 'M',
        long,
        help = "Will try to detect the local system if missing"
    )]
    pub btm_mode: Option<SnapMode>,

    #[cfg(target_os = "linux")]
    #[clap(long, default_value_t = SnapAlgo::Fair)]
    pub btm_algo: SnapAlgo,

    #[cfg(target_os = "linux")]
    #[clap(short = 'I', long, default_value_t = 10)]
    pub btm_itv: u64,

    #[cfg(target_os = "linux")]
    #[clap(short = 'C', long, default_value_t = 100)]
    pub btm_cap: u64,
}

#[cfg(target_os = "linux")]
impl DaemonCfg {
    #[inline(always)]
    pub(crate) fn snapshot(&self, height: BlockHeight) -> Result<()> {
        BtmSysCfg::try_from(self).c(d!())?.snapshot(height).c(d!())
    }
}

#[derive(Debug, Parser)]
pub struct ClientCfg {
    #[clap(
        short = 'A',
        long,
        default_value_t = String::from("localhost"),
        help = "Addresses served by the server end, defalt to 'localhost'"
    )]
    pub serv_addr: String,
    #[clap(
        short = 'p',
        long,
        default_value_t = 30000,
        help = "A port used for http service"
    )]
    pub serv_http_port: u16,
    #[clap(
        short = 'w',
        long,
        default_value_t = 30001,
        help = "A port used for websocket service"
    )]
    pub serv_ws_port: u16,
}

#[derive(Debug, Parser)]
pub struct DevCfg {
    #[clap(short = 'n', long)]
    pub env_name: Option<String>,
    #[clap(short = 'c', long)]
    pub env_create: bool,
    #[clap(short = 'd', long)]
    pub env_destroy: bool,
    #[clap(short = 's', long)]
    pub env_start: bool,
    #[clap(short = 'S', long)]
    pub env_stop: bool,
    #[clap(short = 'a', long)]
    pub env_add_node: bool,
    #[clap(short = 'r', long)]
    pub env_rm_node: bool,
    #[clap(short = 'i', long)]
    pub env_info: bool,
}

#[cfg(target_os = "linux")]
#[derive(Debug, Parser)]
pub struct BtmCfg {
    #[clap(subcommand)]
    commands: BtmOps,
}

#[cfg(target_os = "linux")]
#[derive(Debug, Subcommand)]
enum BtmOps {
    #[clap(about = "Rollback to a custom historical snapshot")]
    Rollback(BtmRollbackArgs),
    #[clap(about = "Clean up all existing snapshots")]
    Clean(BtmCleanArgs),
    #[clap(about = "List all existing snapshots")]
    List(BtmListArgs),
}

#[cfg(target_os = "linux")]
#[derive(Parser, Debug)]
struct BtmRollbackArgs {
    #[clap(
        short = 'P',
        long,
        help = "Will try to use ${ENV_VAR_BTM_VOLUME} if missing"
    )]
    pub volume: Option<String>,

    #[clap(
        short = 'H',
        long,
        help = "Will try to use the latest existing height if missing"
    )]
    pub height: Option<u64>,

    #[clap(
        short = 'X',
        long,
        help = "If specified, a snapshot must exist at the 'height'"
    )]
    pub exact: bool,

    #[clap(
        short = 'M',
        long,
        help = "Will try to detect the local system if missing"
    )]
    pub mode: Option<SnapMode>,
}

#[cfg(target_os = "linux")]
#[derive(Parser, Debug)]
struct BtmCleanArgs {
    #[clap(
        short = 'P',
        long,
        help = "Will try to use ${ENV_VAR_BTM_VOLUME} if missing"
    )]
    pub volume: Option<String>,

    #[clap(
        short = 'M',
        long,
        help = "Will try to detect the local system if missing"
    )]
    pub mode: Option<SnapMode>,
}

#[cfg(target_os = "linux")]
type BtmListArgs = BtmCleanArgs;

#[cfg(target_os = "linux")]
impl TryFrom<&DaemonCfg> for BtmSysCfg {
    type Error = Box<dyn RucError>;
    fn try_from(dc: &DaemonCfg) -> Result<Self> {
        let mut res = Self {
            enable: dc.btm_enable,
            itv: dc.btm_itv,
            cap: dc.btm_cap,
            mode: SnapMode::default(),
            algo: dc.btm_algo,
            volume: dc
                .btm_volume
                .clone()
                .c(d!())
                .or_else(|_| env::var(ENV_VAR_BTM_VOLUME).c(d!()))?,
        };

        res.mode = dc.btm_mode.c(d!()).or_else(|e| res.guess_mode().c(d!(e)))?;

        Ok(res)
    }
}
