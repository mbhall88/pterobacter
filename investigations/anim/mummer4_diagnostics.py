"""Fixed addendum thread/index/runtime checks; all native work uses one ledger."""
import argparse
from concurrent.futures import ThreadPoolExecutor
import json
import os
import pathlib
import types
from measure import Runner


def configuration(args, threads=1):
    return types.SimpleNamespace(fixtures=args.fixtures, budget_ledger=args.run/'outputs/budget.json',
        pair_seconds=90, workers=8//threads, threads=threads, memory_kib=6*1024*1024,
        sample_pairs=True, nucmer=args.cli/'nucmer', delta_filter=args.cli/'delta-filter', mummer_probe=args.probe)


def diagnostics(args):
    manifest=json.loads((args.fixtures/'manifest.json').read_text())
    subset=[c for c in manifest['cases'] if c['id'] in ('fragmented','repeat_reference','genome_5mb')]
    # Repeat1/2 are independent executable invocations. No options change.
    for candidate in ('cli-mummer4','embedded-mummer4'):
        for threads in (1,2,4):
            runner=Runner(configuration(args,threads))
            for repetition in (1,2):
                destination=args.run/'outputs/thread-checks'/candidate/f't{threads}-rep{repetition}'
                if destination.exists():
                    print('retained existing pass',destination,flush=True)
                    continue
                with ThreadPoolExecutor(max_workers=runner.workers) as pool:
                    results=list(pool.map(lambda case:runner.pair(candidate,case,destination),subset))
                print(candidate,threads,repetition,[r['fixture'] for r in results],flush=True)
    # Two independent objects in one executable. Each uses one query thread;
    # constructor/filter RNG sections serialize. Two total affinity CPUs.
    runner=Runner(configuration(args,2))
    timed=runner.timed
    def concurrent(argv,*pos,**kw):
        return timed([*argv,'concurrent'],*pos,**kw)
    runner.timed=concurrent
    for case in subset:
        runner.pair('embedded-mummer4',case,args.run/'outputs/concurrent-objects')


def indexes(args):
    runner=Runner(configuration(args))
    case=next(c for c in json.loads((args.fixtures/'manifest.json').read_text())['cases'] if c['id']=='genome_5mb')
    indexes={}
    root=args.run/'outputs/index-checks'
    root.mkdir(parents=True,exist_ok=False)
    for role,key in [('R','reference'),('Q','query')]:
        reference=args.fixtures/case[key]
        prefix=root/role
        receipt=runner.timed([args.cli/'nucmer','--mum','--threads','1','--save',prefix,reference],root,'construct-'+role,root/(role+'.stdout'),90)
        if receipt['exit_code']:
            raise RuntimeError('index construction failed; retain partial evidence')
        (root/(role+'-construction.json')).write_text(json.dumps(receipt,indent=2)+'\n')
        indexes[str(reference)]=prefix
    # Setup-inclusive --save pair, then warm --load pair. Roles stay bound to
    # the native reference in each orientation, never a shared swapped index.
    for mode in ('save','load'):
        timed=runner.timed
        def indexed(argv,*pos,**kw):
            if pathlib.Path(argv[0])==args.cli/'nucmer':
                argv=[*argv[:-2],'--'+mode,indexes[str(argv[-2])],*argv[-2:]]
            return timed(argv,*pos,**kw)
        runner.timed=indexed
        runner.pair('cli-mummer4',case,root/mode)
        runner.timed=timed


def runtime(args):
    runner=Runner(configuration(args))
    cases=json.loads((args.fixtures/'manifest.json').read_text())['cases']
    subset=[c for c in cases if c['id'] in ('identical','no_alignment')]
    root=args.run/'outputs/runtime-checks'
    # PATH changed only for these sequential isolated calls.
    previous=os.environ.get('PATH')
    os.environ['PATH']=''
    try:
        for case in subset:runner.pair('embedded-mummer4',case,root/'empty-path')
    finally:
        if previous is None:os.environ.pop('PATH',None)
        else:os.environ['PATH']=previous
    case=subset[0]
    directory=root/'output-failure'
    directory.mkdir(parents=True)
    receipt=runner.timed([args.probe,args.fixtures/case['reference'],args.fixtures/case['query'],directory/'missing'/'x','1'],directory,'failure',directory/'stdout',90)
    (directory/'result.json').write_text(json.dumps(receipt,indent=2)+'\n')


if __name__=='__main__':
    parser=argparse.ArgumentParser()
    for name in ('run','fixtures','cli','probe'):parser.add_argument('--'+name,type=pathlib.Path,required=True)
    parser.add_argument('phase',choices=['threads','indexes','runtime'])
    args=parser.parse_args()
    dict(threads=diagnostics,indexes=indexes,runtime=runtime)[args.phase](args)
