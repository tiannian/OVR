#!/usr/bin/env bash

#################################################
#### Ensure we are in the right path. ###########
#################################################
if [[ 0 -eq $(echo $0 | grep -c '^/') ]]; then
    # relative path
    EXEC_PATH=$(dirname "`pwd`/$0")
else
    # absolute path
    EXEC_PATH=$(dirname "$0")
fi

EXEC_PATH=$(echo ${EXEC_PATH} | sed 's@/\./@/@g' | sed 's@/\.*$@@')
cd $EXEC_PATH || exit 1
#################################################

cd .. || exit 1

target_path=$1

if [[ ! -d "${target_path}/.git" ]]; then
    rm -rf $target_path
    url='https://gitee.com/kt10/tendermint'
    # url='https://github.com/tendermint/tendermint.git'
    git clone -b v0.34.15 --depth=1 $url $target_path
fi
