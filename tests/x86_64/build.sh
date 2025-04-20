#!/bin/sh

for name in lilium-hello
do
    if [ ${name}.as -nt ${name} ]
    then
        echo "Building ${name}"
        as -o ${name}.o ${name}.as
        ld -pic -o ${name} ${name}.o
    fi
done