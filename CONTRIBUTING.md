# Contributing

Use the repository checks when changing the implementation. Installation and routine analysis are covered by [the user guide](docs/usage.md).

`just` is optional. From a provisioned checkout:

```sh
just test
just lint
just install-check
cargo fmt --all -- --check
```

`just test` checks the default and all-feature builds. `just lint` runs clippy with warnings denied. `just install-check` installs in a temporary directory and checks the two public commands with synthetic smoke data. Distribution remains on hold; these are contributor checks, not release publication.

Without `just`, run `cargo test --all-targets --no-fail-fast`, `cargo test --all-targets --all-features --no-fail-fast` and `cargo clippy --all-features --all-targets -- -D warnings`. Tests use synthetic fixtures and retained development evidence; do not add private data or research paths.

See [development commands and source layout](docs/development.md), [AGENTS.md](AGENTS.md) and [CONTEXT.md](CONTEXT.md) for implementation guidance. Public commands have no subcommands; evaluation operations stay in the optional development binary. Changes to scientific rules, references, defaults or output schemas require their own scoped review and validation.
