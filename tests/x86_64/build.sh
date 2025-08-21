#!/bin/sh

TARGET=x86_64-lilium-std

for name in lilium-hello print-argv
do
    if [ ${name}.as -nt ${name} ]
    then
        echo "Building ${name}"
        ${TARGET}-as -o ${name}.o ${name}.as
        ${TARGET}-ld -pic -o ${name} ${name}.o
    fi
done