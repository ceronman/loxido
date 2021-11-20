#!/bin/bash

cd "$(dirname "$0")"
cargo build --release
perf record --call-graph dwarf,16384 -e cpu-clock -F 997 target/release/loxido $@