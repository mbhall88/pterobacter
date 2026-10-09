# Test fixtures

## `v1-species-assigned/`

One saved-evidence record from the frozen 0.2.0 development evidence, with the v1 call that release reported for it.

- `evidence.json`: the saved evidence. It keeps all 140 ranked reference rows exactly as frozen, plus the provenance fields the decision reads. It leaves out the query path, the QC block and every path into the research workspace.
- `frozen-v1-call.json`: the species call from the frozen 0.2.0 report for the same evidence. Tests compare against this file, never against output from this implementation.

Source pins (SHA-256 of the frozen files these were derived from):

| Frozen file | SHA-256 |
|---|---|
| 0.2.0 saved evidence | `f3f392ff48941e88b856b84a7ea5bfdfbdc8d2083bd43a2ae2d3d883b309b5c9` |
| 0.2.0 classification report for it | `21cc04c74d8eecc7be1ccac9a52f05c9eb7999e7d09392e9d54b6bcaff473cca` |

The derivation script checks both pins before writing anything, and refuses to write a file that contains a workspace path. The script and its run log stay in the research workspace.

This is development evidence that was already exposed. Passing it shows the implementation reproduces a frozen result. It says nothing about accuracy on new data.

## `reference-v2/`

A synthetic version 2 reference package for the authentication tests. It carries no real reference data. See [its README](reference-v2/README.md) for its contents, derivation and limits.
