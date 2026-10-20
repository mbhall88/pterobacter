# Containers

The Docker image is prepared for GHCR but has not been published yet. Until the first release, build it from a source checkout:

```sh
docker build --platform linux/amd64 -t pterobacter:local .
docker run --rm pterobacter:local --version
```

The tested platform is Linux x86_64. On an Apple Silicon Mac, Docker can emulate it with `--platform linux/amd64`; native Linux ARM support has not been validated.

The image contains both `ptero` and `pterobacter`, including the alignment and MLST engines. It runs as an unprivileged user and contains no compiler or reference database. Versioned release images will be at `ghcr.io/mbhall88/pterobacter:VERSION`.

## Download the database once

Use a host directory for the database so it survives removal of the container. The commands below use your own user ID so files belong to you:

```sh
mkdir -p database

docker run --rm --platform linux/amd64 \
  --user "$(id -u):$(id -g)" \
  -v "$PWD/database:/database" \
  pterobacter:local database install 2026-10-15
```

This installs the reference sketches, alignment sequences and MLST data. Add `--no-mlst` to opt out of downloading MLST data. See [database management](databases.md) for verification and storage requirements. Use local storage with atomic rename support; a container does not change the capabilities of an underlying network filesystem.

## Classify an assembly

Put `isolate.fasta` in the current directory, then run:

```sh
docker run --rm --platform linux/amd64 \
  --user "$(id -u):$(id -g)" \
  -v "$PWD/database:/database" \
  -v "$PWD:/work" \
  pterobacter:local --threads 2 \
  --output isolate.json --report-dir isolate-report isolate.fasta
```

The report and alignment evidence stay in the mounted working directory. Choose new output names for another run. ANIm and MLST are enabled by default; use `--no-anim` or `--no-mlst` to skip either. See [running assemblies](running.md) and [reading results](results.md).

To use the longer command name, pass `--entrypoint pterobacter` before the image name. Without arguments, the image shows command help.

## Release provenance

The Distribution workflow tests the image with synthetic assemblies, including ANIm and MLST, without network access and with a read-only image filesystem. It retains the tested image and its receipt before publication. The approved release workflow uploads complete corresponding source before pushing that same image to GHCR; it refuses to replace a version tag whose image differs. PR checks never push images.

The combined executable is GPL-3.0-or-later. Notices are in `/usr/share/doc/pterobacter/`; complete corresponding source is supplied with the matching [GitHub release](https://github.com/mbhall88/pterobacter/releases). The image uses Debian runtime libraries, which retain their own licences. The receipt records the source commit, image identity, platform and archive checksum. Use a version tag or registry digest when recording an analysis.
