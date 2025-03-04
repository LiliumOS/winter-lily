#!/bin/sh

if [ "$GDB" \= "" ]
then
   GDB=gdb
fi

. ./target.sh

if [ "$RELEASE" != "" ]
then
    TARGET_PATH=release
fi

TARGET="$TARGET" ./build.sh

. ./local-env.sh

exec $GDB $GDBARGS --args target/${TARGET_LD}/${TARGET_PATH}/libwl_ld_lilium.so "$@"

