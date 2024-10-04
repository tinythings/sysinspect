#!/usr/bin/bash

mroot="/tmp/sysinspect/modules"
mkdir -p $mroot

# Sys
s_root="$mroot/sys"
mkdir -p $s_root

for m in proc info; do
    touch "$s_root/$m"
done
