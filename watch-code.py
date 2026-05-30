#!/usr/bin/env python3
"""ZenvX live code view: polls the project and prints a colored diff of every change."""
import os, time, difflib, glob

ROOT = os.path.dirname(os.path.abspath(__file__))
PATTERNS = ['**/*.rs', '**/*.toml', '**/*.c', '**/*.S', '**/*.ld', '**/*.py', '**/*.sh', 'Makefile']
SKIP = ('/target/', '/.git/', '/isodir/')
G, R, Y, C, B0 = '\033[32m', '\033[31m', '\033[1;33m', '\033[1;36m', '\033[0m'


def project_files():
    out = set()
    for p in PATTERNS:
        for f in glob.glob(os.path.join(ROOT, p), recursive=True):
            if not any(s in f for s in SKIP):
                out.add(f)
    return out


def read(f):
    try:
        return open(f, encoding='utf-8', errors='replace').read()
    except OSError:
        return ''


snap = {}
print(f'{C}== ZenvX live code view =={B0}  watching {ROOT}\n')
for f in sorted(project_files()):
    snap[f] = read(f)
    rel = os.path.relpath(f, ROOT)
    print(f'{C}[ existing ] {rel}{B0}')
print('\nwaiting for edits...\n')

while True:
    for f in sorted(project_files()):
        new, old = read(f), snap.get(f)
        if new == old:
            continue
        rel, ts = os.path.relpath(f, ROOT), time.strftime('%H:%M:%S')
        if old is None:
            print(f'{C}[{ts}] + NEW {rel}{B0}')
            for ln in new.splitlines():
                print(f'  {G}+{B0} {ln}')
        else:
            print(f'{Y}[{ts}] ~ EDIT {rel}{B0}')
            for d in difflib.unified_diff(old.splitlines(), new.splitlines(), lineterm='', n=1):
                if d.startswith('+') and not d.startswith('+++'):
                    print(f'  {G}{d}{B0}')
                elif d.startswith('-') and not d.startswith('---'):
                    print(f'  {R}{d}{B0}')
        snap[f] = new
    for f in [f for f in snap if not os.path.exists(f)]:
        print(f'{R}[-] DELETED {os.path.relpath(f, ROOT)}{B0}')
        del snap[f]
    time.sleep(1)
