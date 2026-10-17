"""Draft THIRD-PARTY-NOTICES.txt from `cargo metadata` output + license files in ~/.cargo/registry.

Usage (build_installer.ps1 runs this before building the installer):
  cargo metadata --format-version 1 --offline --locked --filter-platform x86_64-pc-windows-msvc > meta_app.json
  (cd installer && cargo metadata ... > meta_inst.json)
  python -I gen_notices.py meta_app.json meta_inst.json THIRD-PARTY-NOTICES.txt

Only crates linked into the binaries are listed (build scripts, proc-macros and their deps are skipped).
For "A OR B" crates the MIT text is preferred (we elect MIT where offered).
"""
import hashlib, json, os, re, sys

LIC_RE = re.compile(r'^(LICEN[CS]E|COPYING|COPYRIGHT|NOTICE|UNLICENSE|LICENSE-THIRD-PARTY)', re.I)
EXTRA = {  # license files outside the crate root or with unusual names
    'epaint_default_fonts': ['fonts/OFL.txt', 'fonts/UFL.txt', 'fonts/Hack-Regular.txt', 'fonts/emoji-icon-font-mit-license.txt'],
    'ring': ['LICENSE', 'LICENSE-BoringSSL', 'LICENSE-other-bits'],
    'encoding_rs': ['COPYRIGHT', 'LICENSE-MIT', 'LICENSE-WHATWG'],
}
MIT_TEMPLATE = """MIT License

Copyright (c) {holder}

Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated documentation files (the "Software"), to deal in the Software without restriction, including without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
"""


def runtime_packages(path):
    m = json.load(open(path, encoding='utf-8'))
    pkgs = {p['id']: p for p in m['packages']}
    nodes = {n['id']: n for n in m['resolve']['nodes']}
    is_pm = lambda pid: any('proc-macro' in t['kind'] for t in pkgs[pid]['targets'])
    root, seen, stack = m['resolve']['root'], set(), [m['resolve']['root']]
    while stack:
        nid = stack.pop()
        if nid in seen:
            continue
        seen.add(nid)
        for d in nodes[nid]['deps']:
            if any(k['kind'] is None for k in d['dep_kinds']) and not is_pm(d['pkg']):
                stack.append(d['pkg'])
    seen.discard(root)
    return {(pkgs[p]['name'], pkgs[p]['version']): pkgs[p] for p in seen}


def license_files(p):
    root = os.path.dirname(p['manifest_path'])
    names = EXTRA.get(p['name']) or sorted(f for f in os.listdir(root) if LIC_RE.match(f) and os.path.isfile(os.path.join(root, f)))
    lic = p.get('license') or ''
    if ' OR ' in lic or '/' in lic:  # dual/multi licence: elect MIT if its file exists
        mit = [f for f in names if re.search(r'MIT', f, re.I)]
        keep = [f for f in names if re.search(r'COPYRIGHT|NOTICE|THIRD', f, re.I)]
        if mit:
            names = mit + keep
    return [(f, open(os.path.join(root, f), encoding='utf-8', errors='replace').read().strip()) for f in names if os.path.isfile(os.path.join(root, f))]


def main(app_meta, inst_meta, out_path):
    global APACHE_TEXT
    reg = os.path.dirname(os.path.dirname(json.load(open(app_meta, encoding='utf-8'))['packages'][0]['manifest_path']))
    APACHE_TEXT = open(os.path.join(reg, next(d for d in os.listdir(reg) if d.startswith('serde-1.')), 'LICENSE-APACHE'), encoding='utf-8').read()
    pk = runtime_packages(app_meta)
    pk.update(runtime_packages(inst_meta))
    blocks, missing = {}, []
    for (name, ver), p in sorted(pk.items()):
        files = license_files(p)
        if not files:
            holder = ', '.join(a.split('<')[0].strip() for a in p.get('authors') or []) or f'the {name} developers'
            lic = p.get('license') or ''
            if 'MIT' in lic:
                gen = MIT_TEMPLATE.format(holder=holder)
            elif 'Apache-2.0' in lic:
                gen = f'Copyright (c) {holder}\n\n' + APACHE_TEXT
            else:
                gen = f'Licensed under {lic}; see https://spdx.org/licenses/ (no notice required in binary form)'
            files = [('(generated - verify copyright line against upstream repository)', gen)]
            missing.append(f'{name} {ver}')
        text = '\n\n'.join(f'--- {f} ---\n{t}' for f, t in files)
        key = hashlib.sha256(text.encode()).hexdigest()
        blocks.setdefault(key, (text, []))[1].append(f'{name} {ver} ({p.get("license")}) {p.get("repository") or ""}'.strip())
    with open(out_path, 'w', encoding='utf-8', newline='\r\n') as out:
        out.write('THIRD-PARTY SOFTWARE NOTICES - Loonar Compass\n\n'
                  'Loonar Compass and its installer include the open-source components listed below. Each component is\n'
                  'licensed under its own licence (reproduced below); the Loonar Compass licence does not restrict the\n'
                  'rights those licences grant. Where a component is offered under a choice of licences, Loonar elects MIT.\n\n'
                  'Rust standard library (std, core, alloc): MIT OR Apache-2.0, Copyright (c) The Rust Project Developers.\n'
                  'Fonts embedded by egui (epaint_default_fonts): Ubuntu Light (Ubuntu Font Licence 1.0, (c) Canonical Ltd),\n'
                  'Hack Regular (MIT + Bitstream Vera Licence), Noto Emoji (SIL OFL 1.1), emoji-icon-font (MIT, (c) John Slegers).\n\n')
        for i, (text, crates) in enumerate(blocks.values(), 1):
            out.write('=' * 78 + f'\n[{i}] ' + '\n    '.join(crates) + '\n' + '=' * 78 + '\n' + text + '\n\n')
    print(f'{len(pk)} linked crates, {len(blocks)} distinct licence blocks, {len(missing)} without licence files (verify): {", ".join(missing)}')


if __name__ == '__main__':
    main(*sys.argv[1:4])
