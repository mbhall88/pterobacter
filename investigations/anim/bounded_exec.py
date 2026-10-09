"""Linux launcher: constrain all descendants before executing a native stage."""
import os
import resource
import sys

if __name__ == '__main__':
    cpus, memory_kib, *argv = sys.argv[1:]
    os.sched_setaffinity(0, set(map(int, cpus.split(','))))
    ceiling = int(memory_kib) * 1024
    resource.setrlimit(resource.RLIMIT_AS, (ceiling, ceiling))
    os.execvpe(argv[0], argv, os.environ)
