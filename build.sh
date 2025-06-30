#!/bin/bash

CARGOFLAGS=

. ./target.sh

if [ "$RELEASE" != "" ]
then
    CARGOFLAGS="--release"
    CFLAGS_MUSL="-O3"
    TARG_DIR="release"
else 
    CFLAGS_MUSL="-O2 -g"
    export WL_LD_DEBUG_DEFAULT="1"
    TARG_DIR="debug"
fi

# echo "ARCH=$ARCH" > musl/config.mak
# echo "prefix=$PREFIX" >> musl/config.mak
# echo "exec_prefix=$PREFIX" >> musl/config.mak
# echo "syslibdir=$PREFIX/lib" >> musl/config.mak
# echo "LIBCC=-static-libgcc --rtlib=compiler-rt --unwindlib=libunwind" >> musl/config.mak
# echo "CC=clang" >> musl/config.mak
# echo "CFLAGS=$CFLAGS_MUSL" >> musl/config.mak

NPROC=$(("$(nproc)"*2))

# echo "Building: musl"
# make -C musl all -j${NPROC}  && make -C musl install -j${NPROC} || exit $?

build_autotools(){
    prg="$1"
    echo "Building: $1"
    mkdir -p "$CARGO_TARGET_DIR"
    BUILD_DIR="$CARGO_TARGET_DIR/$1-${TARGET_RUST}"
    SRC_DIR="$(pwd)/$2"
    shift 2

    if mkdir "$BUILD_DIR" 2> /dev/null || [ "$REBUILD_TOOL" = "$prg" -o "$REBUILD_TOOL" = "all" ]
    then
        (cd "$BUILD_DIR" && PATH="$PREFIX/bin:$PATH" "$SRC_DIR/configure" --target "${TARGET_RUST}" --prefix "$PREFIX" --exec-prefix "$PREFIX" "$@") && [ "$PREREQS_NO_BUILD" = "$prg" ] || ( make -C "$BUILD_DIR" -j"$NPROC" && make -C "$BUILD_DIR" -s install )
    else
        true
    fi
    return $?
}

# build_autotools binutils binutils-gdb --disable-gdb || exit $?
# build_autotools gcc gcc --with-build-sysroot="$PREFIX" --with-headers="$PREFIX/include" --disable-multilib --disable-bootstrap --enable-languages=c --enable-shared --disable-libvtv --disable-libssp --disable-libquadmath --disable-libsanitizer --disable-libgomp --disable-libatomic  || exit $?

# ln -sf $PREFIX/$TARGET_RUST/lib64/libgcc_s.so $PREFIX/lib/libgcc_s.so

if [ "$STAGE" != "prereqs" ]
then

# echo "Building ld-lilium-$ARCH.so"
# (cd wl-ld-lilium && cargo build -Z build-std="core,alloc" --target-dir "$CARGO_TARGET_DIR" --target "$TARGET_LD") || exit $?

echo "Building: wl host libraries"
LINK_TARGET_DIR="$CARGO_TARGET_DIR/$TARGET_RUST/$TARG_DIR" RUSTFLAGS="--cap-lints allow" cargo build -Z build-std="core,alloc"  --target-dir "$CARGO_TARGET_DIR" --target "$TARGET_RUST" $CARGOFLAGS || exit $?

mkdir -p "$PREFIX/etc"

echo > "$PREFIX/etc/ld.so.conf"

(cd tests/$ARCH && ./build.sh) || exit $?

# echo "$PREFIX/lib" >> $PREFIX/etc/ld.so.conf
# echo "$PREFIX/$LIB_TARG" >> $PREFIX/etc/ld.so.conf
# echo "$PREFIX/$TARGET_RUST/lib" >> $PREFIX/etc/ld.so.conf
# echo "$PREFIX/$TARGET_RUST/$LIB_TARG" >> $PREFIX/etc/ld.so.conf
fi