"""Run a CI command without any cc-rs C++ compiler override."""
import os
import sys

clean = {
    key: value for key, value in os.environ.items()
    if key not in {"CXX", "HOST_CXX", "TARGET_CXX"} and not key.startswith("CXX_")
}
os.execvpe(sys.argv[1], sys.argv[1:], clean)
