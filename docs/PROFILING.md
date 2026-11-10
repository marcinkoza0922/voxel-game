# Profiling

Profiling tools, measurement methodology and historical results for the voxel game.
Runs describe local observations under the stated conditions, not performance guarantees.
Implementation and cache/streaming decisions remain in [DESIGN.md](DESIGN.md).
Historical entries describe the code and instrumentation available at the time.

## Frame profiling and scripted tours

Optional `--profile PATH` diagnostics live in `tools/performance.rs`, included as
a Rust module. A buffered CSV records unclamped start-to-start frame intervals,
simulation/autosave, streaming plus GPU uploads, draw/presentation wall times,
and resident/queued/active chunk counts, plus focus/menu flags. The first frame
has no interval. Memory use stays bounded; explicit flush errors propagate on
exit. Draw times include surface waits and final capture readback, not GPU
execution timestamps. A slow phase in row N typically affects interval N+1.
`tools/analyze-profile.py` reports nearest-rank median/p95/max and chunk peaks;
`--skip` and `--limit` select a frame range. Profiling is opt-in.

`--tour` performs deterministic camera relocations every 180 frames, including
adjacent chunk crossings and distant terrain, repeating after 1,800 frames.
It requires a frame bound and an explicit unused save path. This is a streaming
stress route, not a walked gameplay expedition.

## CPU stress and sampling tools

`cargo run --release -- --stress --frames 12 --profile target/cpu.csv`
runs deterministic CPU workloads without creating a window, reading settings or
accessing saves. `--seed` selects the terrain seed; `--frames` is the positive
repetition count in this mode (default 12). One complete warmup is excluded.
Samples rotate through four fixed surface locations, so distributions include
terrain variation. CSV rows contain workload, repetition index, wall milliseconds
and output units (halo cells, vertices, final fluid edits, or peak resident chunks).
The console prints median/p95/max; timings are observations, not CI speed gates.

`tools/stress.rs` measures halo generation, surface meshing, surface sky access,
a synthetic checkerboard mesh, cave sky access, twelve fluid pulses from 512
sources, and settled radius-one streaming through adjacent chunks and a distant
teleport and return. Setup is outside each timer. Streaming includes planning,
worker generation/lighting, polling and one-millisecond sleeps; it has a
60-second deadline per repetition. Cave fixtures can be empty for some seeds;
consult vertex units before comparing lighting timings. This is CPU coverage;
it does not measure GPU execution, uploads or full render-radius throughput.

`tools/profile-cpu.sh [output-directory] [repetitions]` builds release, records
compiler/OS/git metadata, saves CSV and summaries, then attempts a separate
`perf record --call-graph dwarf` run and writes a flat self-time report. Release
builds retain line tables for symbolized stacks. Perf permission failures are
reported in `perf-error.txt` while baseline timings remain available. For flame
graphs, use `cargo flamegraph --release -- --stress --frames 12` where sampling
permissions allow it. Sampling overhead is excluded from the baseline CSV.

For end-to-end render profiling, existing `--headless --tour --frames 1800
--save /tmp/unique-world.json --profile target/render.csv --settings
/tmp/isolated-settings.json` exercises the full scripted route with isolated
files (the save must not exist). `tools/analyze-profile.py target/render.csv`
summarizes it. Headless tour relocation frames deliberately wait for terrain
settlement; analyze these separately from steady frames. Draw timings measure
CPU submission/presentation waits, not GPU timestamp durations.

Validation (2026-11-09): formatting, short Clippy and all 110 quiet nextest
tests pass. Release stress completed 24 measured repetitions after warmup.
Final baseline median/p95 milliseconds: halo 0.636/0.828, surface mesh
0.326/0.498, surface sky 5.287/19.083, checkerboard 1.437/1.878,
cave sky 5.388/37.843, twelve fluid pulses 149.931/151.824, and four
settled streaming relocations 164.044/168.869. These are local wall-time
observations; earlier runs varied substantially with machine load. Output is
in `target/performance/{cpu.csv,summary.txt,environment.txt}`. Perf recording
was attempted with user-space cycles both sandboxed and unrestricted, but
host performance-counter access was denied; no sampled call graph was obtained.

