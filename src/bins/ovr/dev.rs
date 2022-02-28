//!
//! `ovr dev` SubCommand
//!
//! - make sure all names and ports are unique
//!     - keep a meta file in ${ENV_BASE_DIR}
//! - write ports to the running-dir of every env
//!

use nix::{
    sys::socket::{
        bind, setsockopt, socket, sockopt, AddressFamily, InetAddr, IpAddr, SockAddr,
        SockFlag, SockType,
    },
    unistd::{close, fork, ForkResult},
};
use ovr::{cfg::DevCfg, DECIMAL};
use primitive_types::{H160, U256};
use ruc::{cmd, *};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::ErrorKind,
    path::PathBuf,
    process::{exit, Command, Stdio},
    str::FromStr,
};
use tendermint::{validator::Info as TmValidator, vote::Power as TmPower};
use tendermint_config::{
    PrivValidatorKey as TmValidatorKey, TendermintConfig as TmConfig,
};
use toml_edit::{value as toml_value, Document};

type NodeId = u32;

const ENV_BASE_DIR: &str = "/tmp/__OVR_DEV__";
const ENV_NAME_DEFAULT: &str = "default";
const INIT_POWER: u32 = 1_0000_0000;

#[derive(Default)]
pub struct EnvCfg {
    // the name of this env
    name: String,

    // which operation to trigger
    ops: Ops,

    // seconds between two blocks
    block_itv_secs: u8,

    // how many validator nodes should be created
    validator_num: u8,
}

impl From<DevCfg> for EnvCfg {
    fn from(cfg: DevCfg) -> Self {
        let ops = if cfg.env_create {
            Ops::Create
        } else if cfg.env_destroy {
            Ops::Destroy
        } else if cfg.env_start {
            Ops::Start
        } else if cfg.env_stop {
            Ops::Stop
        } else if cfg.env_add_node {
            Ops::AddNode
        } else if cfg.env_rm_node {
            Ops::DelNode
        } else {
            Ops::default()
        };

        Self {
            name: cfg.env_name.unwrap_or_else(|| ENV_NAME_DEFAULT.to_owned()),
            ops,
            block_itv_secs: cfg.block_itv_secs,
            validator_num: cfg.validator_num,
        }
    }
}

impl EnvCfg {
    pub fn exec(&self) -> Result<Option<Env>> {
        match self.ops {
            Ops::Create => Env::create(self).c(d!()).map(Some),
            Ops::Destroy => Env::load_cfg(self)
                .c(d!())
                .and_then(|env| env.destroy().c(d!()))
                .map(|_| None),
            Ops::Start => Env::load_cfg(self)
                .c(d!())
                .and_then(|mut env| env.start(None).c(d!()))
                .map(|_| None),
            Ops::Stop => Env::load_cfg(self)
                .c(d!())
                .and_then(|env| env.stop().c(d!()))
                .map(|_| None),
            Ops::AddNode => Env::load_cfg(self)
                .c(d!())
                .and_then(|mut env| env.attach_node().c(d!()))
                .map(|_| None),
            Ops::DelNode => Env::load_cfg(self)
                .c(d!())
                .and_then(|mut env| env.kick_node().c(d!()))
                .map(|_| None),
            Ops::Info => Env::load_cfg(self).c(d!()).map(|env| {
                env.print_info();
                Some(env)
            }),
        }
    }
}

#[derive(Default, Debug, Clone, Deserialize, Serialize)]
pub struct Env {
    // the name of this env
    name: String,
    // data path of this env
    home: String,
    // the contents of `genesis.json` of all nodes
    genesis: Vec<u8>,

    token_distribution: TokenDistribution,

    seed_nodes: BTreeMap<NodeId, Node>,
    full_nodes: BTreeMap<NodeId, Node>,
    validator_nodes: BTreeMap<NodeId, Node>,

    // the latest/max id of current nodes
    latest_id: NodeId,

