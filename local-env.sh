export LD_LIBRARY_PATH_WL_HOST="$(pwd)/target/${TARGET_RUST}/${TARGET_PATH}:$(pwd)/target/${TARGET_RUST}/${TARGET_PATH}/deps:${LD_LIBRARY_PATH}:${LD_LIBRARY_PATH_WL_HOST}"
export WL_NATIVE_LD_SO_CONF="$PREFIX/etc/ld.so.conf"
unset WL_SYSROOT
for subsys in $(cat subsysnames)
do
    export WL_SUBSYS_${subsys}="$(pwd)/target/${TARGET_RUST}/${TARGET_PATH}/libwl_usi_${subsys}.so"
done