The offscreen 1,800-frame tour completed outside the sandbox (the sandboxed
attempt stalled and was interrupted). `target/performance-render.csv` contains
all frames: CPU draw/submission median 0.363 ms, p95 1.326 ms; peak residency
1,284 chunks; longest stream settlement 2,215.413 ms. This run overlapped an
initial CPU stress pass, so treat it as stress coverage rather than an isolated
render benchmark. Interval timings describe the preceding frame, so a settlement
at frame 1620 appears in interval 1621. GPU timestamps were not collected.

## Skylight optimization measurements

A fresh release comparison with 24 measured repetitions each (seed 314159) is
in `target/performance/lighting-{before,after}.{csv,txt}`. Median/p95 ms changed:
surface lighting 5.654/21.000 -> 1.214/8.991; cave lighting 6.288/39.734 ->
2.368/15.680; settled streaming 170.369/175.045 -> 113.970/117.777. This is
roughly 79% less median surface-lighting time, 62% less cave-lighting time and
33% less streaming time on this machine. Unchanged workloads show some load
variation, so these are local observations, not fixed performance guarantees.
No new GPU measurement was needed: vertex exposure is checked for exact equality.

Further candidates, not implemented: share procedural column metadata across
occupancy ray queries, precompute ray offsets, and use a bounded dense occupancy
cache in chunk workers. Persistent cross-mesh lighting caches would also need
careful edit invalidation; current per-operation caches avoid that complexity.

## Shared occupancy measurements

The fresh 24-repetition release comparison is recorded in
`target/performance/occupancy-{before,after}.{csv,txt}`. Cave-lighting median/p95
fell from 1.669/9.161 to 0.667/4.745 ms (60%/48% reduction). Surface lighting
changed from 0.889/5.269 to 0.916/6.025 ms, with no demonstrated improvement;
settled streaming changed from 94.985/101.296 to 102.851/118.247 ms. Other
unchanged workloads also varied substantially between runs, so this result
supports faster cave occupancy sampling, not an overall streaming speedup.
Column metadata sharing is now implemented; precomputed ray offsets and bounded
dense occupancy storage remain possible future work.
A 48-repetition confirmation (`occupancy-after-confirm.{csv,txt}`) measured
cave-lighting median 0.669 ms and p95 5.918 ms, surface median 0.853 ms,
and streaming median 98.972 ms, reinforcing the cave improvement while
showing continuing tail and machine-load variability.

## Profiling rerun (2026-11-09)

`target/performance-rerun/` records a fresh release CPU run (24 repetitions,
seed 314159) and a subsequent isolated 1,800-frame offscreen tour. CPU workload
median/p95 ms: halo 0.430/0.823, surface mesh 0.236/0.452, surface sky
0.954/6.265, checkerboard 0.859/2.254, cave sky 0.854/10.463, twelve fluid
pulses 107.043/169.862, four settled relocations 98.194/124.865.
The stress reporter uses the upper middle sample for even-sized medians.

Tour steady frames (excluding relocation/final settlement) measured draw CPU
wall time median 0.510 ms, p95 1.611 ms and max 83.974 ms. Simulation max
was 0.917 ms. Initial settlement took 1,434.973 ms; distant relocations/return
1,056.949–1,329.602 ms; adjacent moves 58.418–91.903 ms. Peak residency
was 1,284 chunks. Settlement is deliberately blocking in the headless runner;
these durations are terrain readiness latency, not native-window frame stalls.
GPU execution timestamps remain unmeasured.

Hardware sampling succeeded outside the sandbox with `perf record -e cycles:u
--user-callchains --call-graph dwarf`. The explicit user-callchain option is
needed with this host's `perf_event_paranoid=2`; user-only counting worked
without it, but recording failed. The helper now includes this flag. The sampled
run is separate from baseline timing. Its core-PMU flat self-time report shows
main-thread hypot 16.75%, height 16.26%, biome 9.61%, floorf 4.50%, tree
3.16%; worker samples also highlight meshing, height/biome/waterway generation
and skylight. These percentages describe this mixed synthetic workload and
include fixture setup, not the game's frame budget. Repeated procedural terrain
queries are the strongest CPU optimization candidate; fluid pulses and terrain
readiness are the largest measured workload costs. Raw samples and report are
`target/performance-rerun/perf.{data}` and `perf-report.txt`.

## Fluid query cache measurements

