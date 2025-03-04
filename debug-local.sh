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

TARGET="$TARGET" ./build.sh || exit $?

. ./local-env.sh

if [[ "$GDB" == *"rr"* ]]
then
    if [[ "$GDB" != *"record"* ]]
    then
        GDB="$GDB record" # use "rr record" if the user didn't try to outsmart us
    fi
    exec $GDB $GDBARGS target/x86_64-unknown-linux-none/${TARGET_PATH}/libwl_ld_lilium.so "$@"
else
    exec $GDB $GDBARGS --args target/x86_64-unknown-linux-none/${TARGET_PATH}/libwl_ld_lilium.so "$@"
fi
