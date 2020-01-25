#!/usr/env/env python3

from argparse import ArgumentParser
from collections import namedtuple
from pathlib import Path
from random import randrange
import os
import subprocess

Params = namedtuple('Params', 'template,files,inserts,rows,last_inserts,last_rows,initialize', defaults=[None, None, None])

prog_folder = Path(__file__).parent.resolve()
default_exe_path = prog_folder.parent.parent / 'target' / 'release' / 'dbgen'
if os.name == 'nt':
    default_exe_path = default_exe_path.with_suffix('.exe')

parser = ArgumentParser(description = 'Generate TPC-C-compatible *.sql dump for MySQL and PostgreSQL.')
parser.add_argument('-o', '--output', metavar='DIR', type=Path, required=True, help='output directory')
parser.add_argument('-w', '--warehouses', metavar='W', type=int, required=True, help='number of warehouses')
parser.add_argument('--nurand-c', metavar='C', type=int, default=randrange(256), help='constant C used in NUrand() function for C_LAST column')
parser.add_argument('--exe', metavar='PATH', type=Path, default=default_exe_path, help='path of dbgen executable')
parser.add_argument('--templates', metavar='DIR', type=Path, default=prog_folder, help='path of tpcc folder containing all *.sql templates')
parser.add_argument('--schema-name', metavar='QNAME', type=str, default='tpcc', help='schema name')
parser.add_argument('-j', '--jobs', type=int, help='number of parallel file generation jobs')
args = parser.parse_args()

def split_inserts_evenly(warehouse_per_file, inserts_per_warehouse, rows):
    warehouses = args.warehouses
    files = max(round(warehouses / warehouse_per_file), 1)
    inserts = round(warehouses / files) * inserts_per_warehouse
    remaining_inserts = warehouses * inserts_per_warehouse - inserts * files
    res = {
        'files': files,
        'inserts': inserts,
        'rows': rows,
    }
    if remaining_inserts != 0:
        res['last_inserts'] = inserts + remaining_inserts
    return res


# The total file size needed is roughly (80.3*W + 8.3) MiB
ALL_PARAMS = (
    # fixed 4 rows
    Params(
        template = '0_config',
        initialize = f'@warehouses := {args.warehouses}; @nurand_c := {args.nurand_c}',
        files = 1,
        inserts = 1,
        rows = 4,
    ),
    # fixed 100000 rows, ~8.3 MiB
    Params(
        template = '1_item',
        files = 1,
        inserts = 1000,
        rows = 100,
    ),
    # W rows, 113 B/warehouse
    Params(
        template = '2_warehouse',
        files = 1,
        inserts = 1,
        rows = args.warehouses,
    ),
    # 100000*W rows, 33 MiB/warehouse => 8 warehouses/file
    Params(
        template = '3_stock',
        **split_inserts_evenly(8, 1000, 100)
    ),
    # 10*W rows, 1.2 KiB/warehouse. Not going to split files unless we want W > 20000
    Params(
        template = '4_district',
        files = 1,
        inserts = (args.warehouses + 9) // 10,
        rows = 100,
        last_rows = (args.warehouses % 10) * 10,
    ),
    # 30000*W rows, 18 MiB/warehouse => 14 warehouses/file
    Params(
        template = '5_customer',
        initialize = f'@nurand_c := {args.nurand_c}',
        **split_inserts_evenly(14, 300, 100)
    ),
    # 30000*W rows, 2.4 MiB/warehouse => 100 warehouses/file
    Params(
        template = '6_history',
        **split_inserts_evenly(100, 300, 100)
    ),
    # 30000*W rows (order), 1.8 MiB/warehouse
    # ~300000*W rows (order_line), 25 MiB/warehouse => 10 warehouses/file
    Params(
        template = '7_order',
        **split_inserts_evenly(10, 3000, 10)
    ),
    # 9000*W rows, 127 KiB/warehouse => 2048 warehouses/file
    Params(
        template = '8_new_order',
        **split_inserts_evenly(2048, 90, 100)
    ),
)

args.output.mkdir(exist_ok=True)
for params in ALL_PARAMS:
    proc_args = [
        str(args.exe),
        '-k', str(params.files),
        '-n', str(params.inserts),
        '-r', str(params.rows),
        '-i', str((args.templates / params.template).with_suffix('.sql')),
        '-o', str(args.output / params.template),
        '--schema-name', args.schema_name,
    ]
    if args.jobs:
        proc_args.extend(('-j', str(args.jobs)))
    if params.initialize:
        proc_args.extend(('-D', params.initialize))
    if params.last_inserts:
        proc_args.extend(('--last-file-inserts-count', str(params.last_inserts)))
    if params.last_rows:
        proc_args.extend(('--last-insert-rows-count', str(params.last_rows)))
    print('****** GENERATING', params.template)
    subprocess.run(proc_args, check=True)
(args.output / '0_config' / f'{args.schema_name}-schema-create.sql').write_text(f'CREATE SCHEMA IF NOT EXISTS {args.schema_name};\n')

print('ALL DONE!')
