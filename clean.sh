#!/bin/bash

what=${1:-all}
case "$what" in
gcc )
    rm -rf target/gcc-*
    ;;
binutils )
    rm -rf target/binutils-*
    ;;
toolbuilds )
    rm -rf target/gcc-*
    rm -rf target/binutils-*
    ;;
all )
    cargo clean
    # make -C musl distclean
    ;;
esac