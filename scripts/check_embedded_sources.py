"""Verify the packaged engines differ from the reviewed baseline only in namespaces."""
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def canonical(text, component, filename):
    text = text.replace('crate::embedded_skani::', 'crate::')
    text = text.replace('crate::embedded_partitions::', 'crate::')
    text = text.replace('use crate::*;', 'use partitions::*;') if component == 'skani' else text
    if component == 'skani' and filename == 'lib.rs':
        # The upstream command parser is not a Pterobacter input path. Sketch DB
        # I/O is still compiled; previously its feature was tied to that parser.
        text = text.replace('#[cfg(feature = "cli")]\n', '')
        text = text.replace('pub mod cli;\n', '').replace('pub mod parse;\n', '')
    if component == 'partitions':
        text = text.replace('#[macro_use]\nmod disjoint_sets;', 'mod disjoint_sets;')
        text = text.replace('#[macro_use]\npub mod partition_vec;', 'pub mod partition_vec;')
        # These traits were enabled by the original partitions defaults.
        text = text.replace('#[cfg(feature = "rayon")]\n', '')
        text = text.replace('#[cfg(feature = "proptest")]\n', '')
        text = text.replace('feature = "embedded-partitions-compact"', 'feature = "compact"')
        text = text.replace('pub use self::disjoint_sets::', 'pub use disjoint_sets::')
        text = text.replace('pub use self::partition_map::', 'pub use partition_map::')
    # Equivalent single-condition cfg and doctest paths for the containing crate.
    text = text.replace('cfg(any(target_arch = "x86_64"))', 'cfg(target_arch = "x86_64")')
    text = text.replace('extern crate terobacter as partitions;', 'extern crate partitions;')
    text = text.replace('terobacter::embedded_partitions::PartitionVec', 'partitions::PartitionVec')
    return text.encode()


def check():
    baseline = json.loads((ROOT / 'scripts/embedded-source-baseline.json').read_text())
    paths = {str(p.relative_to(ROOT)) for component in ['skani', 'partitions']
             for p in (ROOT / 'vendor' / component / 'src').rglob('*.rs')}
    if paths != set(baseline['canonical_sha256']):
        raise ValueError('Embedded source file inventory changed')
    for name, expected in baseline['canonical_sha256'].items():
        p = ROOT / name
        actual = hashlib.sha256(canonical(p.read_text(), Path(name).parts[1], p.name)).hexdigest()
        if actual != expected:
            raise ValueError(f'Non-mechanical embedded source change: {name}')
    print(f'{len(paths)} embedded source files match the reviewed implementation after namespace adaptation')


if __name__ == '__main__':
    check()
