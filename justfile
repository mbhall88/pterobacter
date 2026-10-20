PROJECT := "pterobacter"

# regenerate redistribution notices with cargo-bundle-licenses 4.2.0
licenses:
    python3 scripts/bundle_licenses.py

# reject changes to the locked dependency notices without rewriting files
licenses-check:
    python3 scripts/bundle_licenses.py --check

# run clippy to check for linting issues
lint:
    cargo clippy --all-features --all-targets -- -D warnings

# run tests serially so subprocess deadline fixtures do not compete for CPU
# test bodies that exercise concurrency still spawn their own workers
test:
    python3 scripts/check_embedded_sources.py
    python3 -m unittest discover -s scripts -p 'test_distribution.py'
    python3 -m unittest discover -s scripts -p 'test_release_assets.py'
    python3 -m unittest discover -s scripts -p 'test_prepare_source_archive.py'
    python3 -m unittest discover -s scripts -p 'test_native_filter.py'
    cargo test -v --all-targets --no-fail-fast -- --test-threads=1
    cargo test -v --all-targets --all-features --no-fail-fast -- --test-threads=1
    cargo test --locked --doc
    cargo test --locked --all-features --doc

# check that a default install exposes only the public commands, then smoke-test them
install-check:
    #!/usr/bin/env bash
    set -euo pipefail
    root="$(mktemp -d)"
    trap 'rm -rf "$root"' EXIT
    cargo install --path . --locked --quiet --root "$root"
    installed="$(ls "$root/bin" | sort | tr '\n' ' ')"
    if [ "$installed" != "ptero pterobacter " ]; then
        echo "a default install should expose only ptero and pterobacter, found: $installed" >&2
        exit 1
    fi
    "$root/bin/ptero" --version
    "$root/bin/pterobacter" --version
    TEROBACTER_INSTALLED_BIN="$root/bin" cargo test --locked --test installed_smoke -- --ignored

# get coverage with tarpaulin
coverage:
    cargo tarpaulin -t 300 --all-features --out Xml -- --test-threads 1
