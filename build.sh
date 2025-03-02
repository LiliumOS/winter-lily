#!/bin/sh

CARGOFLAGS=

if [ "$TARGET" \= "" ]
then
   TARGET="$(rustc --print host-tuple)"
fi

if [ "$RELEASE" != "" ]
then
    CARGOFLAGS="--release"
fi

CARGO_TARGET_DIR="$(pwd)/target"

cargo build -Z build-std="core,alloc,std" --target "$TARGET" $CARGOFLAGS

(cd wl-ld-lilium && cargo build --target-dir "$CARGO_TARGET_DIR")