Fresh release measurements (48 repetitions, seed 314159, one excluded warmup)
are in `target/performance-rerun/fluid-{before,after}.{csv,txt}`. Twelve-pulse
median/p95 fell from 91.712/126.552 to 23.315/24.722 ms (75%/80% reduction).
Every workload's output units match between runs. Other workloads were slower
in the after run, consistent with machine-load variation; settled streaming
changed from 93.534/109.694 to 111.570/114.769 ms. This demonstrates faster
fluid stress updates, not a streaming or general frame-rate improvement.

## Windowed profiling follow-up (2026-11-09)

`--profile` now appends five CPU wall-time columns: `draw_prepare_ms`
(dynamic geometry, lighting and UI), `surface_acquire_ms`, `draw_encode_ms`
(including encoder finish), `queue_submit_ms`, and `present_capture_ms`
(including optional capture readback). Phase clocks run only with profiling
enabled. These remain CPU timings, not GPU execution timestamps. The analyzer
accepts older CSVs, reports intervals exceeding common frame budgets, and
attributes the five worst intervals to the preceding frame's phases. The
remaining interval time includes event-loop/pacing waits and uninstrumented
work, not necessarily a stall. `--tour` additionally reports settlement and
ending queue counts per 180-frame stage; only use it for tour recordings.

Fresh release native Wayland tours (seed 314159, isolated saves/preferences)
and a separate 3,600-frame perf sampling run are in
`target/performance-followup/`. The unsampled original 1,800-frame tour had
interval median/p95/max 6.061/6.902/12.499 ms, with no intervals above
16.667 ms. The instrumented repeat measured 6.081/6.669/20.640 ms,
with three intervals above 16.667 ms. Its three worst intervals followed
distant relocations: synchronous stream/upload consumed 18.619, 16.746 and
15.635 ms. Streaming replans still evaluate all 256 heights and waterway
inputs for every newly visible horizontal chunk column on the main thread,
then evict, rebuild desired sets and sort jobs. Moving that planning work off
the main thread or budgeting it is a strong hitch-reduction candidate.

Detailed median/p95 CPU draw phases (ms): preparation 0.294/0.499,
surface acquire 0.051/0.091, encoding 0.380/0.898, queue submission
0.148/0.274, presentation 0.070/0.133. Surface acquisition max was
0.276 ms. Run-to-run draw costs varied, so these are observations rather
than an instrumentation overhead comparison or GPU throughput conclusion.

A structural terrain-throughput limit is independent of these frame costs:
only two jobs including completed results may be outstanding, and new jobs
are dispatched only when the main thread polls once per redraw. Consequently,
throughput cannot exceed twice the redraw rate: 120 chunks/second at 60 FPS,
or about 330 at 165 FPS, even if workers finish early. The initial desired
set contains 1,171 chunks; the original tour accepted 358 in its first 180
frames and did not settle until frame 625 while crossing adjacent columns.
Distant locations commonly remained incomplete at the next relocation.
A further 1,800-frame run with an isolated 60-FPS setting accepted 354
chunks in the first stage and first settled at frame 631 (10.576 seconds),
versus frame 625 (3.795 seconds) in the original uncapped/VSync run.
The route is frame-based, so relocation spacing in seconds also changes;
the nearly identical settlement frame counts support the scheduling limit.
The 60-FPS run had interval median/p95/max 16.767/16.861/26.799 ms
and peak synchronous stream/upload 22.409 ms.
Decoupling a bounded worker backlog from a separately budgeted per-frame
upload queue is the leading terrain-readiness candidate. Simply adding
workers or accelerating generation cannot remove the frame-dependent limit.

The separate perf recording lost no samples (seven out-of-order events were
reported). Core-PMU self time highlights worker height (7.13% across both
workers), hypot (6.99%), skylight sampling (6.64%), biome (6.04%), halo
meshing (5.71%) and waterway queries (5.32%). These are shares of sampled
CPU cycles across the run, not frame-time percentages. UI rectangle generation
also appears at 2.16%. Procedural/lighting optimization remains relevant,
but should follow scheduling and synchronous planning investigation.

Validation: release build, short Clippy, formatting, diff checks and all 114
quiet nextest tests pass. Analyzer exercised against old and extended CSVs,
including range selection. This change adds diagnostics; streaming behavior
is unchanged. Native-upscaler builds and GPU execution timing were not tested.

## Buffered streaming measurements

