#!/usr/bin/env python3
"""Set resource limits, then exec the command: `_limit_exec.py ADDRESS_SPACE_BYTES CPU_SECONDS -- CMD...`.

Used instead of subprocess's preexec_fn, which Python documents as unsafe when the parent has threads (run_candidate.py launches
from a thread pool). This helper runs as its own process, so limits are set before the command starts without running Python in a
forked child of a threaded parent.
"""
import os
import resource
import sys

mem, cpu = int(sys.argv[1]), int(sys.argv[2])
assert sys.argv[3] == "--" and len(sys.argv) > 4
if mem:
    resource.setrlimit(resource.RLIMIT_AS, (mem, mem))
if cpu:
    resource.setrlimit(resource.RLIMIT_CPU, (cpu, cpu + 1))
os.execvp(sys.argv[4], sys.argv[4:])
