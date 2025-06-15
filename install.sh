#!/bin/bash

_in_cmd="$0"

case "$_in_cmd" in
    *[/\\]*)
    _cmd="$0"
    ;;
    *)
    _cmd="$(command -v "$_in_cmd")"
    ;;
esac

whereami="$(dirname "$_cmd")"

print_usage(){
    echo "Usage: $_in_cmd [OPTIONS]..."
    echo "Installs winter-lily to a sysroot"
    echo "Options:"
    echo "  --prefix <prefix>: Sets the prefix to install to (within the sysroot). Default is \`/\`"
    echo "  --exec-prefix <exec_prefix>: Sets the exec-prefix to install to (within the sysroot). Relative directories are resolved relative to the prefix. Default is the prefix"
    echo "  --libdir <libdir>: Sets the library directory for lilium libraries. Relative directories are resolved relative to the exec-prefix. Default is lib"
    echo "  --host-libdir <hostlibdir>: Sets the library directory for host libraries. Relative directories are resolved relative to the libdir. Default is host"
    echo "  --syslibdir <syslibdir>: Sets the directory for the dynamic linker. Must be absolute. Default is /lib"
    echo "  --sysconfdir <sysconfdir>: Sets the directory for system configuration files. Relative directories are resolved relative to the prefix. Default is etc"
    echo "  --sysroot <sysroot>: Sets the sysroot to install into. Default is \$HOME/.wl"
    echo "  --help: Prints this message and exits"
    echo "  --version: Prints version information and exits"
    echo "  --verbose: Prints information about the program's operations"
    echo "  --dry-run: Don't install anything. Only useful if combined with --verbose"
    echo "  --save-temps: Don't delete temporary files created during the installation process"
    echo "Environment Variables:"
    echo "Each directory controlled by an option may be set as an environment variable instead of a flag."
    echo "The environemtn variable name is the name of the flag with - replaced by _."
    echo "If both an environment variable and an option sets the same directory, the option takes precedence"
}

case "$VERBOSE" in
    y|yes|1|true|on)
        _verbose=1
    ;;
    extra|2)
        _verbose=2
    ;;
    *)
        _verbose=0
    ;;
esac

case "$DRY_RUN" in
    y|yes|1|true|on)
        _dry_run="1"
    ;;
    *)
        _dry_run="0"
    ;;
esac

case "$SAVE_TEMPS" in
    y|yes|1|true|on)
        _save_temps="1"
    ;;
    *)
        _save_temps="0"
    ;;
esac

while [ "$#" -ne 0 ]
do
    case "$1" in
        --prefix)
            prefix="$2"
            shift 2
        ;;
        --exec-prefix)
            exec_prefix="$2"
            shift 2
        ;;
        --libdir)
            libdir="$2"
            shift 2
        ;;
        --host-libdir)
            host_libdir="$2"
            shift 2
        ;;
        --sysconfdir)
            sysconfdir="$2"
            shift 2
        ;;
        --syslibdir)
            syslibdir="$2"
            shift 2
        ;;
        --datadir)
            datadir="$2"
            ;;
        --datarootdir)
            datarootdir="$2"
            ;;
        --sysroot)
            sysroot="$2"
            shift 2
        ;;
        --help)
            print_usage
            exit 0
        ;;
        --version)
            echo "winter-lily 0.1.0 install script"
            exit 0
        ;;
        --verbose)
            _verbose=1
            shift 1
        ;;
        --dry-run)
            _dry_run=1
            shift 1
        ;;
        --save-temps)
            _save_temps=1
            shift 1
        ;;
        --*)
            echo "Unknown option $1" 1>&2
            exit 1
        ;;
        *)
            print_usage 1>&2
            exit 1
        ;;
    esac
done 

. "$whereami/target.sh"

[ $_dry_run -eq 1 ] || RELEASE=1 "$whereami/build.sh" || exit $?

_sysroot="${sysroot:-$HOME/.wl}"

_prefix="${prefix:-/}"
case "${exec_prefix}" in
    [/\\]*)
        _exec_prefix="${exec_prefix}"
        ;;
    *)
        _exec_prefix="${_prefix}/${exec_prefix:-}"
        ;;
esac

case "${bindir}" in
    [/\\]*)
        _bindir="${bindir}"
        ;;
    *)
        _bindir="${_exec_prefix}/${bindir:-bin}"
    ;;
esac

_bindir="$(echo "$_bindir" | sed -e "s|//\+|/|g")"

case "${libdir}" in
    [/\\]*)
        _libdir="${libdir}"
        ;;
    *)
        _libdir="${_exec_prefix}/${libdir:-lib}"
    ;;
esac

_libdir="$(echo "$_libdir" | sed -e "s|//\+|/|g")"

case "${host_libdir}" in
    [/\\]*)
        _host_libdir="${host_libdir}"
        ;;
    *)
        _host_libdir="${_libdir}/${host_libdir:-host}"
    ;;
esac

_host_libdir="$(echo "$_host_libdir" | sed -e "s|//\+|/|g")"

case "${sysconfdir}" in
    [/\\]*)
        _sysconfdir="${sysconfdir}"
        ;;
    *)
        _sysconfdir="${_prefix}/${sysconfdir:-etc}"
    ;;
