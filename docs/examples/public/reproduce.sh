#!/usr/bin/env bash
# Download two public whole assemblies and reproduce the documented examples.
# Requires ptero, curl, gzip, and sha256sum or shasum. Bash 3.2 is supported.
set -euo pipefail

work_dir="${1:-pterobacter-example}"
if [ -e "$work_dir" ]; then
    echo "Choose a new directory; refusing to overwrite: $work_dir" >&2
    exit 2
fi
command -v ptero >/dev/null
command -v curl >/dev/null
command -v gzip >/dev/null
if command -v sha256sum >/dev/null; then
    hash_command=sha256sum
else
    command -v shasum >/dev/null
    hash_command=shasum
fi
mkdir -- "$work_dir"
cd -- "$work_dir"
ptero --version > software-version.txt

curl --fail --location --retry 3 \
  'https://ftp.ncbi.nlm.nih.gov/genomes/all/GCA/013/738/375/GCA_013738375.1_ASM1373837v1/GCA_013738375.1_ASM1373837v1_genomic.fna.gz' \
  --output GCA_013738375.1.fna.gz
curl --fail --location --retry 3 \
  'https://ftp.ncbi.nlm.nih.gov/genomes/all/GCA/043/966/805/GCA_043966805.1_ASM4396680v1/GCA_043966805.1_ASM4396680v1_genomic.fna.gz' \
  --output GCA_043966805.1.fna.gz
gzip -dc GCA_013738375.1.fna.gz > GCA_013738375.1.fna
gzip -dc GCA_043966805.1.fna.gz > GCA_043966805.1.fna
cat > inputs.sha256 <<'HASHES'
603ed679732e01cb7d1a06ec73d89a53fc12b6f6b22f071df5dee866cdf982b9  GCA_013738375.1.fna
34f4924c5954e5c424ecdb5476386a3f5ddbfc7d09aa6be2f32c049806b177a0  GCA_043966805.1.fna
HASHES
if [ "$hash_command" = sha256sum ]; then
    sha256sum --check inputs.sha256
else
    shasum -a 256 --check inputs.sha256
fi

# Keep this demonstration's database and alignment files together.
ptero database install 2026-10-15 --database-dir ./database
ptero database verify 2026-10-15 --database-dir ./database
ptero --database-dir ./database --threads 2 --sample-name RHBSTW-00333 \
  --anim-output single-alignments --output single.json --report-dir single \
  GCA_013738375.1.fna
printf 'sample\tpath\nRHBSTW-00333\tGCA_013738375.1.fna\nECC-219\tGCA_043966805.1.fna\n' > samples.tsv
ptero --database-dir ./database --threads 2 --jobs 1 --sample-sheet samples.tsv \
  --anim-output batch-alignments --output batch.json --report-dir batch
printf '\nOpen %s/single/report.md or %s/batch/report.md\n' "$work_dir" "$work_dir"
