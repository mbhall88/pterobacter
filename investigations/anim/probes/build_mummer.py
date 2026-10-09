"""Linux-only development embedding experiment; supplied source copy is immutable."""
import pathlib
import subprocess
import sys

source, out = map(pathlib.Path, sys.argv[1:])

def run(args):
    subprocess.run([str(x) for x in args], check=True)

def localize(obj, exported):
    # Keep the entry point public; isolate legacy stage globals/helper functions.
    symbols = subprocess.check_output(['nm', '--defined-only', obj], text=True)
    names = [row.split()[-1] for row in symbols.splitlines()
             if len(row.split()) == 3 and row.split()[1] in 'TBDR'
             and row.split()[-1] != exported]
    listing = obj.with_suffix('.localize')
    listing.write_text('\n'.join(names) + '\n')
    run(['objcopy', '--localize-symbols=' + str(listing), obj])

objects = []
tigr = source / 'src/tigr'
for stage in ['prenuc', 'mgaps', 'postnuc', 'delta-filter']:
    entry = stage.replace('-', '_') + '_main'
    obj = out / (stage + '.o')
    run(['g++', '-O3', '-fPIC', '-Dmain=' + entry, '-c', tigr / (stage + '.cc'), '-o', obj])
    # C++ main renamed by macro has a mangled name.
    localize(obj, '_Z' + str(len(entry)) + entry + 'iPPc')
    objects.append(obj)
for shared in ['tigrinc', 'delta', 'sw_align']:
    obj = out / (shared + '.o')
    run(['g++', '-O3', '-fPIC', '-c', tigr / (shared + '.cc'), '-o', obj])
    objects.append(obj)
kurtz = source / 'src/kurtz'
# Build every C source from scratch rather than use distribution archives.
mm_objects = []
for folder in ['libbasedir', 'streesrc', 'mm3src']:
    names = {
        'libbasedir': 'cleanMUMcand clock mapfile multiseq procopt safescpy seterror space',
        'streesrc': 'construct access scanpref linkloc depthtab ex2leav dfs overmax oversucc addleafcount iterator',
        'mm3src': 'maxmat3 maxmatopt maxmatinp findmumcand findmaxmat procmaxmat',
    }[folder].split()
    for name in names:
        path = kurtz / folder / (name + '.c')
        obj = out / (folder + '_' + path.stem + '.o')
        args = ['gcc', '-O3', '-fPIC', '-DSTREEHUGE', '-I' + str(kurtz / 'libbasedir'), '-I' + str(kurtz / 'streesrc')]
        if path.name == 'maxmat3.c':
            args.append('-Dmain=mummer_main')
        run(args + ['-c', path, '-o', obj])
        mm_objects.append(obj)
mm_combined = out / 'mummer.o'
run(['ld', '-r', *mm_objects, '-o', mm_combined])
localize(mm_combined, 'mummer_main')
objects.append(mm_combined)
bridge = out / 'bridge.cc'
bridge.write_text('''#include <cstring>
int prenuc_main(int,char**); int mgaps_main(int,char**); int postnuc_main(int,char**); int delta_filter_main(int,char**);
extern "C" int mummer_main(int,char**);
extern "C" int mummer_stage(const char *stage,int argc,char **argv) {
 if (!strcmp(stage,"prenuc")) return prenuc_main(argc,argv);
 if (!strcmp(stage,"mummer")) return mummer_main(argc,argv);
 if (!strcmp(stage,"mgaps")) return mgaps_main(argc,argv);
 if (!strcmp(stage,"postnuc")) return postnuc_main(argc,argv);
 if (!strcmp(stage,"delta-filter")) return delta_filter_main(argc,argv);
 return 64;
}
''')
obj = out / 'bridge.o'
run(['g++', '-O3', '-fPIC', '-c', bridge, '-o', obj])
objects.append(obj)
run(['ar', 'rcs', out / 'libembedded_mummer3.a', *objects])
