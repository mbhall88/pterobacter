# Containers

[![Container version](https://ghcr-badge.egpl.dev/mbhall88/pterobacter/latest_tag?label=GHCR)](https://github.com/mbhall88/pterobacter/pkgs/container/pterobacter) [![Container downloads](https://ghcr-badge.elias.eu.org/shield/mbhall88/pterobacter/pterobacter)](https://github.com/mbhall88/pterobacter/pkgs/container/pterobacter)

Find the latest release tag and download its Linux amd64 image from GitHub Container Registry:

```sh
release_url=$(curl --fail --silent --show-error --location \
  --output /dev/null --write-out '%{url_effective}' \
  https://github.com/mbhall88/pterobacter/releases/latest)
PTERO_VERSION=${release_url##*/}
docker pull --platform linux/amd64 "ghcr.io/mbhall88/pterobacter:$PTERO_VERSION"
docker run --rm --platform linux/amd64 "ghcr.io/mbhall88/pterobacter:$PTERO_VERSION" --version
```

The tested platform is Linux x86_64. On an Apple Silicon Mac, Docker can emulate it with `--platform linux/amd64`; native Linux ARM support has not been validated.

The image contains both `ptero` and `pterobacter`, including the alignment and MLST engines. It runs as an unprivileged user and contains no compiler or reference database. The commands below reuse `PTERO_VERSION` from the first step; the registry publishes release tags rather than a `latest` alias.

## Download the database once

Use a host directory for the database so it survives removal of the container. The commands below use your own user ID so files belong to you:

```sh
mkdir -p database

docker run --rm --platform linux/amd64 \
  --user "$(id -u):$(id -g)" \
  -v "$PWD/database:/database" \
  "ghcr.io/mbhall88/pterobacter:$PTERO_VERSION" database install 2026-10-15
```

This installs the reference sketches, alignment sequences and MLST data. Add `--no-mlst` to opt out of downloading MLST data. See [database management](databases.md) for verification and storage requirements. Use local storage with atomic rename support; a container does not change the capabilities of an underlying network filesystem.

## Classify an assembly

Put `isolate.fasta` in the current directory, then run:

```sh
docker run --rm --platform linux/amd64 \
  --user "$(id -u):$(id -g)" \
  -v "$PWD/database:/database" \
  -v "$PWD:/work" \
  "ghcr.io/mbhall88/pterobacter:$PTERO_VERSION" --threads 2 \
  --output isolate.json --report-dir isolate-report isolate.fasta
```

The report and alignment evidence stay in the mounted working directory. Choose new output names for another run. ANIm and MLST are enabled by default; use `--no-anim` or `--no-mlst` to skip either. See [running assemblies](running.md) and [reading results](results.md).

Without arguments, the image shows command help. A small init process starts Pterobacter and forwards signals to it. Keep that entrypoint for normal use; it also gives alignment workers the parent process they require. To use the longer command name:

```sh
docker run --rm --entrypoint /usr/bin/tini "ghcr.io/mbhall88/pterobacter:$PTERO_VERSION" -- pterobacter --help
```

## Build from source

```sh
docker build --platform linux/amd64 -t pterobacter:local .
docker run --rm pterobacter:local --version
```

## Release provenance

The Distribution workflow tests the image with synthetic assemblies, including ANIm and MLST, without network access and with a read-only image filesystem. It retains the tested image and its receipt before publication. The approved release workflow uploads complete corresponding source before pushing that same image to GHCR; it refuses to replace a version tag whose image differs. PR checks never push images.

The combined executable is GPL-3.0-or-later. Notices are in `/usr/share/doc/pterobacter/`; complete corresponding source is supplied with the matching [GitHub release](https://github.com/mbhall88/pterobacter/releases). The image uses Debian runtime libraries, which retain their own licences. The receipt records the source commit, image identity, platform and archive checksum. Use a version tag or registry digest when recording an analysis.
