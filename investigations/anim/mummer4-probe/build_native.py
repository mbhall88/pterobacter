"""Compile bridge plus the exact pinned DeltaGraph implementation; no filter main."""
import hashlib
import json
import pathlib
import subprocess
import sys

source, native, out = map(pathlib.Path, sys.argv[1:])
inputs = [p for folder in ('include', 'src') for p in (source/folder).rglob('*') if p.is_file()]
inputs += [native/'.libs/libumdmummer.a', pathlib.Path('bridge.cc')]
hashes = {str(p): hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(inputs)}
(out/'native-input-hashes.json').write_text(json.dumps(hashes, indent=2)+'\n')
commands = []
objects = []
for index, path in enumerate([pathlib.Path('bridge.cc'), source/'src/tigr/delta.cc']):
    obj = out/f'{index}.o'
    cmd = ['g++','-std=c++17','-O3','-DNDEBUG','-pthread','-I'+str(source/'include'),'-I'+str(native),'-c',str(path),'-o',str(obj)]
    subprocess.run(cmd, check=True)
    commands.append(cmd)
    objects.append(str(obj))
cmd = ['ar','rcs',str(out/'libmummer4_bridge.a'),*objects]
subprocess.run(cmd, check=True)
commands.append(cmd)
(out/'native-build-argv.json').write_text(json.dumps(commands, indent=2)+'\n')
