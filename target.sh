if [ "$TARGET" \= "" ]
then
   TARGET="$(rustc --print host-tuple)"
fi

IFS="-" read -r ARCH _REST <<< "$TARGET"

echo "$ARCH"

TARGET_LD="$ARCH-unknown-linux-none"
TARGET_RUST="$ARCH-unknown-linux-musl"