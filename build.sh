#!/bin/bash

CARGOFLAGS=

. ./target.sh

if [ "$RELEASE" != "" ]
then
    CARGOFLAGS="--release"
fi

CARGO_TARGET_DIR="$(pwd)/target"

echo "ARCH=$ARCH" > musl/config.mak
echo "prefix=$CARGO_TARGET_DIR/musl" >> musl/config.mak
echo "exec_prefix=$CARGO_TARGET_DIR/musl" >> musl/config.mak
echo "syslibdir=$CARGO_TARGET_DIR/musl/lib" >> musl/config.mak

make -C musl all
make -C musl install

ln -sf "$(find /usr/lib -name libgcc.a -print -quit)" "$CARGO_TARGET_DIR/musl/lib/libgcc_s.a"

PATH="$CARGO_TARGET_DIR/musl/bin:$PATH" RUSTFLAGS="-Ctarget-feature=-crt-static -C link-arg=-static-libgcc -Clinker=musl-gcc -Clinker-flavor=gcc" cargo build -Z build-std="core,alloc,std" --target "$TARGET_RUST" $CARGOFLAGS

(cd wl-ld-lilium && cargo build -Z build-std="core,alloc" --target-dir "$CARGO_TARGET_DIR" --target "$TARGET_LD")