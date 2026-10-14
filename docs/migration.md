# Pterobacter naming

The project and Cargo package are now Pterobacter (`pterobacter`). Use `ptero` for the short command or `pterobacter` if another installed tool already uses `ptero`. Both names run the same implementation. The optional development executable is `pterobacter-dev` and still requires the `dev` feature.

A new installation provides `ptero` and `pterobacter`. It does not remove `tero` or `terobacter` from an earlier installation. Update your scripts to the new command names and check `ptero --version` and `command -v ptero` after installing.

The internal Rust library is still named `terobacter`; it is not a separately published library. Machine-readable schema strings such as `terobacter.assembly-call/1`, reference identifiers and MLST engine identities retain their original spelling. Existing fixtures, reports and evaluated-build records also retain the names used when they were created. The rename changes the interface and display name, while preserving those format contracts and scientific rules.

The GitHub repository is still `mbhall88/terobacter` while the pending documentation and build PRs are reviewed. Clone links and repository metadata point to that existing location. A later repository rename must update those URLs, the Pages prefix and release configuration together.