Fresh sequential release tour recordings are in `target/performance-fix/`;
comparisons use `target/performance-followup/` from the preceding investigation.
Each tour uses seed 314159, 1,800 frames, and isolated saves/preferences:

| Measurement | Before | After |
| --- | ---: | ---: |
| 60-FPS tour first settlement | 10.576 s (frame 631) | 0.752 s (frame 45) |
| ~165-Hz tour first settlement | 4.002 s (frame 660) | 0.922 s (frame 147) |
| 60-FPS peak stream/upload CPU | 22.409 ms | 3.982 ms |
| ~165-Hz peak stream/upload CPU | 18.619 ms | 1.986 ms |
| ~165-Hz interval p95 / max | 6.669 / 20.640 ms | 6.465 / 12.480 ms |

All ten stages settled before the next relocation in both after runs, reaching
1,284 resident chunks. Earlier tours commonly left distant stages incomplete
and peaked at 1,200 resident chunks. Tours relocate every 180 frames; first
settlement thus compares route readiness, not an identical stationary scene.
Run-to-run CPU/driver load varies. The 60-FPS interval p95 changed from 16.861
to 17.144 ms despite lower peak stream work; remaining pacing/scheduling tails
are visible, and GPU execution time is still unmeasured. These results establish
faster readiness and lower main-thread streaming peaks, not universal FPS gains.

The release CPU stress completed 24 measured repetitions after warmup:
settled radius-one streaming median/p95 73.787/77.045 ms, twelve fluid pulses
26.468/28.305 ms. Its relocation timing includes worker polling and sleeps;
it is separate from the native frame measurements.

## Buffered streaming offscreen validation

A separate 1,800-frame offscreen tour also completed successfully, including
startup, relocation and final blocking settlement, with no queued/active work
or pending uploads at exit (`target/performance-fix/headless.csv`).

## Underground frontier diagnostic samples

Offscreen Vulkan validation of the latest save contained 1,446 resident chunks.
A three-frame profile measured settled streaming/upload calls
at 0.17 and 0.39 ms; initial settling took 1.38 s. These are diagnostic samples,
not a comparative performance benchmark.

## Mining frontier measurements

The original frontier implementation cleared all cached caps on every world
revision. Offscreen mining updates against `saves/world-36.json` measured a
351.98 ms median and 360.52 ms maximum across twelve edits. Limiting rebuilds
to nearby dirty chunk columns reduced this to 20.20/20.88 ms, but still repeated
unchanged roof queries. Shared roof caching plus exact changed-column tracking
reduced the same workload to 0.555 ms median and 0.658 ms maximum. These debug
build measurements cover the immediate renderer streaming/upload update after
mining, excluding subsequent asynchronous remesh settling, drawing and simulation;
they are not full-frame FPS measurements.

## Mining frontier profiling tool

`tools/frontier_review.rs`, invoked with `cargo run -- --frontier-review --save
PATH [--profile CSV]`, loads an existing save without writing it, settles its
view offscreen, and measures twelve stone-removal updates. Placement and remesh
settling occur between samples. `--profile` writes each measured update in
milliseconds. The tool bypasses window/audio initialization and does not advance
the game simulation. Regression tests verify unchanged-roof reuse, distant cache
retention, negative-coordinate dependencies, vertical invalidation, and exact
cached/fresh cap geometry after roof placement and removal.

## CPU, GPU and memory profiling (2026-11-10)

`--profile PATH.csv` now also writes `PATH.gpu.csv`. GPU timestamps measure
shadow, world (sky/opaque/frontiers/fluids), temporal reconstruction and output,
held item and UI stages, plus the span from the first stage to the last. These
are GPU execution timeline measurements; they exclude CPU preparation, queue
waiting before the first timestamp, presentation, uploads before the frame
encoder and screenshot readback. Temporal timing includes native upscaling when
that build/settings path is active. Stage boundaries use encoder timestamps;
inter-stage GPU scheduling/barriers are included in the enclosing spans.

`tools/gpu_profile.rs` requests optional timestamp capabilities exposed by the
adapter and allocates queries only when a render profile is enabled. Four slots
hold query/resolve/map buffers. Each draw polls callbacks without waiting;
completed maps emit frame-ID-correlated samples to the CSV writer. A saturated
ring emits an empty row instead of blocking. Unsupported timestamp capabilities
produce a diagnostic and an empty GPU CSV. Map failures also emit empty rows.
The analyzer automatically reads the sibling GPU CSV, reports missing samples
and applies the same frame selection. Native shutdown drains the device before
flushing the profile; headless profiling explicitly drains before its flush.
`tools/analyze-profile.py PATH.csv --tour` summarizes CPU and GPU measurements.

