#!/bin/sh

CARGOFLAGS=

TARGET_PATH=debug

if [ "$RELEASE" -ne "" ]
then
    CARGOFLAGS="--release"
    TARGET_PATH=release
end

TARGET="$(rustc --print host)"

IFS="-" read -r TARGET_ARCH <<< "$TARGET"

LILIUM_TARGET="$TARGET_ARCH-pc-lilium-std"

WL_LILIUM_TARGET="$LILIUM_TARGET" cargo build --target="$TARGET" $(CARGOFLAGS)

LD_LIBRARY_PATH_WL_HOST="$(pwd)/target/$(TARGET)/$(TARGET_PATH):$(pwd)/target/$(TARGET)/$(TARGET_PATH)/deps:${LD_LIBRARY_PATH}" exec target/$($TARGET)/$(TARGET_PATH)/libwl_ld_lilium.so "$@"

