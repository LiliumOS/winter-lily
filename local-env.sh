export LD_LIBRARY_PATH_WL_HOST="$(pwd)/target/${TARGET_RUST}/${TARGET_PATH}:$(pwd)/target/${TARGET_RUST}/${TARGET_PATH}/deps:${LD_LIBRARY_PATH}:${LD_LIBRARY_PATH_WL_HOST}"
export WL_NATIVE_LD_SO_CONF="$PREFIX/etc/ld.so.conf"
export WL_SUBSYS_base="target/${TARGET_RUST}/${TARGET_PATH}/libwl_usi_base.so"
export WL_SUBSYS_io="target/${TARGET_RUST}/${TARGET_PATH}/libwl_usi_io.so"
unset WL_SYSROOT