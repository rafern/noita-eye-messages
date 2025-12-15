#!/bin/sh
gdb --args ./target/release/search data/ciphertext/all-original.csv 'pt(0,0)==pt(1,0)' arx 2 -s