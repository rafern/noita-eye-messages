#!/bin/sh
perf record --call-graph dwarf ./target/release/search data/ciphertext/all-original.csv 'equals(pt(0,0),pt(1,0))' arx 2 -s