    // seconds between two blocks
    block_itv_secs: u8,
}

impl Env {
    // - initilize a new env
    // - `genesis.json` will be created
    fn create(cfg: &EnvCfg) -> Result<Env> {
        let mut env = Env {
            name: cfg.name.clone(),
            home: format!("{}/{}", ENV_BASE_DIR, &cfg.name),
            token_distribution: TokenDistribution::generate(),
            block_itv_secs: cfg.block_itv_secs,
            ..Self::default()
        };

        fs::create_dir_all(&env.home).c(d!())?;

        macro_rules! add_initial_nodes {
            ($kind: tt) => {{
                let id = env.next_node_id();
                env.alloc_resources(id, Kind::$kind).c(d!())?;
            }};
        }

        add_initial_nodes!(Seed);
        add_initial_nodes!(Full);
        for _ in 0..cfg.validator_num {
            add_initial_nodes!(Node);
        }

        env.gen_genesis()
            .c(d!())
            .and_then(|_| env.apply_genesis(None).c(d!()))
            .and_then(|_| env.start(None).c(d!()))
            .map(|_| env)
    }

    // start one or all nodes
    fn start(&mut self, n: Option<NodeId>) -> Result<()> {
        let ids = n.map(|id| vec![id]).unwrap_or_else(|| {
            self.seed_nodes
                .keys()
                .chain(self.full_nodes.keys())
                .chain(self.validator_nodes.keys())
                .copied()
                .collect()
        });

        self.update_seeds()
            .c(d!())
            .and_then(|_| self.write_cfg().c(d!()))?;

        for i in ids.iter() {
            if let Some(n) = self.validator_nodes.get_mut(i) {
                n.start().c(d!())?;
            } else if let Some(n) = self.full_nodes.get_mut(i) {
                n.start().c(d!())?;
            } else if let Some(n) = self.seed_nodes.get_mut(i) {
                n.start().c(d!())?;
            } else {
                return Err(eg!("not exist"));
            }
        }

        Ok(())
    }

    // - stop all processes
    // - release all occupied ports
    fn stop(&self) -> Result<()> {
        self.validator_nodes
            .values()
            .chain(self.full_nodes.values())
            .chain(self.seed_nodes.values())
            .map(|n| n.stop().c(d!()))
            .collect::<Result<Vec<_>>>()
            .map(|_| ())
    }

    // destroy all nodes
    // - stop all running processes
    // - delete the data of every nodes
    fn destroy(&self) -> Result<()> {
        info_omit!(self.stop());
        sleep_ms!(10);
        fs::remove_dir_all(&self.home).c(d!())
    }

    // seed nodes and full nodes are kept by system for now,
    // so only the validator nodes can be added on demand
    fn attach_node(&mut self) -> Result<()> {
        let id = self.next_node_id();
        let kind = Kind::Node;
        self.alloc_resources(id, kind)
            .c(d!())
            .and_then(|_| self.apply_genesis(Some(id)).c(d!()))
            .and_then(|_| self.start(Some(id)).c(d!()))
    }

    fn kick_node(&mut self) -> Result<()> {
        self.validator_nodes
            .keys()
            .rev()
            .copied()
            .next()
            .c(d!())
            .and_then(|k| self.validator_nodes.remove(&k).c(d!()))
            .and_then(|n| n.stop().c(d!()).and_then(|_| n.delete().c(d!())))
            .and_then(|_| self.write_cfg().c(d!()))
    }

