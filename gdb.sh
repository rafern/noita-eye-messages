#!/bin/sh
gdb --args ./target/release/search data/ciphertext/all-original.csv 'out(0,0)==out(1,0)' arx 2 -s