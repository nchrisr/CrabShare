#!/usr/bin/env python3
"""Fails when the docs agents rely on point at code that is no longer there.

Checks, in AGENTS.md, CLAUDE.md and docs/**/*.md:
  1. Every file or folder a doc names in backticks exists: `lib/auth/session.ts`, `lib/billing/`,
     or a bare file name that names exactly one file: `invoice-key.ts`.
  2. A line reference is inside the file: `order-total.ts:55` or `:55-70`.
  3. A symbol written just before a file reference is in that file, and on that line if the doc gives
     one: "`buildInvoice`, `lib/billing/invoice.ts:202`". When it has moved, the new line is given.
  4. Relative Markdown links point at files that exist.
  5. Every `owns` pattern in a system card (docs/systems/*.md) matches at least one file, so the
     card's reviewer still wakes for its files.

Skipped: docs/specs/ (work in progress, the code wins), the decisions doc and the changelog (history
may name old files), fenced code blocks, placeholders with < >, site paths like `/sw.js`, files git
ignores (local notes, build output), and any line containing "doc-refs: ignore".

Warnings, which don't fail the check: a bare file name found nowhere in the repo (it may be a file
outside it, like an editor's config), and an `owns` glob that matches nothing yet (it may be watching
for files that don't exist yet, like git hook folders).

Usage: python3 scripts/doc-refs-check.py          Exit 0 when clean, 1 with one line per problem.
Set per project: DOC_REFS_DOCS (glob list of docs to read) and DOC_REFS_SKIP (glob list to skip).
"""
import fnmatch
import os
import re
import subprocess
import sys

DOCS = os.environ.get('DOC_REFS_DOCS', 'AGENTS.md,CLAUDE.md,docs/*.md').split(',')
SKIP = os.environ.get('DOC_REFS_SKIP', 'docs/specs/*,docs/DECISIONS.md,CHANGELOG.md').split(',')
CARDS = os.environ.get('CARDS_DIR', 'docs/systems')
# File extensions that make a backticked word a file reference rather than code like `process.env`.
EXTS = set(os.environ.get(
    'DOC_REFS_EXTS',
    'ts tsx js jsx mjs cjs json md mdx py rs go java kt swift rb php cs c h cc cpp hpp sql sh '
    'yml yaml toml css scss html vue svelte astro txt lock prisma graphql proto tf').split())

root = subprocess.run(['git', 'rev-parse', '--show-toplevel'], capture_output=True, text=True).stdout.strip()
if not root:
    sys.exit('doc-refs-check: not in a git repository')
os.chdir(root)
listed = subprocess.run(['git', 'ls-files', '--cached', '--others', '--exclude-standard'],
                        capture_output=True, text=True).stdout.split('\n')
files = sorted({f for f in listed if f and os.path.isfile(f)})
file_set = set(files)
dirs = {os.path.dirname(f) for f in files}
for d in list(dirs):
    while d:
        d = os.path.dirname(d)
        dirs.add(d)
top_level = {f.split('/')[0] for f in files}
by_name = {}
for f in files:
    by_name.setdefault(os.path.basename(f), []).append(f)

PATH_RE = re.compile(r'^(?:\./)?(?P<path>[\w@.\-\[\]()+/]+?)(?::(?P<a>\d+)(?:-(?P<b>\d+))?)?$')
SYMBOL_RE = re.compile(r'^[A-Za-z_$][\w$]*(?:\.[A-Za-z_$][\w$]*)*(?:\(\))?$')
# Text allowed between a symbol and the file reference it belongs to.
JOIN_RE = re.compile(r'^\s*(?:,|in|at|from|—|-|:|\()?\s*$')
DEF_RE = r'\b(?:function|class|const|let|var|def|fn|type|interface|enum|struct|trait|impl|export|func)\b'

problems = []
warnings = []
_lines = {}


def lines_of(path):
    if path not in _lines:
        try:
            with open(path, encoding='utf-8', errors='replace') as fh:
                _lines[path] = fh.read().splitlines()
        except OSError:
            _lines[path] = []
    return _lines[path]


def skipped(path):
    return any(fnmatch.fnmatchcase(path, g.strip()) for g in SKIP if g.strip())


def resolve(path):
    """Returns (status, files): 'ok' with the matching file(s), 'dir', 'missing' or 'ambiguous'."""
    path = path.rstrip('/')
    if path in file_set:
        return 'ok', [path]
    if path in dirs:
        return 'dir', []
    if '/' not in path:
        hits = by_name.get(path, [])
    else:
        hits = [f for f in files if f.endswith('/' + path)]
        if not hits and any(d.endswith('/' + path) for d in dirs):
            return 'dir', []
    if len(hits) == 1:
        return 'ok', hits
    return ('ambiguous', hits) if hits else ('missing', [])


def ignored(path):
    return subprocess.run(['git', 'check-ignore', '-q', path.rstrip('/')]).returncode == 0


