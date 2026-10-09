PROJECT := "terobacter"

# regenerate redistribution notices with cargo-bundle-licenses 4.2.0
licenses:
    python3 scripts/bundle_licenses.py

# reject changes to the locked dependency notices without rewriting files
licenses-check:
    python3 scripts/bundle_licenses.py --check

# run clippy to check for linting issues
lint:
    cargo clippy --all-features --all-targets -- -D warnings

# run all tests, first as a default build and then with the development feature
test:
    cargo test -v --all-targets --no-fail-fast
    cargo test -v --all-targets --all-features --no-fail-fast

# check that a default install exposes only the public commands
install-check:
    #!/usr/bin/env bash
    set -euo pipefail
    root="$(mktemp -d)"
    trap 'rm -rf "$root"' EXIT
    cargo install --path . --locked --quiet --root "$root"
    installed="$(ls "$root/bin" | sort | tr '\n' ' ')"
    if [ "$installed" != "tero terobacter " ]; then
        echo "a default install should expose only tero and terobacter, found: $installed" >&2
        exit 1
    fi
    "$root/bin/tero" --version
    "$root/bin/terobacter" --version

# get coverage with tarpaulin
coverage:
    cargo tarpaulin -t 300 --all-features --out Xml -- --test-threads 1