    // 1. allocate ports
    // 2. change configs: ports, seed address, etc.
    // 3. insert new node to the meta of env
    // 4. write new configs of tendermint to disk
    fn alloc_resources(&mut self, id: NodeId, kind: Kind) -> Result<()> {
        // 1.
        let ports = alloc_ports(&kind, &self.name).c(d!())?;

        // 2.
        let home = format!("{}/{}", self.home, id);
        fs::create_dir_all(&home).c(d!())?;

        let cfg_path = format!("{}/config/config.toml", &home);
        let mut cfg = fs::read_to_string(&cfg_path)
            .c(d!())
            .or_else(|_| {
                cmd::exec_output(&format!("tendermint init validator --home {}", &home))
                    .c(d!())
                    .and_then(|_| fs::read_to_string(&cfg_path).c(d!()))
            })
            .and_then(|c| c.parse::<Document>().c(d!()))?;

        cfg["p2p"]["addr_book_strict"] = toml_value(false);
        cfg["p2p"]["allow_duplicate_ip"] = toml_value(true);
        cfg["p2p"]["pex"] = toml_value(true);
        cfg["p2p"]["persistent_peers_max_dial_period"] = toml_value("3s");

        cfg["consensus"]["timeout_propose"] = toml_value("3s");
        cfg["consensus"]["timeout_propose_delta"] = toml_value("500ms");
        cfg["consensus"]["timeout_prevote"] = toml_value("1s");
        cfg["consensus"]["timeout_prevote_delta"] = toml_value("500ms");
        cfg["consensus"]["timeout_precommit"] = toml_value("1s");
        cfg["consensus"]["timeout_precommit_delta"] = toml_value("500ms");
        cfg["consensus"]["timeout_commit"] =
            toml_value(self.block_itv_secs.to_string() + "s");
        cfg["consensus"]["create_empty_blocks"] = toml_value(true);
        cfg["consensus"]["create_empty_blocks_interval"] = toml_value("0s");

        cfg["p2p"]["laddr"] = toml_value(format!("tcp://127.0.0.1:{}", ports.tm_p2p));
        cfg["rpc"]["laddr"] = toml_value(format!("tcp://127.0.0.1:{}", ports.tm_rpc));
        cfg["proxy_app"] = toml_value(format!("tcp://127.0.0.1:{}", ports.tm_abci));
        cfg["moniker"] = toml_value(format!("{}-{}", &self.name, id));

        if matches!(kind, Kind::Seed) {
            cfg["p2p"]["seed_mode"] = toml_value(true)
        }

        // 3.
        let node = Node {
            id,
            tm_id: TmConfig::load_toml_file(&cfg_path)
                .map_err(|e| eg!(e))?
                .load_node_key(&home)
                .map_err(|e| eg!(e))?
                .node_id()
                .to_string()
                .to_lowercase(),
            home: format!("{}/{}", &self.home, id),
            kind,
            ports,
        };

        match kind {
            Kind::Node => self.validator_nodes.insert(id, node),
            Kind::Full => self.full_nodes.insert(id, node),
            Kind::Seed => self.seed_nodes.insert(id, node),
        };

        // 4.
        fs::write(cfg_path, cfg.to_string()).c(d!())
    }

    fn update_seeds(&self) -> Result<()> {
        for n in self
            .validator_nodes
            .values()
            .chain(self.full_nodes.values())
        {
            let cfg_path = format!("{}/config/config.toml", &n.home);
            let mut cfg = fs::read_to_string(&cfg_path)
                .c(d!())
                .and_then(|c| c.parse::<Document>().c(d!()))?;
            cfg["p2p"]["seeds"] = toml_value(
                self.seed_nodes
                    .values()
                    .map(|n| format!("{}@127.0.0.1:{}", &n.tm_id, n.ports.tm_p2p))
                    .collect::<Vec<_>>()
                    .join(","),
            );
            fs::write(cfg_path, cfg.to_string()).c(d!())?;
        }

        Ok(())
    }

    // allocate unique IDs for nodes within the scope of an env
    fn next_node_id(&mut self) -> NodeId {
        self.latest_id += 1;
        self.latest_id
    }

