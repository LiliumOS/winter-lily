export LD_LIBRARY_PATH_WL_HOST="$(pwd)/target/${TARGET}/${TARGET_PATH}:$(pwd)/target/${TARGET}/${TARGET_PATH}/deps:${LD_LIBRARY_PATH}:${LD_LIBRARY_PATH_WL_HOST}:/lib"
export WL_SUBSYS_base="target/${TARGET}/${TARGET_PATH}/libwl_usi_base.so"