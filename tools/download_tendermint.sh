#!/usr/bin/env bash

target_path=$1

if [[ ! -d "${target_path}/.git" ]]; then
    rm -rf $target_path
    url='https://github.com/tendermint/tendermint.git'
    git clone -b v0.34.15 --depth=1 $url $target_path
fi