`python3 tools/profile-resources.py OUTPUT.csv COMMAND [ARG ...]` samples the
child's aggregate thread CPU use and Linux RSS/high-water RSS, plus NVIDIA GPU
load, memory-controller load, device VRAM, device power and child VRAM at 0.5 s.
CPU 100% represents one logical core. NVIDIA load/power/device memory include
other desktop applications; process VRAM is PID-specific. Peaks are sampled,
including the most recently observed RSS high-water value; short-lived/final
peaks may be missed. This measures resident memory, not allocator attribution
or proof of a leak. It requires Linux `/proc` and `nvidia-smi`; unavailable GPU
fields remain empty. Child output remains on stdout/stderr for caller logging.

Release artifacts are in `target/performance-2026-11-10/`, at source base
`6beed0856f3a56f3481bde784e15998c64bbbccc` plus the profiling instrumentation.
Runs were sequential: 24-repetition deterministic CPU suite (seed 314159),
1,800-frame native baseline tour, separate sampled CPU suite, instrumented
1,800-frame native tour, and a 180-frame offscreen validation. Tour saves and
settings are isolated in that directory. Native tours use 1280x800, full render
scale, TAA, VSync on, no explicit cap and no native upscalers. Hardware is an
RTX 3060 Ti (8 GiB), NVIDIA driver 595.112.07. No capture readback was requested.

CPU stress median/p95 milliseconds: halo 5.551/8.615, surface mesh
0.184/0.246, surface sky 1.008/1.545, checkerboard mesh 0.656/0.742,
cave sky 0.000/1.366, twelve fluid pulses 16.415/17.181 and streaming
relocations 108.177/115.132. Cave fixtures can be empty; consult output units.
CPU stress sampled peak RSS was 18.87 MiB. Separate perf sampling succeeded,
with zero lost samples. On the core PMU, waterway planning accounted for about
23% self time across the two workers and contended mutex locking about 10%.
The atom PMU is reported separately; percentages must not be added across PMUs.
These are investigation candidates, not proof that any particular lock or
waterway operation should be changed. World generation has changed since the
prior profiling baseline, so terrain timings are not like-for-like regressions.

Instrumented native tour GPU timings (milliseconds):

| GPU stage | Median | p95 | Maximum |
| --- | ---: | ---: | ---: |
| Shadow | 0.260 | 0.400 | 0.520 |
| World | 0.571 | 1.016 | 1.426 |
| Temporal/output | 0.133 | 0.276 | 0.999 |
| Held item | 0.051 | 0.074 | 0.109 |
| UI | 0.002 | 0.004 | 0.219 |
| Total span | 1.023 | 1.660 | 2.357 |

All 1,800 GPU rows were measured, including the final frame, with no dropped
samples. Frame interval median/p95/max was 6.072/10.975/16.109 ms; none
exceeded 16.667 ms. Streaming/upload CPU peaked at 10.323 ms; peak resident
chunks were 1,475. Some tour stages did not settle within their 180-frame
window, so the GPU distribution includes partially populated scenes. Sampled
process CPU median/p95 was 66%/238%, RSS peaked at 434 MiB and process VRAM
at 483 MiB. Device GPU utilization median/p95 was 28%/41%. This short paced
route does not establish sustained GPU throughput or long-term memory growth.
The world pass is the largest measured GPU stage; terrain planning/streaming
and pacing are more immediate CPU/frame-latency investigation targets here.

Validation: formatting, clean short-format Clippy, all 151 quiet nextest tests,
a complete native Vulkan tour and an offscreen timestamp/readback check.
The uncapped 180-frame offscreen check produced eight measured GPU samples
and 172 explicit unavailable rows as the CPU outran the four-slot ring. All
frame IDs were retained and measured stage sums matched their total spans;
this validates saturation/drop reporting without adding synchronization.
Native-upscaler builds were not exercised in this pass.

## Waterway lookup and lock measurements (2026-11-10)