    // generate a new `genesis.json` based on the collection of validators
    fn gen_genesis(&mut self) -> Result<()> {
        let tmp_id = self.next_node_id();
        let tmp_home = format!("{}/{}", &self.home, tmp_id);

        let gen = |genesis_file: String| {
            self.validator_nodes
                .values()
                .map(|n| {
                    TmConfig::load_toml_file(&format!("{}/config/config.toml", &n.home))
                        .map_err(|e| eg!(e))
                        .and_then(|cfg| {
                            cfg.priv_validator_key_file
                                .as_ref()
                                .c(d!())
                                .and_then(|f| {
                                    PathBuf::from_str(&n.home).c(d!()).map(|p| {
                                        p.join(f).to_string_lossy().into_owned()
                                    })
                                })
                                .and_then(|p| {
                                    TmValidatorKey::load_json_file(&p)
                                        .map_err(|e| eg!(e))
                                })
                        })
                        .map(|key| {
                            TmValidator::new(key.pub_key, TmPower::from(INIT_POWER))
                        })
                })
                .collect::<Result<Vec<_>>>()
                .and_then(|vs| serde_json::to_value(&vs).c(d!()))
                .and_then(|mut vs| {
                    vs.as_array_mut().c(d!())?.iter_mut().enumerate().for_each(
                        |(i, v)| {
                            v["power"] = Value::String(INIT_POWER.to_string());
                            v["name"] = Value::String(format!("node-{}", i));
                        },
                    );
                    let app_state =
                        serde_json::to_value(&self.token_distribution.addr_to_amount)
                            .c(d!())?;
                    fs::read_to_string(format!("{}/{}", tmp_home, genesis_file))
                        .c(d!())
                        .and_then(|g| serde_json::from_str::<Value>(&g).c(d!()))
                        .map(|mut g| {
                            g["validators"] = vs;
                            g["app_state"] = app_state;
                            self.genesis = g.to_string().into_bytes();
                        })
                })
        };

        cmd::exec_output(&format!("tendermint init validator --home {}", &tmp_home))
            .c(d!())
            .and_then(|_| {
                TmConfig::load_toml_file(&format!("{}/config/config.toml", &tmp_home))
                    .map_err(|e| eg!(e))
            })
            .and_then(|cfg| cfg.genesis_file.to_str().map(|f| f.to_owned()).c(d!()))
            .and_then(gen)
            .and_then(|_| fs::remove_dir_all(tmp_home).c(d!()))
    }

    // apply genesis to all nodes in the same env
    fn apply_genesis(&mut self, n: Option<NodeId>) -> Result<()> {
        let nodes = n.map(|id| vec![id]).unwrap_or_else(|| {
            self.seed_nodes
                .keys()
                .chain(self.full_nodes.keys())
                .chain(self.validator_nodes.keys())
                .copied()
                .collect()
        });

        for n in nodes.iter() {
            self.validator_nodes
                .get(n)
                .or_else(|| self.full_nodes.get(n))
                .or_else(|| self.seed_nodes.get(n))
                .c(d!())
                .and_then(|n| {
                    TmConfig::load_toml_file(&format!("{}/config/config.toml", &n.home))
                        .map_err(|e| eg!(e))
                        .and_then(|cfg| {
                            PathBuf::from_str(&n.home)
                                .c(d!())
                                .map(|home| home.join(&cfg.genesis_file))
                        })
                        .and_then(|genesis_path| {
                            fs::write(genesis_path, &self.genesis).c(d!())
                        })
                })?;
        }

        Ok(())
    }

    fn load_cfg(cfg: &EnvCfg) -> Result<Env> {
        let p = format!("{}/{}/config.json", ENV_BASE_DIR, &cfg.name);
        fs::read_to_string(&p)
            .c(d!())
            .and_then(|d| serde_json::from_str(&d).c(d!()))
    }

    fn write_cfg(&self) -> Result<()> {
        serde_json::to_vec_pretty(self)
            .c(d!())
            .and_then(|d| fs::write(format!("{}/config.json", &self.home), d).c(d!()))
    }