esac
_sysconfdir="$(echo "$_sysconfdir" | sed -e "s|//\+|/|g")"

case "${syslibdir}" in
    [/\\*])
        _syslibdir="${syslibdir}"
        ;;
    *)
        _syslibdir="/${syslibdir:-lib}"
        ;;
esac

_syslibdir="$(echo "$_syslibdir" | sed -e "s|//\+|/|g")"

case "${datarootdir}" in
    [/\\*])
        _datarootdir="${datarootdir}"
        ;;
    *)  _datarootdir="${_prefix}/${datarootdir:-share}"
        ;;
esac 

case "${datadir}" in
    [/\\*])
        _datadir="${datadir}"
        ;;
    *)  _datadir="${_datarootdir}/${datadir}"
        ;;
esac

case "${STRIP:-no}" in
    1|y|yes|true)
    _strip_opts="--strip"
    ;;
    0|n|no|false)
    _strip_opts=""
    ;;
    *)
    _strip_opts="--strip --strip-program=\"$STRIP\""
    ;;
esac

install_dir(){
    [ $_verbose -ne 0 ] && echo install -m755 -D -d "${_sysroot}$1"
    [ $_dry_run -eq 1 ] || install -m755 -D -d "${_sysroot}$1"
}

install_prg(){
    [ $_verbose -ne 0 ] && echo install -m755 $_strip_opts -D -T "$2" "${_sysroot}$1"
    [ $_dry_run -eq 1 ] || install -m755 $_strip_opts -D -T "$2" "${_sysroot}$1"
}

install_lib(){
    [ $_verbose -ne 0 ] && echo install -m644 $_strip_opts -D -T "$2" "${_sysroot}$1"
    [ $_dry_run -eq 1 ] || install -m644 $_strip_opts -D -T "$2" "${_sysroot}$1"
}

install_other(){
    [ $_verbose -ne 0 ] && echo install -m644 -D -T "$2" "${_sysroot}$1"
    [ $_dry_run -eq 1 ] || install -m644 -D -T "$2" "${_sysroot}$1"
}

install_link() {
    [ $_verbose -ne 0 ] && echo ln -sf "$2" "${_sysroot}$1"
    [ $_dry_run -eq 1 ] || ln -sf "$2" "${_sysroot}$1" 
}

install_template() {
    _mode="${install_mode:-644}"
    declare -a _sed_args
    _target="$1"
    _file="$2"
    shift 2
    [ $_verbose -ne 0 ] && echo sed install -m${_mode} -D -T "$_file" "${_sysroot}${_target}"
    _fname="$(mktemp)"
    [ $_verbose -gt 1 ] && echo "Temporary File ${_fname}"
    for arg in "$@"
    do
        _sed_args+=("-es!%${arg}%!${!arg}!g")
    done

    [ $_verbose -gt 1 ] && echo "sed ${_sed_args[*]}"
    cat "$_file" | sed "${_sed_args[@]}" > "$_fname" || return $?
    [ $_dry_run -eq 1 ] || install -m${_mode} -D -T "$_fname" "${_sysroot}${_target}" || return $?
    [ $_save_temps -ne 1 ] && unlink "$_fname"
}

libdir="$_libdir" host_libdir="$_host_libdir" install_template "${_sysconfdir}/ld-lilium.so.conf" "${whereami}/install/ld-lilium.so.conf.in" libdir host_libdir || exit $?
libdir="$_libdir" host_libdir="$_host_libdir" install_template "${_sysconfdir}/ld.so.conf" "${whereami}/install/ld.so.conf.in" libdir host_libdir || exit $?

# install_lib "${_host_libdir}/libc.so" "$PREFIX/lib/libc.so" || exit $?
# install_lib "${_host_libdir}libgcc_s.so.1" "$PREFIX/$TARGET_RUST/$LIBTARG/libgcc_s.so.1" || exit $?
install_prg "${_syslibdir}/ld-lilium-$ARCH.so.1" "$CARGO_TARGET_DIR/$TARGET_LD/release/libwl_ld_lilium.so" || exit $?

_to_syslibdir=""

_comp="$_libdir"

while [ "$_comp" != "/" ]
do
    _comp="$(dirname "$_comp")"
    _to_syslibdir="../${_to_syslibdir}"
done

install_link "${_libdir}/wl-ld-lilium-$ARCH.so" "${_to_syslibdir}${_syslibdir}/ld-lilium-$ARCH.so.1"

install_mode=755 install_template "${_bindir}/winter-lily" "${whereami}/install/winter-lily.in" ARCH || exit $?
install_template "${_datarootdir}/wl-dev" "${whereami}/install/wl-dev.in" _sysroot _host_libdir || exit $?

install_lib "${_host_libdir}/libwl_impl.so" "$CARGO_TARGET_DIR/$TARGET_RUST/release/libwl_impl.so" || exit $?
for subsys in $(cat ${whereami}/subsysnames)
do
    install_lib "${_host_libdir}/libwl-usi-$subsys.so" "$CARGO_TARGET_DIR/$TARGET_RUST/release/libwl_usi_$subsys.so" || exit $?
    libdir="$_libdir" host_libdir="$_host_libdir" usilib="$subsys" install_template "${_libdir}/libusi-$subsys.so" "${whereami}/install/scripts/libusi-X.so.in" libdir host_libdir usilib || exit $?
done