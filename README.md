![GitHub top language](https://img.shields.io/github/languages/top/ccmlm/OVR)
![GitHub issues](https://img.shields.io/github/issues-raw/ccmlm/OVR)
![GitHub pull requests](https://img.shields.io/github/issues-pr-raw/ccmlm/OVR)
![GitHub Workflow Status](https://img.shields.io/github/workflow/status/ccmlm/OVR/Rust)

# OVR

Overeality project.

## Usage

Quick start:

```shell
# compile binaries
make
# crate a local cluster
ovr dev --env-create --block-itv-secs 10
# stop it
ovr dev --env-stop
# start it again
ovr dev --env-start
# destroy it
ovr dev --env-destroy
```

Top-level overview:

```
ovr 0.2.1
OVR development team, fanhui.x@gmail.com
Official implementations of the Overeality project.

USAGE:
    ovr <SUBCOMMAND>

OPTIONS:
    -h, --help       Print help information
    -V, --version    Print version information

SUBCOMMANDS:
    client    Run ovr in client mode
    daemon    Run ovr in daemon mode, aka run a node
    dev       Development utils, create a local env, .etc
    help      Print this message or the help of the given subcommand(s)
```

A very useful sub-command for developers:

```
ovr-dev
Development utils, create a local env, .etc

USAGE:
    ovr dev [OPTIONS]

OPTIONS:
    -a, --env-add-node
    -c, --env-create
    -d, --env-destroy
    -h, --help                               Print help information
    -i, --env-info
    -I, --block-itv-secs <BLOCK_ITV_SECS>    [default: 1]
    -n, --env-name <ENV_NAME>
    -r, --env-rm-node
    -s, --env-start
    -S, --env-stop
```
