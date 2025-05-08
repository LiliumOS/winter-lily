#!/bin/bash
_infname="$1"
_ofname="$(echo "$_infname" | sed -e "s/\\(\\.*\\).in/\1/")"
cat "$_infname" | sed -e "s/%subsysname%/${subsysname}/" > "$_ofname"
rm -f "$_infname"