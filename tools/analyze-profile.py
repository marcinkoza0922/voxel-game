#!/usr/bin/env python3
"""Summarize all recorded intervals; optionally exclude startup frames."""
import argparse
import csv
import math
from pathlib import Path

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('csv')
parser.add_argument('--skip', type=int, default=0)
parser.add_argument('--limit', type=int, help='maximum frames after skipping')
parser.add_argument('--tour', action='store_true', help='report settlement per 180-frame tour stage')
args = parser.parse_args()
with open(args.csv, newline='') as source:
    # A live buffered writer may leave a partial trailing record.
    rows = [row for row in csv.DictReader(source) if all(value is not None for value in row.values())][args.skip:]
    if args.limit is not None:
        rows = rows[:args.limit]
if not rows:
    parser.error('no frames remain')
print(f'{len(rows)} frames (skipped {args.skip}); wall times include presentation waits')
for field in ('interval_ms', 'simulation_ms', 'stream_upload_ms', 'draw_present_ms',
              'draw_prepare_ms', 'surface_acquire_ms', 'draw_encode_ms',
              'queue_submit_ms', 'present_capture_ms'):
    if field not in rows[0]:
        continue
    ordered = sorted(float(row[field]) for row in rows if row[field])
    if not ordered:
        continue
    quantile = lambda fraction: ordered[max(0, math.ceil(len(ordered) * fraction) - 1)]
    print(f'{field}: median {quantile(.5):.3f}, p95 {quantile(.95):.3f}, worst {ordered[-1]:.3f} ms')
print('Peak chunks: ' + ', '.join(f'{key}={max(int(row[key]) for row in rows)}' for key in ('resident', 'queued', 'active')))
intervals = [float(row['interval_ms']) for row in rows if row['interval_ms']]
if intervals:
    for budget in (8.333, 16.667, 33.333):
        count = sum(value > budget for value in intervals)
        print(f'Intervals > {budget:.3f} ms: {count}/{len(intervals)} ({100 * count / len(intervals):.1f}%)')
print('Slowest intervals and preceding work (phase times belong to the previous frame):')
slowest = sorted(range(len(rows)), key=lambda i: float(rows[i]['interval_ms'] or 0), reverse=True)[:5]
for index in slowest:
    row = rows[index]
    print(f"  frame {row['frame']}: {row['interval_ms']} ms; focused={row['focused']}, menu={row['menu']}")
    if index and int(rows[index - 1]['frame']) + 1 == int(row['frame']):
        previous = rows[index - 1]
        work = sum(float(previous[key]) for key in ('simulation_ms', 'stream_upload_ms', 'draw_present_ms'))
        gap = float(row['interval_ms'] or 0) - work
        print(f"    previous: simulation={previous['simulation_ms']}, stream/upload={previous['stream_upload_ms']}, draw/present={previous['draw_present_ms']} ms; outside phases={gap:.3f} ms")
if args.tour:
    print('Tour stages, if --tour was used (180 frames per stage):')
    for stage in sorted({int(row['frame']) // 180 for row in rows}):
        group = [row for row in rows if int(row['frame']) // 180 == stage]
        settled = next((row['frame'] for row in group if int(row['queued']) == 0 and int(row['active']) == 0), 'not settled')
        print(f"  stage {stage}: first settled frame={settled}; ending resident/queued/active="
              + '/'.join(group[-1][key] for key in ('resident', 'queued', 'active')))

# Separate asynchronous GPU rows retain their source frame, not readback frame.
gpu_path = Path(args.csv).with_suffix('.gpu.csv')
if gpu_path.exists():
    with gpu_path.open(newline='') as source:
        gpu_rows = [r for r in csv.DictReader(source)
                    if int(r['frame']) >= args.skip
                    and (args.limit is None or int(r['frame']) < args.skip + args.limit)]
    valid = [r for r in gpu_rows if r.get('gpu_total_ms')]
    print(f'GPU timestamps: {len(valid)} measured, {len(gpu_rows) - len(valid)} unavailable; '
          f'{len(rows) - len(gpu_rows)} CPU frames without GPU rows')
    for field in ('gpu_shadow_ms', 'gpu_world_ms', 'gpu_temporal_ms',
                  'gpu_held_ms', 'gpu_ui_ms', 'gpu_total_ms'):
        values = sorted(float(r[field]) for r in valid)
        if values:
            q = lambda f: values[max(0, math.ceil(len(values) * f) - 1)]
            print(f'{field}: median {q(.5):.3f}, p95 {q(.95):.3f}, worst {values[-1]:.3f} ms')