Artifacts are in `target/waterway-investigation/`. Preserved `voxel-before`
and `voxel-after` release binaries permit independent reproduction without
symbol changes when rebuilding. Sequential unsampled runs use seed 314159,
48 measured repetitions and one excluded warmup, with identical workload output
counts across all 336 rows. First comparison median/p95 milliseconds:

| Workload | Before | After |
| --- | ---: | ---: |
| Four settled streaming relocations | 108.014 / 112.890 | 72.946 / 79.463 |
| Cold terrain halo | 5.515 / 6.976 | 5.376 / 6.913 |
| Surface sky | 1.010 / 1.559 | 0.928 / 1.439 |
| Twelve fluid pulses | 16.382 / 16.816 | 16.638 / 17.185 |

A second sequential 48-repetition comparison reproduced the result: streaming
median/p95 was 108.879/113.428 ms before and 71.990/75.492 ms after, with
all workload output counts again matching.

The streaming median improved about 32–34%; cold halo construction barely changed
because new fixture worlds still pay cold plan-building and terrain-sampling
costs. Fluid timing showed no improvement in this comparison. These are local
CPU workloads, not an FPS claim.

Separate hardware sampling on the core PMU measured aggregate waterway-plan
self time falling from 23.47% to 1.24%, and contended mutex self time from
10.24% to 0.05%. These percentages are sampled CPU-cycle shares, not wall time
blocked on locks, and include the relevant process threads. The baseline
sampling run lost 39 samples; the after run lowered sampling frequency to
999 Hz and lost none. Treat exact percentages as approximate. Along with the
code path and unsampled timings, this supports shared per-voxel lookup traffic
as the dominant contention source. Remaining worker costs include height,
relief and climate sampling and distance calculations; cold waterway plan
construction remains a separate optimization candidate.

## CPU profiling follow-up (2026-11-10, pass 2)

Source `61f70c7b618ed07349999fcc4ac38acf8c712016` was profiled without
code changes. Artifacts are in `target/performance-cpu-2026-11-10-pass2/`:
24-repetition unsampled CPU stress CSV/summary, a separate sampled stress run,
a fresh 1,800-frame native tour with phase timings, and a separate 3,600-frame
native sampling tour. Runs were sequential, seed 314159, with isolated tour
saves/settings copied from the previous profiling pass (1280x800, TAA, VSync,
no explicit cap, no native upscaler). Both perf recordings lost zero samples;
the native recording used 999 Hz. Core and atom PMUs are separate populations;
percentages below refer only to core-PMU self time across process threads.

Stress median/p95 milliseconds, using the stress tool's upper-middle median:

| Workload | Median | p95 |
| --- | ---: | ---: |
| Cold terrain halo | 5.595 | 7.215 |
| Surface mesh | 0.194 | 0.271 |
| Surface sky | 0.952 | 1.487 |
| Checkerboard mesh | 0.689 | 0.871 |
| Cave sky | 0.000 | 1.248 |
| Twelve fluid pulses | 17.026 | 17.754 |
| Four settled streaming relocations | 74.350 | 82.506 |

All 168 workload output counts match the earlier 24-repetition baseline.
Streaming remains about 31% faster than its pre-fix 108.177 ms median, and
is about 3% slower than the prior post-fix repeat's 71.990 ms median. This is
one local repeat, not evidence of a code regression. Cave fixtures can be empty.
`comparison.txt` uses the conventional midpoint median for even sample counts;
`summary.txt` and the table above use the stress executable's existing convention.

The stress core samples are now dominated by terrain workers: height 11.89%,
relief 10.16%, climate 8.80%, double hypot 13.06% and float hypot 6.77%.
These are separate self-time categories, not inclusive call costs. Worker
waterway-plan lookup totals only 1.30% above the report's 0.5% row threshold;
shared-cache contention is no longer a leading visible hotspot. Cold waterway
construction still contributes 4.06% across main/planning threads above that
threshold. Native-tour samples independently show worker height 13.86%,
climate 8.85%, relief 7.97%, double hypot 9.88% and float hypot 7.02%.
Main-thread renderer world updates contribute 2.44%, height 2.02%, and chunk
visibility 0.70%. These cycle shares describe sampled CPU activity, not frame
latency or time asleep waiting for pacing.

