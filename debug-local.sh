#!/bin/bash

if [ "$GDB" \= "" ]
then
   GDB=gdb
fi

. ./target.sh

TARGET_PATH="debug"

if [ "$RELEASE" != "" ]
then
    TARGET_PATH=release
fi

TARGET="$TARGET" ./build.sh

. ./local-env.sh

exec $GDB $GDBARGS --args target/${TARGET_LD}/${TARGET_PATH}/libwl_ld_lilium.so "$@"

