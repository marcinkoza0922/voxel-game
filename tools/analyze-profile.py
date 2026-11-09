#!/usr/bin/env python3
"""Summarize all recorded intervals; optionally exclude startup frames."""
import argparse
import csv
import math

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('csv')
parser.add_argument('--skip', type=int, default=0)
parser.add_argument('--limit', type=int, help='maximum frames after skipping')
args = parser.parse_args()
with open(args.csv, newline='') as source:
    # A live buffered writer may leave a partial trailing record.
    rows = [row for row in csv.DictReader(source) if all(value is not None for value in row.values())][args.skip:]
    if args.limit is not None:
        rows = rows[:args.limit]
if not rows:
    parser.error('no frames remain')
print(f'{len(rows)} frames (skipped {args.skip}); wall times include presentation waits')
for field in ('interval_ms', 'simulation_ms', 'stream_upload_ms', 'draw_present_ms'):
    ordered = sorted(float(row[field]) for row in rows if row[field])
    if not ordered:
        continue
    quantile = lambda fraction: ordered[max(0, math.ceil(len(ordered) * fraction) - 1)]
    print(f'{field}: median {quantile(.5):.3f}, p95 {quantile(.95):.3f}, worst {ordered[-1]:.3f} ms')
print('Peak chunks: ' + ', '.join(f'{key}={max(int(row[key]) for row in rows)}' for key in ('resident', 'queued', 'active')))
print('Slowest frame:', max(rows, key=lambda row: float(row['interval_ms'] or 0)))