Native frame interval median/p95/max was 6.086/9.895/17.835 ms; two of
1,799 intervals exceeded 16.667 ms and none exceeded 33.333 ms. CPU phase
median/p95: stream/upload 0.843/3.215 ms, draw preparation 0.422/0.599 ms,
encoding 0.897/1.379 ms, queue submission 0.146/0.235 ms. Stream/upload
peaked at 7.604 ms; simulation peaked at 6.752 ms. The worst interval followed
6.630 ms simulation work plus 7.893 ms outside the recorded phases, so it
cannot be attributed solely to terrain uploads. All ten tour stages settled
before their next relocation, with initial settlement at frame 134 and peak
residency 1,478 chunks. This compares with some unsettled stages in the earlier
pre-fix tour, but frame-paced local runs are not a controlled throughput test.

The next investigation candidate is repeated terrain-field evaluation across
height, biome, vegetation and lighting queries, including repeated distance
calculations. Reusing exact column results should be evaluated before changing
floating-point distance formulas, which could alter generated terrain. Renderer
world-update traversal/uploads remain a secondary native CPU candidate. This
pass collected evidence only; no optimization or gameplay changes were made.
Validation: release build, complete stress/tour runs, zero-loss hardware samples,
1,800 complete CPU/GPU frame rows, and documentation whitespace checks. Tests
and Clippy were not rerun because only profiling artifacts and documentation changed.

## Terrain-column field reuse (2026-11-10)

Artifacts are in `target/terrain-column-investigation/`. Preserved `voxel-before`
and `voxel-after` release executables compare source `61f70c7` against column
biome reuse, shared climate/relief inputs for height and biome, and early tree
canopy rejection. The implementation is described in [DESIGN.md](DESIGN.md).
No terrain arithmetic, noise, distance formulas or persistent caches changed.

Two sequential before/after pairs used seed 314159, 48 measured repetitions
and one excluded warmup per run. All 336 workload/sample/output-unit tuples
matched in each pair. First comparison median/p95 milliseconds (upper-middle
median, matching the stress executable):

| Workload | Before | After |
| --- | ---: | ---: |
| Cold terrain halo | 5.483 / 7.089 | 5.044 / 6.884 |
| Surface mesh | 0.183 / 0.269 | 0.189 / 0.272 |
| Surface sky | 0.951 / 1.499 | 0.615 / 1.173 |
| Checkerboard mesh | 0.657 / 0.992 | 0.658 / 0.754 |
| Cave sky | 0.000 / 1.257 | 0.000 / 1.016 |
| Twelve fluid pulses | 16.980 / 18.080 | 14.053 / 14.513 |
| Four settled streaming relocations | 74.634 / 79.586 | 50.062 / 54.508 |

The second pair reproduced streaming 72.931/77.661 -> 49.977/56.180 ms,
surface sky 0.927/1.435 -> 0.614/1.306 ms, and fluid pulses
16.565/17.822 -> 14.049/15.427 ms. Median streaming improved 31–33%,
surface lighting 34–35%, and fluid pulses 15–17%. Cold halo medians improved
7–8%; cold hydrology work remains. Cave fixtures are sometimes empty, so their
zero median does not describe occupied-cave lighting. Unchanged mesh workloads
show run-to-run variation; these are local CPU results, not an FPS claim.
The combined change does not isolate the contributions of biome reuse and tree
candidate pruning. No new native frame-timing comparison was collected.

A separate 48-repetition after run sampled user CPU cycles at 999 Hz with DWARF
call stacks, losing zero samples. Core-PMU worker self time now includes
`height_from_fields` 18.17%, climate 2.74%, relief 1.65%, double hypot 4.00%
and float hypot 5.39%. The main/planning threads' waterway construction accounts
for 6.05% above the report's 0.5% row threshold. These are shares of a smaller
remaining CPU workload, not direct speedup percentages or inclusive call costs;
atom-PMU samples remain separate. The leading remaining work still includes
height evaluation and cold hydrology construction. Short Clippy validation
briefly overlapped this sampled run; unsampled timing pairs ran independently.

Validation: all 153 quiet nextest tests pass, including golden halo fingerprints
captured from the original implementation across three seeds and seven horizontal
locations at surface and subterranean heights (42 halos, 244,944 block values).
Existing edit/resident precedence, cached/direct queries, skylight, fluid,
hydrology and tree tests also pass. Release build, standard short-format Clippy,
formatting and whitespace checks pass. Golden fingerprints serialize blocks in
stable halo order and use FNV-1a, independent of randomized hash-map iteration.
