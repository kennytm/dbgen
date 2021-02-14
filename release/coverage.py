#!/usr/bin/env python3

import itertools, json, os, pathlib, shutil, subprocess

cov_dir = pathlib.Path('target', 'cov')

# Install the LLVM tools if the command produces errors.
#
# $ rustup component add llvm-tools-preview
# $ cargo install cargo-binutils rustfilt
#
# Open target/cov/html/index.html after finished.

shutil.rmtree(cov_dir, ignore_errors=True)
os.makedirs(cov_dir)

cargo_test = subprocess.run(
    ['cargo', '+nightly', 'test', '--tests', '--message-format=json'],
    env=os.environ | {
        'LLVM_PROFILE_FILE': str(cov_dir / 'dbgen-%m.profraw'),
        'RUSTFLAGS': '-Zinstrument-coverage',
    },
    check=True,
    text=True,
    cwd='.',
    stdout=subprocess.PIPE,
    stderr=None,
)

messages = (json.loads(line) for line in cargo_test.stdout.splitlines() if line.startswith('{'))
exes = [obj['executable'] for obj in messages if obj.get('profile', {}).get('test')]

profraws = [path for path in cov_dir.iterdir() if path.suffix == '.profraw']

print('merging profdata')
subprocess.run([
    'cargo', 'profdata', '--', 'merge',
    '--sparse',
    '-o', cov_dir / 'dbgen.profdata',
    *profraws,
])

print('generating report')
subprocess.run([
    'cargo', 'cov', '--', 'show',
    '--format', 'html',
    '--Xdemangler', 'rustfilt',
    '--ignore-filename-regex', r'[\\/]\.(cargo|rustup)[\\/]',
    '--instr-profile', cov_dir / 'dbgen.profdata',
    '--output-dir', cov_dir / 'html',
    *itertools.chain.from_iterable(('--object', exe) for exe in exes),
])