    fn print_info(&self) {
        println!("Env name: {}", &self.name);
        println!("Env home: {}", &self.home);
        println!("Seed nodes: {:#?}", &self.seed_nodes);
        println!("Full nodes: {:#?}", &self.full_nodes);
        println!("Validator nodes: {:#?}", &self.validator_nodes);
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct Node {
    id: NodeId,
    tm_id: String,
    home: String,
    kind: Kind,
    ports: Ports,
}

impl Node {
    // - start node
    // - collect results
    // - update meta
    fn start(&mut self) -> Result<()> {
        match unsafe { fork() } {
            Ok(ForkResult::Child) => {
                let cmd = format!(
                    r"
                    ovr daemon -a {1} -T {2} -p {3} -w {4} -d {5} >{0}/app.log 2>&1 & \
                    tendermint node --home {0} >{0}/tendermint.log 2>&1
                    ",
                    &self.home,
                    self.ports.tm_abci,
                    self.ports.tm_rpc,
                    self.ports.web3_http,
                    self.ports.web3_ws,
                    self.vsdb_base_dir(),
                );
                pnk!(exec_spawn(&cmd));
                exit(0);
            }
            Ok(_) => Ok(()),
            Err(_) => Err(eg!("fork failed!")),
        }
    }

    fn vsdb_base_dir(&self) -> String {
        format!("{}/__vsdb__", &self.home)
    }

    fn stop(&self) -> Result<()> {
        let cmd = format!(
            "ps ax -o pid,args \
                | grep '{}' \
                | grep -v 'grep' \
                | grep -Eo '^ *[0-9]+' \
                | sed 's/ //g' \
                | xargs kill -9",
            &self.home
        );

        let outputs = cmd::exec_output(&cmd).c(d!())?;

        println!("\x1b[31;1mCommands:\x1b[0m {}", cmd);
        println!(
            "\x1b[31;1mOutputs:\x1b[0m {}",
            alt!(outputs.is_empty(), "...", outputs.as_str())
        );

        for port in [
            self.ports.web3_http,
            self.ports.web3_ws,
            self.ports.tm_rpc,
            self.ports.tm_p2p,
            self.ports.tm_abci,
        ] {
            PortsCache::rm(port).c(d!())?;
        }

        Ok(())
    }

    fn delete(self) -> Result<()> {
        fs::remove_dir_all(self.home).c(d!())
    }
}

impl Default for Node {
    fn default() -> Self {
        Node {
            id: 0,
            tm_id: "".to_owned(),
            home: "".to_owned(),
            kind: Kind::Node,
            ports: Ports::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
enum Kind {
    Node,
    Full,
    Seed,
}

/// Active ports of a node
#[derive(Default, Debug, Clone, Deserialize, Serialize)]
struct Ports {
    web3_http: u16,
    web3_ws: u16,
    tm_p2p: u16,
    tm_rpc: u16,
    tm_abci: u16,
}

enum Ops {
    Create,
    Destroy,
    Start,
    Stop,
    AddNode,
    DelNode,
    Info,
}

impl Default for Ops {
    fn default() -> Self {
        Self::Info
    }
}

// global alloctor for ports
fn alloc_ports(node_kind: &Kind, env_name: &str) -> Result<Ports> {
    // web3_http, web3_ws, tm p2p, tm rpc, tm tm_abci
    const RESERVED_PORTS: [u16; 5] = [26654, 26655, 26656, 26657, 26658];

    let mut res = vec![];
    if matches!(node_kind, Kind::Full) && ENV_NAME_DEFAULT == env_name {
        res = RESERVED_PORTS.to_vec();
    } else {
        let mut cnter = 10000;
        while RESERVED_PORTS.len() > res.len() {
            let p = 20000 + rand::random::<u16>() % (65535 - 20000);
            if !RESERVED_PORTS.contains(&p)
                && !PortsCache::contains(p).c(d!())?
                && port_is_free(p)
            {
                res.push(p);
            }
            cnter -= 1;
            alt!(0 == cnter, return Err(eg!("ports can not be allocated")))
        }
    }

    PortsCache::set(res.as_slice()).c(d!())?;

    Ok(Ports {
        web3_http: res[0],
        web3_ws: res[1],
        tm_p2p: res[2],
        tm_rpc: res[3],
        tm_abci: res[4],
    })
}

fn port_is_free(port: u16) -> bool {
    info!(check_port(port)).is_ok()
}

fn check_port(port: u16) -> Result<()> {
    let fd = socket(
        AddressFamily::Inet,
        SockType::Datagram,
        SockFlag::empty(),
        None,
    )
    .c(d!())?;

    setsockopt(fd, sockopt::ReuseAddr, &true)
        .c(d!())
        .and_then(|_| setsockopt(fd, sockopt::ReusePort, &true).c(d!()))
        .and_then(|_| {
            bind(
                fd,
                &SockAddr::Inet(InetAddr::new(IpAddr::new_v4(0, 0, 0, 0), port)),
            )
            .c(d!())
        })
        .and_then(|_| close(fd).c(d!()))
}

#[derive(Debug, Serialize, Deserialize)]
struct PortsCache {
    file_path: String,
    port_set: BTreeSet<u16>,
}

impl PortsCache {
    fn new() -> Self {
        Self {
            file_path: Self::file_path(),
            port_set: BTreeSet::new(),
        }
    }

    fn file_path() -> String {
        format!("{}/ports_cache", ENV_BASE_DIR)
    }

    fn load() -> Result<Self> {
        match fs::read_to_string(Self::file_path()) {
            Ok(c) => serde_json::from_str(&c).c(d!()),
            Err(e) => {
                if ErrorKind::NotFound == e.kind() {
                    Ok(Self::new())
                } else {
                    Err(e).c(d!())
                }
            }
        }
    }

    fn write(&self) -> Result<()> {
        serde_json::to_string(self)
            .c(d!())
            .and_then(|c| fs::write(&self.file_path, c).c(d!()))
    }

    fn contains(port: u16) -> Result<bool> {
        Self::load().c(d!()).map(|i| i.port_set.contains(&port))
    }

    fn set(ports: &[u16]) -> Result<()> {
        let mut i = Self::load().c(d!())?;
        for p in ports {
            i.port_set.insert(*p);
        }
        i.write().c(d!())
    }

    fn rm(port: u16) -> Result<()> {
        let mut i = Self::load().c(d!())?;
        i.port_set.remove(&port);
        i.write().c(d!())
    }
}

fn exec_spawn(cmd: &str) -> Result<()> {
    Command::new("bash")
        .arg("-c")
        .arg(cmd)
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .c(d!())?
        .wait()
        .c(d!())
        .map(|exit_status| println!("{}", exit_status))
}

#[derive(Default, Debug, Clone, Deserialize, Serialize)]
struct TokenDistribution {
    addr_to_amount: BTreeMap<H160, U256>,
    addr_to_amount_readable: BTreeMap<H160, String>,
    addr_to_phrase: BTreeMap<H160, String>,
}

impl TokenDistribution {
    fn generate() -> Self {
        // pre-mint: 1 billion readable tokens
        const AM: u128 = 1_000_000_000;

        let (addr, phrase) = crate::client::gen_account();
        let am = pnk!(AM.checked_mul(pnk!(10u128.checked_pow(DECIMAL))));
        let am = U256::from(am);
        let am_readable = AM
            .to_string()
            .as_bytes()
            .rchunks(3)
            .rev()
            .collect::<Vec<_>>()
            .join(&b',');
        let am_readable = format!("{} OFUEL", pnk!(String::from_utf8(am_readable)));

        TokenDistribution {
            addr_to_amount: map! { B addr => am },
            addr_to_amount_readable: map! { B addr => am_readable },
            addr_to_phrase: map! { B addr => phrase },
        }
    }
}
