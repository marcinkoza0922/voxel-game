# Playtest and performance record — 2026-11-09

## Scope and outcome

The repeatable gameplay expedition passes through copper gear, and native Vulkan
world/pack captures render correctly. A measured chunk-crossing hitch was reduced
by caching procedural surface bounds and avoiding repeated queue searches/sorts.
This session used automated game actions and scripted camera relocations. It did
not include a human completing an unaided expedition in a naturally generated
world; resource discoverability, navigation, combat feel and long-term balance
remain manual playtest work.

## Checks performed

- `./tools/verify.sh`: formatting, Clippy with warnings denied, 31 quiet nextest
  tests, original smoke, and expanded expedition passed.
- `cargo run -- --expedition`: fresh starting inventory; controlled arena with
  reach-based mining of 12 logs, 14 stone, 8 copper and 10 coal; crafting and
  actual placement of table/furnace/bed; stone and copper axe/pick, copper sword;
  hostile combat and meat loot; cooking/eating; enemy-blocked sleep and successful
  sleep after victory; torch placement; natural forest discovery and cave
  collision after diagnostic relocation; exact saved player, edits, torch timers
  and clock match after reload. Resource search and walked travel are bypassed.
- Native release Vulkan baseline: 900 frames, world capture inspected, clean exit.
- Native release streaming comparison: 1,800 baseline frames versus the first
  1,800 optimized frames, same seed `314159`, fresh saves. The route rotates the
  camera, crosses successive x chunk boundaries, relocates to `(240,240)` and
  `(1024,1024)`, crosses another boundary and returns to spawn. Relocation every
  180 frames deliberately exercises replanning before every view finishes loading.
- Optimized soak: 10,800 frames, six route repetitions, about 66 seconds, saved
  and exited normally. At most two worker jobs remained outstanding. This is
  a short soak, not a long-duration stability or memory-leak test.
- Native pack: 240 frames, capture inspected; inventory, all recipes and stat
  controls fit and are readable. World capture shows terrain, HUD and shadows.
  Cave lighting was not visually evaluated.

## Measurements

Wall times in milliseconds, Linux Vulkan desktop with FIFO VSync, release build.
Single before/after run, no statistical claim across machines or repeated trials.
First interval excluded in both comparisons. GPU timestamp queries are not used;
draw/present includes waiting and capture readback. Start-to-start intervals
include compositor pacing; a slow phase in frame N affects interval N+1.

| Metric | Before, 1,799 intervals | After, 1,799 intervals |
| --- | ---: | ---: |
| Frame interval median | 6.059 | 6.059 |
| Frame interval p95 | 6.423 | 6.666 |
| Worst frame interval | 22.991 | 12.102 |
| Streaming/uploads median | 0.030 | 0.014 |
| Streaming/uploads p95 | 0.174 | 0.033 |
| Streaming/uploads worst | 19.709 | 5.733 |

Median stayed refresh-bound; interval p95 was slightly higher, so this does not
show a general FPS increase. It does show a reduction in the measured chunk-crossing
hitch. Adjacent x crossings at frames 180/360/540/720/900 took
19.709/16.691/15.819/4.249/4.384 ms before and
0.467/0.418/0.390/0.375/0.373 ms after. Distant relocations still require full
height sampling, so they retain synchronous planning cost.

The complete optimized soak recorded median/p95/worst intervals of
6.059/6.923/13.254 ms, streaming/uploads p95/worst of 0.035/10.996 ms,
and peak resident/queued/active chunks of 1,200/1,159/2. The final framebuffer
capture is included in the draw/present measurements. Queued chunks are work
waiting to be dispatched; active includes worker results not yet collected.

## Reproduce

Use an unused save filename for each tour:

```sh
./tools/verify.sh
cargo run --release -- --tour --frames 1800 --profile /tmp/tour.csv --save /tmp/fresh-tour.json
python3 tools/analyze-profile.py /tmp/tour.csv --skip 1 --limit 1799
```

Increase to `--frames 10800` for six route repetitions. Add `--capture` to inspect
the final frame; capture readback affects that frame's draw time. A live CSV may
have an incomplete trailing row, which the analyzer ignores. Profiling records
focus and menu status so manual sessions can be interpreted correctly. Avoid
running other GPU workloads during comparison.

## Next manual session

Start a fresh world with profiling and play without fixture resources. Record
time to first logs, stone tools, coal/copper, copper gear and a safe camp. Include
forest travel, cave entry, combat, sleep, quit and reload. Note places where the
next action is unclear or a resource is hard to find. The automated expedition
checks the mechanics; this session will establish whether finding and using them
feels good. Further rendering work should follow a repeatable measured hitch.

## Iron/tundra expansion validation — 2026-11-09

- Formatting, short Clippy with warnings denied, 65 quiet nextest tests, gameplay
  smoke and expanded expedition passed.
- Expedition now mines iron with copper tools, builds a crude smelter, processes
  iron and crafts all three iron tools/weapons. It relocates to a naturally
  generated tundra ruin, defeats the guardian through normal combat, mines and
  collects the six-ingot cache, then verifies player, edits and all station
  states after reload. Seed 314159's tested ruin is at (-1951, 174, -1902).
- A native release Vulkan run rendered 900 frames from a temporary tundra ruin
  save and exited cleanly. The inspected capture shows ruin walls/pillars,
  the blue-gray guardian, held iron sword, iron tool/ore/ingot hotbar icons,
  tundra HUD and combat damage feedback. It also shows the guardian pursuing
  the stationary player to melee range. Sound was muted for this capture.
- After excluding 180 startup frames, this static ruin view recorded frame
  interval median/p95/worst of 6.059/6.332/9.483 ms, streaming/upload p95/worst
  of 0.167/1.604 ms, and peak resident/queued/active chunks of 1295/933/2.
  This is one native check, not a comparison with the earlier streaming tour.
- Natural ruin search, full travel, smelter mouse ergonomics, combat balance and
  extended play remain human evaluation work. Tundra temperature/clothing are
  outside this content pass.

## Automated functional suite

Run `cargo nextest run` for the mechanics course and progression expedition along
with the unit regressions. Run `cargo run -- --playtest` for both scenarios with
success reports, or `./tools/verify.sh` for formatting, lint, tests and CLI checks.
No display, audio service or existing save is required. Failures name the failed
mechanics checkpoint or expedition action and return a failing test/exit status.

The mechanics course covers movement speeds, sprint hunger gating, jumping,
landing, solid walls, falls, hunger/rest drain, starvation, death/respawn inventory
retention and timed torch expiry. The expedition covers resource-to-iron
progression, placed processing stations, combat/loot, food/sleep, exploration
rewards, the tundra guardian/cache and persistence. Both use controlled fixtures;
see DESIGN.md for their setup and limits. They establish functional correctness,
not fun, natural resource discoverability or UI/audio/visual quality.
