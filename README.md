![](https://tokei.rs/b1/github/ccmlm/OVR)
![GitHub top language](https://img.shields.io/github/languages/top/ccmlm/OVR)
![GitHub Workflow Status](https://img.shields.io/github/workflow/status/ccmlm/OVR/Rust)
![GitHub issues](https://img.shields.io/github/issues-raw/ccmlm/OVR)
![GitHub pull requests](https://img.shields.io/github/issues-pr-raw/ccmlm/OVR)

# OVR

Overeality project.

## Usage

```
ovr 0.1.1
fanhui.x@gmail.com
Official implementations of the Overeality project.

USAGE:
    ovr [OPTIONS] <SUBCOMMAND>

OPTIONS:
    -a, --serv-addr-list <SERV_ADDR_LIST>
            Addresses served by the daemon, defalt to '[::]' and '0.0.0.0'

    -h, --serv-http-port <SERV_HTTP_PORT>
            A port used for http service, default to 30000

        --help
            Print help information

    -m, --serv-mgmt-port <SERV_MGMT_PORT>
            An udp port used for system managements, default to 9527

    -V, --version
            Print version information

    -w, --serv-ws-port <SERV_WS_PORT>
            A port used for websocket service, default to 30001

SUBCOMMANDS:
    client    Run ovr in client mode, default option
    daemon    Run ovr in daemon mode, aka run a node
    help      Print this message or the help of the given subcommand(s)
```

```
ovr-daemon
Run ovr in daemon mode, aka run a node

USAGE:
    ovr daemon [OPTIONS] --chain-id <CHAIN_ID> --chain-name <CHAIN_NAME> --chain-version <CHAIN_VERSION>

OPTIONS:
        --block-base-fee-per-gas <BLOCK_BASE_FEE_PER_GAS>
            A field for EIP1559

        --block-gas-limit <BLOCK_GAS_LIMIT>
            The limitation of the total gas of any block

        --chain-id <CHAIN_ID>
            The ID of your chain, an unsigned integer

        --chain-name <CHAIN_NAME>
            A custom name of your chain

        --chain-version <CHAIN_VERSION>
            A custom version of your chain

    -d, --vsdb-base-dir <VSDB_BASE_DIR>
            A path where all data will be stored in, default to '~/.vsdb'

        --gas-price <GAS_PRICE>
            Basic gas price of the evm transactions

    -h, --help
            Print help information
```
