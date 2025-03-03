#!/bin/sh

if [ "$GDB" \= "" ]
then
   GDB=gdb
fi

TARGET_PATH=debug


if [ "$TARGET" \= "" ]
then
   TARGET="$(rustc --print host-tuple)"
fi

if [ "$RELEASE" != "" ]
then
    TARGET_PATH=release
fi

TARGET="$TARGET" ./build.sh

. ./local-env.sh

exec $GDB $GDBARGS --args target/x86_64-unknown-linux-none/${TARGET_PATH}/libwl_ld_lilium.so "$@"

