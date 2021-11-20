#!/bin/python3

import argparse
import pathlib
import subprocess
import time

BENCHMARK_DIR = "tests/benchmarks/lox"


parser = argparse.ArgumentParser(description='Run Lox benchmarks')
parser.add_argument('lox_path', metavar='lox-path', type=str, help='Path to the Lox interpreter binary')
args = parser.parse_args()
lox = pathlib.Path(args.lox_path).resolve()

for benchmark in sorted(pathlib.Path(BENCHMARK_DIR).glob("*.lox")):
    times = []
    for i in range(5):
        output = subprocess.check_output([str(lox), str(benchmark.resolve())])
        times.append(float(output.splitlines()[-1]))
        time.sleep(2)
    print(f"{benchmark.name}: {round(min(times), 4)}")