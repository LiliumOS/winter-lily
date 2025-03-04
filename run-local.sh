#!/bin/bash

. ./target.sh

TARGET_PATH=debug

if [ "$RELEASE" != "" ]
then
    TARGET_PATH=release
fi
TARGET="$TARGET" ./build.sh

echo "$@"
. ./local-env.sh
exec target/${TARGET_LD}/${TARGET_PATH}/libwl_ld_lilium.so "$@"

