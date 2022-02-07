![GitHub top language](https://img.shields.io/github/languages/top/ccmlm/OVR)
![GitHub Workflow Status](https://img.shields.io/github/workflow/status/ccmlm/OVR/Rust)
![GitHub issues](https://img.shields.io/github/issues-raw/ccmlm/OVR)
![GitHub pull requests](https://img.shields.io/github/issues-pr-raw/ccmlm/OVR)

# OVR

Overeality project.

## Usage

```
ovr 0.1.2
OVR development team, fanhui.x@gmail.com
Official implementations of the Overeality project.

USAGE:
    ovr <SUBCOMMAND>

OPTIONS:
    -h, --help       Print help information
    -V, --version    Print version information

SUBCOMMANDS:
    daemon    Run ovr in daemon mode, aka run a node
    client    Run ovr in client mode, default option
    debug     Use debug utils, eg, create a local env
    btm       BTM related operations
```

## Usage: `ovr daemon`

```
ovr-daemon
Run ovr in daemon mode, aka run a node

USAGE:
    ovr daemon [OPTIONS]

OPTIONS:
    -a, --serv-addr-list <SERV_ADDR_LIST>
            Addresses served by the daemon, seperated by ',' [default: [::],0.0.0.0]

        --block-base-fee-per-gas <BLOCK_BASE_FEE_PER_GAS>
            A field for EIP1559

        --block-gas-limit <BLOCK_GAS_LIMIT>
            The limitation of the total gas of any block

        --btm-algo <BTM_ALGO>
            [default: Fair]

    -C, --btm-cap <BTM_CAP>
            [default: 100]

        --chain-id <CHAIN_ID>
            The ID of your chain, an unsigned integer [default: 0]

        --chain-name <CHAIN_NAME>
            A custom name of your chain [default: NULL]

        --chain-version <CHAIN_VERSION>
            A custom version of your chain [default: NULL]

    -d, --vsdb-base-dir <VSDB_BASE_DIR>
            A path where all data will be stored in [default: ~/.vsdb]

        --gas-price <GAS_PRICE>
            Basic gas price of the evm transactions

    -h, --help
            Print help information

    -I, --btm-itv <BTM_ITV>
            [default: 10]

    -m, --serv-mgmt-port <SERV_MGMT_PORT>
            An UDP port used for system managements [default: 9527]

    -M, --btm-mode <BTM_MODE>
            Will try to detect the local system if missing

    -p, --serv-http-port <SERV_HTTP_PORT>
            A port used for http service [default: 30000]

    -P, --btm-volume <BTM_VOLUME>
            Will try ${ENV_VAR_BTM_TARGET} if missing

    -w, --serv-ws-port <SERV_WS_PORT>
            A port used for websocket service [default: 30001]
```

## Usage: `ovr client`

```
ovr-client
Run ovr in client mode, default option

USAGE:
    ovr client [OPTIONS]

OPTIONS:
    -a, --serv-addr <SERV_ADDR>
            Addresses served by the server end, defalt to 'localhost' [default: localhost]

    -h, --help
            Print help information

    -m, --serv-mgmt-port <SERV_MGMT_PORT>
            An UDP port used for system managements [default: 9527]

    -p, --serv-http-port <SERV_HTTP_PORT>
            A port used for http service [default: 30000]

    -w, --serv-ws-port <SERV_WS_PORT>
            A port used for websocket service [default: 30001]
```

## Usage: `ovr debug`

**TODO**

## Usage: `ovr btm`

```
ovr-btm
BTM related operations

USAGE:
    ovr btm <SUBCOMMAND>

OPTIONS:
    -h, --help    Print help information

SUBCOMMANDS:
    list        List all existing snapshots
    rollback    Rollback to a custom historical snapshot
    clean       Clean up all existing snapshots
```

```
ovr-btm-list
List all existing snapshots

USAGE:
    ovr btm list [OPTIONS]

OPTIONS:
    -h, --help               Print help information
    -M, --mode <MODE>        Will try to detect the local system if missing
    -P, --volume <VOLUME>    Will try ${ENV_VAR_BTM_TARGET} if missing
```

```
ovr-btm-rollback
Rollback to a custom historical snapshot

USAGE:
    ovr btm rollback [OPTIONS]

OPTIONS:
    -h, --help               Print help information
    -H, --height <HEIGHT>    Will try to use the latest existing height if missing
    -M, --mode <MODE>        Will try to detect the local system if missing
    -P, --volume <VOLUME>    Will try to use ${ENV_VAR_BTM_TARGET} if missing
    -X, --exact              If specified, a snapshot must exist at the 'height'
```

```
ovr-btm-clean
Clean up all existing snapshots

USAGE:
    ovr btm clean [OPTIONS]

OPTIONS:
    -h, --help               Print help information
    -M, --mode <MODE>        Will try to detect the local system if missing
    -P, --volume <VOLUME>    Will try ${ENV_VAR_BTM_TARGET} if missing
```
