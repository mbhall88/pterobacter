"""Exercise blank override rejection through real Cargo build-script execution."""
import argparse
import json
import os
from pathlib import Path
import subprocess

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--compiler", required=True)
parser.add_argument("--offline", action="store_true")
parser.add_argument("--log-dir", type=Path)
args = parser.parse_args()
command = ["cargo", "check", "--locked"]
if args.offline:
    command.append("--offline")
clean = {
    key: value for key, value in os.environ.items()
    if key not in {"CXX", "HOST_CXX", "TARGET_CXX"} and not key.startswith("CXX_")
}
host = next(line[6:] for line in subprocess.check_output(
    ["rustc", "-vV"], text=True
).splitlines() if line.startswith("host: "))
keys = [f"CXX_{host}", f"CXX_{host.replace('-', '_').replace('.', '_')}", "HOST_CXX", "CXX"]
receipts = []
if args.log_dir:
    args.log_dir.mkdir(parents=True, exist_ok=True)


def run(label, environment):
    result = subprocess.run(command, env=environment, text=True,
                            stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    if args.log_dir:
        (args.log_dir / f"{label}.log").write_text(result.stdout)
    return result


# cc-rs still owns parsing of nonblank values, including surrounding spaces/flags.
control = run("nonblank-control", dict(clean, CXX=f" {args.compiler} -pthread "))
if control.returncode:
    raise AssertionError(f"nonblank compiler/argument control failed: {control.stdout}")
for index, key in enumerate(keys):
    for blank_name, blank in [("empty", ""), ("whitespace", " \t\n ")]:
        environment = dict(clean)
        # Every lower applicable override names a valid compiler; none may be selected.
        environment.update({lower: args.compiler for lower in keys[index + 1:]})
        environment[key] = blank
        label = f"level-{index}-{blank_name}"
        result = run(label, environment)
        expected = f"Invalid GNU C++ compiler override {key}: value is empty or whitespace"
        if result.returncode == 0 or expected not in result.stdout:
            raise AssertionError(f"{label} failed to reject {key}: {result.stdout}")
        receipts.append({"variable": key, "blank": blank_name,
                         "returncode": result.returncode, "diagnostic": expected})
        print(f"{label}: {expected}", flush=True)
print(json.dumps({"nonblank_argument_control_passed": True, "rejections": receipts}, indent=2))
