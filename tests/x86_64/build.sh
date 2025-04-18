#!/bin/sh

for name in lilium-hello
do
    as -o ${name}.o ${name}.as
    ld -pic -o ${name} ${name}.o
done