# Synthetic orientation fixture

The decoded FASTA is the existing query from tests/skani_query_label.rs: 200003 bases from common::dna(200003, 42), a >synthetic header, one sequence line and a final newline. These files encode those exact bytes; no study sequence is included. The test independently regenerates the uncompressed sequence.

Decoded SHA-256: 2fd361595857423ea69b4955672e2b969116799c400932f50090e9eaa91821bf.

| File | SHA-256 |
| --- | --- |
| query.fa.gz | ca587c1fac624970a2a3c65e746ae559e438ac71a73425dfddff3d2c24d4f6dd |
| query.fa.bz2 | d0ec59e91d0bbcdf62fc4cede45e68fb448d68bdea88bfbd7b6f50b9456707ca |
| query.fa.xz | 0d37fc268c71e5596c8192c76c1e7b4ed783a240c63596ecb91aca049725f2de |
| query.fa.zst | 590144201bc61193393cfbf06747c076206bcd47576e9dd491910e4a2825d207 |

Generation uses Python gzip with mtime 0, bz2 and lzma defaults, and zstd -q -c. Full generation commands are retained with the issue 50 research evidence.
