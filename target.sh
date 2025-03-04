if [ CARGO_MAKEFLAGS != "" -a MAKEFLAGS \= "" ]
then
   export MAKEFLAGS="$CARGO_MAKEFLAGS" # If we're running via cargo,  set `CARGO_MAKEFLAGS` accordingly
fi

if [ "$TARGET" \= "" ]
then
   TARGET="$(rustc --print host-tuple)"
fi

IFS="-" read -r ARCH _REST <<< "$TARGET"

case "$_RUST" in
   *-*-*)
      IFS="-" read -r VENDOR OS SYS <<< "$VENDOR"
      ;;
   *-* )
      VENDOR="pc"
      IFS="-" read -r OS SYS <<< "$VENDOR"
      ;;
esac

echo "$ARCH"

TARGET_LD="$ARCH-unknown-linux-none"
TARGET_RUST="$ARCH-unknown-linux-musl"


CARGO_TARGET_DIR="$(pwd)/target"

PREFIX="$CARGO_TARGET_DIR/musl-${TARGET_RUST}"
LIB_TARG=lib64 # TODO: disambiguate between these