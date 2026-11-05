#!/usr/bin/env python3
# Sol against Opus on the same judge pools: solcmp.py POOL... (judge.POOL.jsonl and judge.POOL.sol.jsonl), 2026-11-05.
import json,sys,statistics as st,itertools,collections,re
def load(f): return {(r['task'],r['harness'],r['run']):r['overall'] for r in map(json.loads,open(f))}
def rank(xs):
    o=sorted(range(len(xs)),key=lambda i:xs[i]); r=[0]*len(xs); i=0
    while i<len(o):
        j=i
        while j+1<len(o) and xs[o[j+1]]==xs[o[i]]: j+=1
        for k in range(i,j+1): r[o[k]]=(i+j)/2
        i=j+1
    return r
def pearson(a,b):
    ma,mb=st.mean(a),st.mean(b); return sum((x-ma)*(y-mb) for x,y in zip(a,b))/((sum((x-ma)**2 for x in a)*sum((y-mb)**2 for y in b))**.5 or 1)
allo,alls=[],[]; agree=dis=ties=0
for pool in sys.argv[1:]:
    O,S=load(f'judge.{pool}.jsonl'),load(f'judge.{pool}.sol.jsonl')
    k=[x for x in O if x in S]; o=[O[x] for x in k]; s=[S[x] for x in k]
    allo+=o; alls+=s
    # pairwise order within a task
    a=d=t=0
    for t_ in {x[0] for x in k}:
        kk=[x for x in k if x[0]==t_]
        for x,y in itertools.combinations(kk,2):
            do,ds=O[x]-O[y],S[x]-S[y]
            if do==0 or ds==0: t+=1
            elif (do>0)==(ds>0): a+=1
            else: d+=1
    agree+=a; dis+=d; ties+=t
    # arm means (per-task means averaged)
    A=lambda h: re.sub(r'-v\d+@','@',h)
    arm=lambda D: {h: st.mean(st.mean(D[x] for x in k if A(x[1])==h and x[0]==t_) for t_ in {x[0] for x in k if A(x[1])==h}) for h in {A(x[1]) for x in k}}
    ao,as_=arm(O),arm(S)
    print(f"{pool}: n={len(k)} mean opus {st.mean(o):.2f} sol {st.mean(s):.2f} r={pearson(o,s):.2f} spearman={pearson(rank(o),rank(s)):.2f} pairs agree {a} disagree {d} tie {t}")
    for h in sorted(ao, key=lambda h:-ao[h]): print(f"   {h[:48]:48} opus {ao[h]:.2f} sol {as_[h]:.2f}")
print(f"ALL n={len(allo)} r={pearson(allo,alls):.2f} spearman={pearson(rank(allo),rank(alls)):.2f} pairs agree {agree} disagree {dis} tie {ties} offset {st.mean(alls)-st.mean(allo):+.2f}")
