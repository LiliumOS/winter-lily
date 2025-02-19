#!/bin/sh

CARGOFLAGS=

TARGET_PATH=debug

if [ "$TARGET" \= "" ]
then
   TARGET="$(rustc --print host-tuple)"
fi

if [ "$RELEASE" != "" ]
then
    CARGOFLAGS="--release"
    TARGET_PATH=release
fi

cargo build --target="$TARGET" $(CARGOFLAGS)

LD_LIBRARY_PATH_WL_HOST="$(pwd)/target/${TARGET}/${TARGET_PATH}:$(pwd)/target/${TARGET}/${TARGET_PATH}/deps:${LD_LIBRARY_PATH}" exec gdb target/${TARGET}/${TARGET_PATH}/libwl_ld_lilium.so "$@"

