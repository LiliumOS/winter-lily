#!/bin/bash

for subsysname in "$@"
do
cp -r -T module-lib-templ "wl-usi-${subsysname}" || exit $?

subsysname="$subsysname" find "wl-usi-${subsysname}" -name '*.in' -execdir $(pwd)/new-subsys-replace.sh "{}" \; || exit $?

echo -n "${subsysname} " >> subsysnames 

sed -i -e "s|#%MARKER%|\"wl-usi-${subsysname}\",\\n    \\0|" Cargo.toml
done