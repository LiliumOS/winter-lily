#!/bin/sh


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

echo "$@"

LD_LIBRARY_PATH_WL_HOST="$(pwd)/target/${TARGET}/${TARGET_PATH}:$(pwd)/target/${TARGET}/${TARGET_PATH}/deps:${LD_LIBRARY_PATH}" WL_SUBSYS_base="target/${TARGET}/${TARGET_PATH}/libwl_usi_base.so" exec target/x86_64-unknown-linux-none/${TARGET_PATH}/libwl_ld_lilium.so "$@"

