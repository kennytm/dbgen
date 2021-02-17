#!/usr/bin/env python

# release/release.py builds the dbgen binaries for release.

import os.path, subprocess, sys

if sys.platform.startswith('linux'):
    subprocess.check_call(['rustc', '-vV'])
    subprocess.check_call(['cargo', 'build', '--release', '--target', 'x86_64-unknown-linux-gnu'])
    subprocess.check_call(['strip', '-s', 'target/x86_64-unknown-linux-gnu/release/dbgen'])
    subprocess.check_call(['strip', '-s', 'target/x86_64-unknown-linux-gnu/release/dbschemagen'])
else:
    p = os.path.dirname(os.path.realpath(os.path.dirname(__file__)))
    home = os.path.expanduser('~')
    subprocess.check_call([
        'docker', 'run',
        '--volume', p + ':/dbgen',
        '--volume', os.path.join(home, '.cargo', 'git') + ':/root/.cargo/git:ro',
        '--volume', os.path.join(home, '.cargo', 'registry') + ':/root/.cargo/registry:ro',
        '--workdir', '/dbgen',
        '--rm',
        '--network=host',
        'kennytm/dbgen-build-env',
        '/dbgen/release/release.py',
    ])