def is_path(token):
    if any(c in token for c in '<>* ') or '://' in token or token.startswith('/'):
        return None
    m = PATH_RE.match(token)
    if not m:
        return None
    path = m.group('path')
    ext = os.path.splitext(path.rstrip('/'))[1][1:]
    if ext in EXTS:
        return m
    # Folder-like: has a slash and starts at a real top-level folder, so `and/or` isn't a path.
    if '/' in path.strip('/') and path.strip('/').split('/')[0] in top_level:
        return m
    return None


def check_symbol(doc, n, symbol, target, a):
    name = symbol.rstrip('()').split('.')[-1]
    text = lines_of(target)
    word = re.compile(r'(?<![\w$])' + re.escape(name) + r'(?![\w$])')
    hits = [i for i, line in enumerate(text, 1) if word.search(line)]
    if not hits:
        problems.append(f'{doc}:{n}: `{symbol}` is not in {target}')
        return
    if a and not word.search(text[a - 1] if a <= len(text) else ''):
        defs = [i for i in hits if re.search(DEF_RE, text[i - 1])]
        now = f'{target}:{defs[0]}' if defs else 'used at ' + ', '.join(f'{target}:{i}' for i in hits[:5])
        problems.append(f'{doc}:{n}: `{symbol}` is no longer at {target}:{a}; now {now}')


def check_doc(doc):
    fenced = False
    for n, line in enumerate(lines_of(doc), 1):
        if line.lstrip().startswith(('```', '~~~')):
            fenced = not fenced
            continue
        if fenced or 'doc-refs: ignore' in line:
            continue
        spans = [(m.start(), m.end(), m.group(1)) for m in re.finditer(r'`([^`\n]+)`', line)]
        for i, (start, end, token) in enumerate(spans):
            m = is_path(token.strip())
            if not m:
                continue
            path, a, b = m.group('path'), m.group('a'), m.group('b')
            status, hits = resolve(path)
            if status == 'missing':
                if ignored(path):
                    continue
                if '/' in path.strip('/'):
                    problems.append(f'{doc}:{n}: `{token}` does not exist')
                else:
                    warnings.append(f'{doc}:{n}: `{token}` is not in the repo (if it lives outside it, '
                                    f'add "doc-refs: ignore" to the line)')
                continue
            if status != 'ok':
                continue
            target = hits[0]
            length = len(lines_of(target))
            a = int(a) if a else None
            if a and (a > length or (b and int(b) > length)):
                problems.append(f'{doc}:{n}: `{token}` is past the end of {target} ({length} lines)')
                continue
            if i and JOIN_RE.match(line[spans[i - 1][1]:start]) and SYMBOL_RE.match(spans[i - 1][2]):
                symbol = spans[i - 1][2]
                if not is_path(symbol):
                    check_symbol(doc, n, symbol, target, a)
        # Relative Markdown links: [text](path) or [text](path#anchor).
        for m in re.finditer(r'\]\(([^)\s]+)\)', line):
            link = m.group(1).split('#')[0]
            if not link or re.match(r'^[a-z]+:', link) or '<' in link or not re.search(r'[./]', link):
                continue
            path = os.path.normpath(os.path.join(os.path.dirname(doc), link))
            if not os.path.exists(path) and not ignored(path):
                problems.append(f'{doc}:{n}: link ({m.group(1)}) does not exist')


def check_cards():
    if not os.path.isdir(CARDS):
        return
    for card in sorted(os.listdir(CARDS)):
        path = os.path.join(CARDS, card)
        if not card.endswith('.md'):
            continue
        text = lines_of(path)
        if not text or text[0] != '---':
            continue
        grab = False
        for n, line in enumerate(text[1:], 2):
            if line == '---':
                break
            if re.match(r'^\w+:', line):
                grab = line.startswith('owns:')
                continue
            if grab and line.startswith('  - '):
                pattern = line[4:].strip().strip('\'"')
                if '<' in pattern:
                    continue
                if any(fnmatch.fnmatchcase(f, pattern) for f in files):
                    continue
                if any(c in pattern for c in '*?['):
                    warnings.append(f'{path}:{n}: owns \'{pattern}\' matches no file yet')
                else:
                    problems.append(f'{path}:{n}: owns \'{pattern}\' does not exist, so this card never '
                                    f'wakes for it')


docs = sorted({f for f in files for g in DOCS if fnmatch.fnmatchcase(f, g.strip()) and f.endswith('.md')})
for doc in docs:
    if not skipped(doc):
        check_doc(doc)
check_cards()

for w in warnings:
    print('warning: ' + w)
for p in problems:
    print(p)
if problems:
    print(f'\n{len(problems)} broken doc reference(s). Fix the doc, or mark a line that is history '
          f'with "doc-refs: ignore".', file=sys.stderr)
    sys.exit(1)
