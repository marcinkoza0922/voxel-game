# Design

## Agreed direction

A Linux, single-player, first-person voxel adventure in Rust. The initial delivery is a playable vertical slice; PROPOSAL.md remains the long-term content direction. The player approved window/input and GPU libraries and other primitive dependencies provided they do not constrain the design. The ECS, renderer, simulation, GUI layout/widgets, crafting, terrain, and persistence are owned here. No game engine, external ECS, or GUI toolkit.

The player approved deterministic seeded chunk streaming and persistent player state/block edits. They requested substantially more vertical space than the proposal's Minecraft comparison for large mountains. The initial engineering choice is y=-512..1535 (2,048 blocks), using sparse chunk storage rather than a tall dense world allocation. Approved slice: grasslands, forest, caves, wood/stone/copper progression, one hostile creature, hunger/sleep, basic XP/stats, inventory/crafting, sunlight/shadows/AO/torch lighting. Ray tracing is deferred.

## Architecture (before implementation)

- `world`: deterministic integer-hashed multi-octave terrain, broad mountain ridges, grassland/forest climate, caves, ore and trees. 16-cube chunks are streamed near the camera; generated terrain can be queried without resident chunks. Save edits override generation. CPU face meshing culls hidden faces, including across chunk boundaries; vertex corner AO and sunlight shadows supplement directional lighting. Shader fog hides the view boundary. No authored raster, sound, or 3D assets are needed for these code-native cubes.
- `ecs`: bespoke generational entity IDs and component slots, initially used by hostile creatures. Simulation owns entities, fixed-step movement/collision, combat, mining, crafting, progression, hunger and fatigue. Rendering consumes state without owning gameplay rules.
- `game`: 60 Hz simulation with a bounded catch-up accumulator. Walking, gravity, jumps, voxel collision, fall damage, reach-limited DDA selection, resource drops, durable tools, placement, hostile pursuit and melee. Player starts with axe and pickaxe. Death respawns at the original spawn and retains inventory to keep the slice forgiving.
- `items`: typed items, stack inventory, explicit recipes and station requirements. Crafting uses shaped 2x2/3x3 grids with a read-only recipe guide; 6x6 crafting remains future work. Wood/stone/copper tools and furnace processing form the initial loop. First crafts award diminishing XP (quartering each repetition); biome/resource discoveries and level-adjusted kills also award XP. Stats give modest bounded bonuses.
- `render`: winit event loop and wgpu raster pipeline with own camera and geometry buffers. Lit vertex-colored voxel faces, shadow sampling, fog, torch point lights, a day/night cycle and a separate unlit GUI pass. Rebuild only changed or newly resident chunk meshes. GPU resources are isolated from simulation.
- `ui`: owned immediate-mode rectangle/text geometry, tiny embedded bitmap glyphs, panels, click hit testing and keyboard navigation. HUD displays vitals, selected item, target, biome and progression. Inventory contains icon slots, drag-and-drop crafting and stat allocation. Menus release the pointer; gameplay captures it.
- `save`: versioned serde JSON with seed, player, inventory, progression, clock, and sparse edits. Atomic temp-file rename, explicit errors, periodic autosave and save on exit. Generated chunks are reconstructible and not serialized. Default save under `saves/`; CLI supports alternate path/seed and a headless smoke mode.

## Slice boundaries and evolution

Advanced metallurgy, magic/gems, breeding, quests, logistics, electricity, automation, extra biomes and multiplayer are future work. Existing station/recipe/item registries and separate simulation should accommodate them. The slice has finite render distance but not a finite horizontal map. Terrain generation and CPU meshing run on bounded background workers using immutable jobs. Stream planning and GPU uploads remain on the main thread. Save format versions reject unsupported data rather than silently discarding it.

## Validation

Use quiet nextest tests for determinism, negative coordinate chunk mapping, height limits, cross-chunk edits, voxel ray casting/collision, generation-safe entity reuse, recipe transactions, XP decay, save round trips and malformed saves. Run clippy with short diagnostics. A headless deterministic simulation smoke command exercises gameplay and persistence without a display. GPU validation requires a real window or supported offscreen adapter; record actual checks and limitations here. Helpers belong in `tools/`.

## Implemented decisions and limits

Presentation defaults to strict FIFO VSync. Settings can select VSync off (Immediate, then Mailbox if supported; FIFO fallback on surfaces exposing neither). An independent optional frame cap uses winit `WaitUntil` deadlines and skips early redraws; uncapped rendering requests the next frame continuously. `Window::pre_present_notify` supplies Wayland compositor frame callbacks. Simulation remains fixed at 60 Hz. VSync can constrain the frame rate below the requested cap; GPU workload and compositor behavior still determine actual pacing.

The slice uses `winit 0.30`, `wgpu 27`, `glam` for primitive vector/matrix math, `bytemuck` for GPU byte layouts, `pollster` for startup, and `serde`/`serde_json`. Cargo.lock is checked in for reproducibility. GPU initialization explicitly selects Vulkan on Linux. The native graphics check exposed an NVIDIA EGL/Wayland teardown crash when probing all backends; Vulkan selection and releasing renderer/window resources before ending the event loop produce clean exits. GPU work is drained before destruction.

The initial ECS is a small, single-archetype component store: generational entity IDs index reusable slots containing position, health, level and AI cooldown. Creature behavior and physics run in explicit simulation systems. This is deliberately smaller than a general archetype/query framework. More component stores and scheduling can be added when multiple entity classes require them. Creatures are transient and respawn after loading; player state, discoveries, recipe counts, tool wear, torches, time and block edits persist.

Terrain chunks are 16x16x16. Stream radius is ten chunks horizontally (160 blocks along each axis). Each horizontal chunk column samples all 256 terrain heights and streams from one chunk below its lowest surface through its highest surface plus tree canopy, without clamping mountains to player elevation. A five-by-five horizontal area additionally streams two chunks above/below the player for cave exploration. Streaming replans on player chunk or radius changes and evicts chunks outside the desired set. Jobs are ordered by distance with deterministic ties; obsolete mesh jobs are discarded and remaining mesh jobs reprioritized near the player. Two persistent CPU workers generate chunk blocks and meshes together from a shared one-block halo. At most two jobs, including completed results, are outstanding per world; thus a redraw accepts/uploads at most two meshes. Workers receive immutable seed/local-edit snapshots and never access authoritative state or GPU resources. The main thread polls without waiting, accepts only still-desired results matching the current edit revision, and retries invalidated jobs. Revision checks are conservatively world-wide: unrelated edits can also trigger retries. Evicted results are discarded. Pending resident remeshes survive stream replanning; boundary edits invalidate diagonal neighbors for AO. Worker channels close and threads join on world destruction, bounded by outstanding jobs. Stream planning, priority sorting, edit snapshot copying and GPU uploads remain synchronous. Sampling is deterministic whether or not a chunk is resident. Surface heights are roughly 24–800, with mountain growth suppressed near spawn. The full legal range is -512..1535. Mountain surfaces within the view radius can render at any generated elevation. Horizontal fog fades between 112 and 160 blocks; the camera far plane is 1,024 blocks to include tall nearby peaks. Conservative chunk AABB tests independently cull camera and sun-shadow draws. CPU meshing uses a one-block halo with shared column height/tree data (324 column evaluations instead of up to 5,832 for an unloaded halo), shared-face culling and diagonal-aware corner AO. LOD and greedy meshing remain future performance work; this iteration retains exact voxel geometry and existing terrain generation/save compatibility. Rendering uses a 2048-square sun depth map with 3x3 comparison filtering, a day/night intensity cycle, fog, and the nearest sixteen warm torch lights. Held torches occupy one light slot. Placed torches expire after 600 simulation seconds, and placement is capped at 256 active torches.

The owned GUI is an immediate-mode triangle builder with rectangle buttons, pointer hit regions, a built-in 5x7 font and resolution-aware scaling. The pack has a 27-slot backpack, nine real hotbar slots, shaped crafting, a recipe pattern guide, a furnace view and attribute allocation. Escape menus and lost focus pause simulation. Opening the pack pauses movement and survival while furnace processing continues. Crafting recipes are instant transactions; furnaces process automatically over five seconds per item. Tables and furnaces must be within three blocks. A wood bedroll recipe substitutes for the later wool economy. Leaves supply berries as a renewable slice food source. Husks provide combat and meat; animals and audio are deferred. All visuals are code-native voxel geometry and glyphs, so no asset authoring tools or generated raster assets were used.

Vigor adds 3 health per point; Strength adds 2.5% melee damage; Dexterity reduces the attack interval through a 1.5%-per-point speed multiplier; Agility modestly improves walking/jumping and reduces falls. Ingenuity grants one bonus output every `24 - points` crafts (only when allocated). All stats cap at 20. Wisdom and Willpower remain visible but cannot consume points before magic is implemented. First recipe XP is quartered per repeat to zero. First biome/resource discoveries grant XP once. Kill XP falls with player level and reaches zero for sufficiently weak enemies. Death restores the player at spawn and retains inventory. Temperature and magic are deferred.

Saves use a synced temporary file followed by atomic rename. Unsupported versions, malformed JSON, invalid positions, inventory selection/count/wear, stat bounds and torch timer inconsistencies are rejected. Load failures preserve existing files. A failed save keeps the quit action from closing the window. Save format v3 requires the current recipe count and stores placed furnace states; v1/v2 saves migrate on load. Version 1 aggregate inventories migrate to real stacks on load; changed recipe registries will still require explicit migration. Concurrent writers to the same save are not supported.

## Developer tools and validation results

- `tools/verify.sh`: quiet formatting check, Clippy with warnings denied, nextest, and headless gameplay smoke. It keeps output in a temporary log and prints only a success line or the last 60 diagnostic lines on failure.
- `cargo run -- --smoke`: deterministic landing, real reach-based log mining, crafting, XP and save/load; uses and removes a unique temporary world file.
- `cargo run -- --frames 180 --capture /tmp/frame.ppm --save /tmp/test-world.json`: bounded windowed run, final GPU framebuffer readback to dependency-free PPM, save and clean exit. Add `--pack` for GUI inspection. Captures default to 240 frames when no frame count is specified. These flags are diagnostic helpers, not gameplay requirements.

Validated locally: quiet nextest tests, clean short-format Clippy with warnings denied, formatting, and headless smoke. Tests cover negative chunks, determinism, distant mountains, eviction/edit persistence, DDA face selection, hidden-face meshing, generation-safe entity reuse, recipe atomicity, station-gated wood/stone/copper progression, tool durability, copper mining requirements, XP decay, collision/landing/jumping, stat caps, torch expiry, sleep, save round trips and invalid saves. Windowed Vulkan checks captured both the world HUD and inventory/crafting GUI; shaders and shadow/render passes initialized and rendered successfully, and the corrected native teardown exited normally. This is an initial playable slice, not a validation of every GPU/desktop combination or a long-duration balance/playtest.

Terrain/view-distance validation: regression tests compare cached halo sampling to normal queries across negative coordinates, resident chunks and edits; verify streaming above the old player-relative mountain ceiling, radius changes and obsolete job eviction; and check perspective/orthographic frustum rejection and intersections. Formatting, Clippy, quiet nextest and gameplay/persistence smoke passed.

A 900-frame native Vulkan run completed with 1,161 resident chunks, captured the extended terrain view, saved to a temporary world and exited cleanly. The capture reported 60 FPS after timed redraw pacing (the preceding FIFO-only run reported 427 FPS). This verifies pacing/rendering on this desktop, not a comparative performance benchmark. This run predates background streaming; LOD remains deferred.


Background streaming validation: regression tests exercise edit invalidation while immutable jobs are outstanding, generated block equivalence, byte-for-byte mesh equivalence to synchronous sampling, obsolete result rejection after movement, resident remeshes across replanning, and the two-job bound. Existing streaming tests wait with deadlines for asynchronous residency. Headless worlds start no threads until streaming is requested. No save-format or terrain-generation changes are required.

Validation for this change: formatting, Clippy with warnings denied, all 30 quiet nextest tests, and gameplay/persistence smoke passed. A native 180-frame capture was attempted but could not initialize winit because this session has no accessible Wayland compositor; GPU presentation and comparative frame timing remain unverified for this change.

VSync update validation: formatting, short-format Clippy with warnings denied, quiet nextest, and gameplay/persistence smoke passed. The initial sandboxed native run could not connect to the Wayland socket (EPERM). Repeating the 120-frame Vulkan run outside the sandbox connected to the compositor, rendered, saved to a temporary world, and exited successfully. Refresh-rate timing was not measured. The historical 60 FPS capture above used the removed software timer.

## Measured playtest and streaming follow-up

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
stress route, not a walked gameplay expedition. `tools/expedition.rs` implements
`--expedition`: fresh inventory, controlled resource arena, DDA mining, recipe
transactions, station placement, copper gear, combat/loot, eating, sleep blocked
by enemies and restored after victory, persistent torches, natural forest
and cave relocations, and full player/edit/torch/time save comparisons. Fixture
resources and relocation deliberately isolate mechanics from search/navigation.
The verification helper now runs this expedition as well as the original smoke.

Measured replanning hitches motivated a bounded cache of per-horizontal-chunk
surface height bounds. New columns still sample all 256 heights; overlapping
columns reuse exact seed-derived bounds. Cache eviction follows the horizontal
view, independent of resident voxel chunks. Edits do not affect procedural
surface heights. Replanning uses hash-set membership for queued remesh merging,
and queue sorting runs only when priorities change rather than every redraw.
Regression coverage compares desired chunks and cached bounds to fresh worlds
across negative coordinates, elevation changes, distant moves and radius changes.
See PLAYTEST.md for timings, actual checks and limitations.

## Slot inventory and shaped crafting

Inventory now owns 36 actual slots: indices 0–8 are the hotbar and 9–35 are
the backpack. Materials stack to 64; tools occupy individual slots and retain
independent wear. All item icons are original 16x16 pixel shapes built directly
as colored UI geometry, with material shading and details. No external assets
or rendering dependencies are needed. Slot counts, durability bars, hover names,
and a carried stack replace abbreviated item buttons. The HUD uses the same icons.

Left-click picks up, places, merges or swaps a stack; pressing on a stack and
releasing on another slot also moves it. Right-click picks up half (rounded up)
or places one. Crafting outputs support click pickup and dragging to inventory.
E opens/closes the pack; using a placed table/furnace opens its tab. Nearby-station
tabs remain available within the existing station detection volume. The personal
grid is 2x2, and a table exposes a separate 3x3 grid. Both use a fixed row stride
of three; personal cells outside its 2x2 area must remain empty. Closing menus or changing tabs retains all inputs
and the carried stack; these are serialized, including when saving from gameplay.

Shaped matching requires empty cells around the pattern, permits translation
within the grid and horizontal mirroring, and ignores stack sizes above one per
occupied cell. Planks, sticks, tables, tools, torches, furnaces and swords use
familiar arrangements. Wooden tools now require a table. Copper substitutes for
other tool materials. The existing bedroll is custom: six planks across the top
two rows, and logs in the bottom corners. The read-only recipe guide displays
patterns and yields without inserting ingredients. Taking a valid output consumes
one item per occupied cell, checks carried-stack compatibility/capacity first,
and grants existing craft XP and Ingenuity rewards.

Furnace input and fuel slots are separate from crafting. Copper ore or raw meat
smelts automatically in five seconds (300 fixed 60 Hz ticks) into a separate
output stack. Taking output collects finished goods; it never starts a smelt.
Output stacks to 64; a full or incompatible output pauses work without consuming
fuel or energy. Input is consumed only on completion. Changing or removing the
input type resets progress; fuel energy already used is not refunded. Replacing
the same input type preserves progress.

Coal adds 4,000 internal joules only when active work needs energy, powering four
copper ingots or cooked meat at 1,000 joules each. Integer cumulative energy
costs drain exactly 1,000 joules over each 300-tick smelt without rounding drift.
Fuel insertion alone consumes nothing; idle furnaces retain their energy. The UI
shows a loaded-fuel/remaining-energy bar, a smelting progress bar, and completed
output. Escape menus and lost focus pause furnace processing too; opening the
pack allows furnace-only fixed steps while the rest of gameplay remains paused.

Input, fuel, output, energy, progress and tracked input type belong to each placed
furnace, keyed by block coordinates in `Game::furnaces`. Every stored furnace
advances on each active fixed step, including outside resident terrain; there is
no offline elapsed-time catch-up. The same stepping runs with the pack open.
All processing state persists in v3 saves; old v2 fields default to empty/zero
before migration. Stored energy is validated in
0..=4,000 and progress below 300 ticks. Direct instant furnace crafting is disabled
so it cannot bypass processing; the expedition advances the same furnace steps
as gameplay. Craft XP is awarded once on completion, not on output collection.

Save v2 validates slot count, stack sizes and individual wear, including grid,
furnace and carried stacks. V1 counts split into stacks, old hotbar preferences
are mapped onto real items, and old wear is attached to the first matching tool.
If the old inventory exceeds 36 slots, migration returns an explicit error and
preserves the source file. Mining leaves resource drops in the world until collected. Craft transactions refuse full output inventories without consumption.

Validation: formatting, short Clippy with warnings denied, quiet nextest, smoke,
and the expedition. The expedition now arranges actual crafting/furnace inputs
and takes outputs through the same transaction as the UI. Regression coverage
includes every recipe pattern/yield, offsets/mirroring/extra ingredients,
stack splitting/merging/swapping, independent tool wear, full-output atomicity,
furnace fuel validation, legacy migration and overflow preservation, held/input
save roundtrips, invalid stacks, and scaled UI hit regions. A bounded native
Vulkan pack capture rendered and exited cleanly; its layout was inspected.
Live mouse ergonomics and prolonged play remain manual validation.

Timed furnace validation: formatting, short Clippy with warnings denied, quiet
nextest, gameplay smoke, and expedition passed. Regression tests cover elapsed
smelting time, four ingots per coal, blocked-output pause and collection atomicity,
idle energy retention, input-type changes, cooking, saved progress/output/energy,
old v2 defaults, and invalid energy. The updated furnace UI has not been checked
in a native window for this change.


## Placed furnace persistence

Each placed furnace owns a typed `items::Furnace` state. Right-clicking a furnace
selects that exact block for the screen, slot transfers and output collection.
The pack's furnace tab uses the nearest station within the existing station
volume (three blocks horizontally, two vertically), with coordinate tie-breaking.
E resets explicit selection. An explicit station that becomes unreachable never
redirects transactions to another furnace. Crafting grids and the carried stack
remain player-owned. Inventory transactions temporarily exchange only furnace
state with the selected block, then restore it; no active copy remains on the
player. Completed smelts award crafting XP once, independently for every furnace.

Mining refuses a furnace containing any input, fuel or output, leaving the block,
items and tool wear intact. Empty furnaces can be mined; their saved state is
removed and remaining energy/progress is discarded. Replacing one starts empty.

Save v3 serializes a coordinate/state list, rejecting duplicate coordinates,
states without a placed furnace edit, invalid stacks, energy or progress. Existing
v1 inventory migration still runs. V1/v2 shared processing state moves once to
the placed furnace nearest the saved player position, with coordinate tie-breaking.
If none exists, the legacy flattened inventory fields preserve the state without
processing it; the next successful furnace placement transfers it once. V3 reloads
never repeat migration. Existing inventory, tools, terrain edits and progression
are retained. No user save is rewritten until the normal save action runs.

Validation: formatting, short Clippy with warnings denied, quiet nextest, gameplay
smoke and expedition passed. New regressions exercise independent ore/meat furnaces,
blocked-output isolation, processing far from the player, station-specific input
and output transactions, single XP awards, safe mining, saved partial progress
and energy for multiple furnaces, invalid/duplicate states, migration with and
without placed stations, and one-time legacy recovery on placement. The expedition
also compares all furnace states after reload. README now describes five-second
smelting and station ownership. Native mouse interaction has not been manually
playtested for this change.

## Quick transfers and presentation priority

Shift-left-click moves stacks directly between inventory and the active crafting
or furnace slots, merging matching stacks before filling empty cells. Personal
crafting uses only its four valid cells. Furnace routing accepts coal as fuel
and copper ore/raw meat as input; rejected or excess items remain at the source.
Ingredients can be returned to inventory with wear preserved. Shift-click output
collects one crafting result or the completed furnace stack into inventory only
when the entire result fits, preserving the carried stack and normal XP rules.
Placed furnace transfers use the selected station transaction and reach checks.

Player feedback identifies presentation as the next priority: held tool/view
models and action animations, synchronized mining/combat audio, impact/break
particles, and clear hit feedback. These should make existing actions readable
before expanding progression content.

## First-person action feedback

`feedback` owns transient fixed-step swing/hit/hurt timers, a presentation clock,
a capped 192-particle pool and up to eight queued sound cues. None is serialized.
Mining cadence emits material-specific impacts only after tool requirements pass;
successful block removal emits a larger break burst. Successful melee damage
emits a hit marker and particles. Health loss triggers a brief red border, with
repeated damage cues throttled by its lifetime. Holding attack repeats a 0.32 s
swing, including misses; combat damage retains its existing independent cooldown.
Particle gravity is cosmetic, without voxel collision. Effects freeze with gameplay.

The renderer builds voxel hand/tool silhouettes for axes, picks and swords,
material variants, and simple held blocks/other-item placeholders. Camera-relative
geometry includes swing and grounded movement bob. A separate color-load/depth-clear
pass draws the held item over terrain without casting a world shadow; it uses
existing world lighting. Menus hide it. Break chips use source block colors;
melee particles are green impact motes. Hit and damage overlays use the owned GUI.

`audio` synthesizes short mono noise/tone cues, mixed with a 12-voice cap on a
background worker and an eight-cue nonblocking channel. Optional `paplay` streams
22,050 Hz s16 PCM to PulseAudio or PipeWire; no new Cargo dependency or authored
assets are required. Missing playback support disables audio; `--mute` skips
worker/process creation. Audio is currently non-spatial with a fixed conservative
mix. Sounds already submitted may finish after pausing. This is a first feedback
pass; food, placement, footsteps, creature locomotion animations and authored
sound design remain follow-up work.

Validation: formatting, short Clippy with warnings denied, quiet nextest, smoke
and expedition passed. Regression checks cover bounded particle/cue storage,
effect expiry and successful versus tool-rejected mining feedback. A native
120-frame Vulkan capture rendered the held axe and exited cleanly; capture
inspection confirmed visible tool/hand placement. Live swing/impact timing and
sound quality have not been manually evaluated.

Audio buffering correction: PCM production is paced in real time at one 441-frame
chunk per 20 ms, independently of pipe backpressure. After a worker stall the
schedule restarts from the current time instead of generating catch-up chunks.
Cues carry emission timestamps and are discarded after 100 ms in the queue.
This prevents the pipe from accumulating seconds of future audio and replaying
old swings after release. Already playing cues retain their short decay tails.
A regression test uses a freely accepting fake sink to verify that 200 ms of PCM
cannot be generated faster than real time; another checks stale cue rejection.
Formatting, short Clippy, quiet nextest, smoke and expedition passed. Live desktop
listening has not been repeated for this correction.

## Creating new worlds

The pause menu offers New world (saves current). The current save must succeed
before creating or switching worlds; failure keeps the current game active.
New worlds receive a fresh seed from the standard library randomly seeded hasher,
initial player state and an exclusive `world-N.json` file beside the current save.
Existing names are skipped using create_new, including the current filename.
The initial save is written and synced before switching; a failed initial write
removes its incomplete file. Switching clears renderer meshes, inventory UI,
input and simulation/autosave accumulators. The window title identifies the
active save path. Resume a selected file with the existing --save option.

--new creates a separate save at launch, using a fresh seed unless --seed is
provided. --save chooses its parent directory; it never overwrites that path.
Regression coverage checks distinct new saves, seeds on reload and preservation
of existing files. Native menu interaction still requires a desktop playtest.


## Floating world drops

Breaking blocks and defeating creatures creates persistent item drops at the
source position. Drops render as camera-facing versions of the inventory icons,
with a gentle bob, terrain depth testing and an 80-block draw limit. Drops launch with a small upward and sideways impulse, fall under gravity,
and collide with solid voxels using a 0.4-block box. Substeps prevent tunneling;
impacts lose energy, giving a small bounce before settling. Removing support
makes settled drops fall again. Icons retain a subtle hover bob above their
physical position. Drops have no expiry. Within 1.4 blocks of the player body,
drops that fit in inventory steer toward the player at up to seven blocks per
second, retaining voxel collisions. They enter inventory only within 0.35 blocks
of the body, so attraction is visible before collection. Attraction replaces
gravity while in range; leaving range or filling the inventory restores gravity.
Resource discovery XP is awarded on collection. Full inventories leave drops
untouched and do not prevent mining; each drop transfers atomically.
World drops are stored in saves with validated positions, velocities and stack counts.
Older drops without velocity load at rest.
Existing saves default to an empty drop list. Pausing freezes animation and pickup.
Smoke and expedition helpers explicitly approach their fixture loot.

Successful world-item collection queues a short, bright rising pickup tone through
the existing audio worker. Multiple drops collected in one step share one cue;
out-of-range drops and pickups blocked by a full inventory remain silent.

## Iron expeditions and tundra ruins

The second tier extends the existing registries without changing earlier item
discriminants or recipe indices. Iron veins occur in every biome, below twelve
blocks of cover, and require a copper pickaxe. A table crafts a crude smelter
from six cobblestone and two copper ingots. It shares placed-furnace inventory,
reach, transfer, fuel, XP, mining protection and persistence rules, but can also
process iron. Basic furnaces cannot process iron and consume no fuel trying.
Both processors take five seconds and 1,000 joules per item. Iron axes, picks and
swords use the existing tool patterns; durability is 600, harvesting tier four,
and sword damage 19 before stat bonuses (copper: 360, tier three, damage 14).

Tundra uses a separate smooth climate field outside a 240-block spawn exclusion.
It has snow-colored solid surface blocks, no oak trees, and the existing hill,
mountain, cave and ore generation. Temperature, clothing and snow accumulation
remain future systems; current expedition preparation consists of food, lights,
a bed/camp and better weapons. Natural generation now includes the new biome
and ores in existing worlds too: generated terrain is reconstructed from seed,
so untouched terrain can change, while saved block edits remain authoritative.
This is an intentional content-generation change, not a save reset.

Each 256-block horizontal cell has one seeded ruin candidate. Only tundra sites
with gentle corner elevations qualify. A 13-block square foundation, broken
walls, four taller pillars and a south entrance surround a blue iron cache.
Foundation fill and interior clearing make the ruin traversable. Column halo
generation and direct sampling use the same structure rules; normal chunk bounds
include the structures. Ruins use code-native colored voxels and require no assets.

Approaching within 28 blocks summons one ruin guardian. It has 65 health,
level four, 2.2-block/second pursuit and 12 melee damage, compared with the husk's
30 health, level one, 1.7 speed and seven damage. Blue-gray body/head colors and
the target label distinguish it. The guardian uses existing collision, line of
sight, melee, hit feedback and loot systems. Defeat drops coal and grants a
one-time 60-XP completion discovery in addition to normal kill XP. Initial ruin
discovery grants 50 XP. The cache refuses mining until that ruin's guardian has
been defeated, then drops six iron ingots. Its removed block persists, so the
reward cannot be farmed by reloading. Guardian completion is keyed by coordinates
in existing player discoveries. Living guardians remain transient and respawn
when revisiting/reloading an unfinished ruin; defeated ones do not.

Save v3 retains its existing structure. New items/blocks serialize by enum name,
placed smelters use the furnace-state list, and 15-entry craft histories expand
to twenty entries with zero counts for new recipes. Legacy v1 inventory migration
only indexes the original 22 items. Unknown/malformed recipe-history lengths
still fail validation. No user save is rewritten during migration alone.

Validation includes all recipe patterns, basic-furnace iron rejection, saved
half-finished iron processing, exact fuel accounting, copper-pick gating,
deterministic tundra/ruin sampling and cross-chunk halos, sealed-cache rewards,
single guardian spawning/completion, legacy craft history expansion, placed
smelter persistence and reload without guardian/reward regeneration.
The expanded tools/expedition.rs mines and crafts through iron using normal
transactions, then relocates to a natural tundra ruin, fights its guardian and
collects the cache. It retains controlled resources and diagnostic travel;
natural discovery difficulty and combat balance still need human play.

Native validation: a bounded 900-frame release Vulkan session using a temporary
ruin save rendered and exited cleanly. Capture inspection confirmed ruin geometry,
the guardian, held iron sword, new hotbar icons and damage feedback. The session
was muted. See docs/PLAYTEST.md for measured frame times and remaining manual checks.

## Automated functional playtesting

`tools/playtest.rs` provides a deterministic, display-free mechanics course at
seed 314159, with cleared headroom and a stone floor at y=299. Fixtures set up
conditions (including low hunger, a high fall, death, and a shortened torch timer);
normal `Game::tick` calls at 60 Hz drive each action. Bounded frame counts and
checkpoint-specific errors make failures repeatable without real-time waits.
The course checks walking distance, sprint speed and its hunger gate, hunger/rest
drain, jumping and landing, wall collision, fall damage, starvation, respawn
vitals/inventory retention, and torch presence before/after expiry.

`playtest::tests::mechanics_course` and
`playtest::tests::progression_expedition` are ordinary nextest tests, so the
functional scenarios run with `cargo nextest run`. The latter reuses
`tools/expedition.rs` for mining, crafting, station placement, timed processing,
combat/loot, eating, sleep safety, biome discovery, ruin completion and save/load.
`cargo run -- --playtest` runs both with concise success reports and a nonzero
exit on failure; `tools/verify.sh` also checks that command-line entry point.
The expedition uses a temporary save, cleans it up on success or failure, and
never opens the player's save. Neither scenario starts a window or audio worker.

These are mechanics contracts, not an autonomous exploration agent: controlled
resource blocks, inventory/grid setup and diagnostic relocation bypass resource
search, mouse/UI interactions and travel difficulty. Shortened timers avoid
waiting for long survival cycles. Rendering, audible sound quality, input wiring,
performance, balance and fun still require separate native/human checks. Existing
unit tests cover detailed transaction rejection and persistence edge cases.
Add new functional checkpoints here or extend the expedition as systems arrive;
keep fixtures explicit and assert observable outcomes through game actions.

## Pixel material art

`art` owns deterministic 32×32 RGB material tiles for every current block,
with separate side/top/underside layers. Broad pixel clusters establish bark
ridges, cut-log rings, grass overhangs, soil pebbles, granite veins, embedded
ore clusters, leaf clusters, staggered plank joints, masonry mortar, furnace
openings, crafting grids, cloth, snow, sand ripples and the cache emblem.
Full-resolution grain adds fine variation. Ruins have moss and cyan carvings.
The textures are generated from fixed integer hashes at startup, without files,
network access, world generation changes or save changes.

The renderer uploads a 60-layer texture array with six box-filtered mip levels
in linear color. Nearest texel filtering preserves close pixel edges; linear
mip filtering reduces distant shimmer. World vertices carry UV/layer attributes
and retain existing corner AO and seeded block brightness variation. Untextured
geometry uses a negative layer sentinel. Face counts and chunk geometry are
unchanged; vertex stride increases from 36 to 48 bytes. The shadow pass uses the
same vertex layout. Inventory block icons sample the same materials within their
existing 16-pixel silhouettes; held block cubes use full material UVs.

Held tools gain grip wraps, collars and brighter cutting edges. Creatures gain
recessed faces, eyes, brows, belts and buckles, with moss patches for husks and
masonry shoulder armor/cyan chest details for guardians. Rendered creatures face
the player; this is cosmetic and does not change AI or collision bounds.

Validation: short Clippy, all 68 quiet nextest tests, and diff whitespace checks
passed. Material regression coverage checks complete mip sizes/opaque pixels,
face-specific textures and distinct ore layer mapping. A bounded 240-frame native Vulkan run exited cleanly; its terrain capture was
inspected for grass, soil edges, bark, foliage, shadows and the held axe. Inspection
caught and corrected float-to-layer truncation artifacts by rounding the tile
layer before integer conversion. The smoke gameplay/save round trip also passed.
Creature details and all crafted materials have not been individually inspected
in native views.

Torch appearance and mounting: placed and held torches share a procedural mesh
with a split-grain wood handle, linen head, dark binding and stepped amber flame.
The flame uses an unlit material marker so it stays visible at night. Wall torches
lean outward from the clicked support face; floor torches stand upright. Torches
require a solid wall or floor, reject other torches as supports, and disappear
when their support is removed. Support offsets persist in an optional
`torch_mounts` save field (older saves retain upright torches), and worker snapshots
carry these offsets so streamed meshes preserve orientation. Torch light positions
follow the flame head; held and placed light strength is 0.8, reduced from 1.0,
with the existing nine-block falloff radius retained.

Torch validation: all 69 quiet nextest tests, short-format Clippy, formatting and
headless gameplay/save smoke passed. Regressions cover wall placement, rejection
of torch supports, support removal, and saved attachment round trips. The updated
appearance has not been checked in a live GPU window.

## Ground vegetation and light-transmitting leaves

Natural generation places non-solid shrubs, tall-grass and six colored flower blocks
above grass surfaces, with the same seed/coordinate hash used by their meshes.
Three tapered blade tufts vary in position, orientation, height and green shading.
Grass stature varies per plant from 0.16 to 0.95 blocks, with individual
tufts at 72–100% of that height, mixing short growth and nearly block-high blades. A smooth seeded 14-block density field gives 2% placement in open
regions, 12% in transition areas and 38% inside denser tufts. Flower placement
keeps its previous independent abundance. Flowers have bent two-segment stems,
paired angled leaves, four to six irregular petals, tilted heads and golden
centers in red, yellow, purple, white/cream, pink and blue. Petal size, rotation,
height and shade vary deterministically. Shrubs occur at about 2% of remaining
sites, with seven asymmetric leaf clusters and woody branches. They are
non-solid, harvest without a drop, and share plant support/removal persistence.
All vegetation geometry remains inside its own voxel. Snow, sand
and ruin floors receive no plants. Existing unedited terrain gains these plants.

Plants occupy their own streamed voxels, are selectable with reduced height/width
bounds and use the regular mining, particles and world-drop systems. They break
in 0.12 seconds by hand or tool without consuming tool durability. Tall grass
drops one wheat seed at one in eight seeded coordinates; flowers always drop
one item of their color. Breaking plants writes ordinary air edits so they stay
removed across streaming and reload. Removing their dirt support or replacing it with non-dirt
removes the plant. Bare and grass-covered dirt both support plants. Sampling
checks the actual supporting block, including natural outcrops and saved edits.
Flower items can be replanted on either form of dirt, and each crafts into one matching dye
in any personal/table crafting cell. Seeds and dyes are collectible ingredients;
wheat cultivation and dye application remain future systems. Flowers, seeds and
dyes have distinct inventory/drop icons; held flowers use the 3D plant mesh.

Item/block variants and six hand recipes are appended to preserve existing
indices. Saves retain v3, with enum names for new items and blocks; known legacy
craft histories of fifteen or twenty entries expand to the current recipe count.

Leaf faces use a stable world-space perforation mask in both color and shadow
passes, allowing views and sunlight through small gaps. Internal leaf faces and
solid faces beside leaves remain visible through those gaps. Leaf and grass
lighting includes softer reverse-side sunlight transmission. This approximates
translucency with cutouts and backlighting; it does not use blended transparent
voxel faces. The shadow pipeline now has a depth-only fragment stage for the
mask and renders both sides of grass blades. Existing collision rules remain.

Validation: all 79 quiet nextest tests and short-format Clippy passed. Mesh and
world regressions cover repeatable shapes, sparse varied grass, natural/halo
agreement, plant selection and support removal. Gameplay checks cover chance
seed drops, all six flower drops and preserved tool wear. Craft pattern tests
cover all dye recipes; save checks cover harvested natural plants, placed
flowers, new items and twenty-entry craft-history migration. Native capture
could not start because the session has no Wayland compositor; visual quality
and GPU shader validation still need a desktop run.

## Dirt and spreading grass cover

Both bare and grass-covered soil are labeled Dirt and drop/place the existing
Dirt item. The serialized `Grass` variant remains the internal covered-soil
state to retain save compatibility; `Dirt` is bare soil. Surface cover and tall
grass objects are independent, so gaining cover does not respawn harvested plants.

Every five active simulation seconds, exposed bare dirt within 32 blocks of
the player can gain cover from a horizontally adjacent covered dirt block,
including a one-block height difference for slopes. Both source and destination
require a non-solid block directly above. Exposure here means an uncovered top
face; it does not require a sky ray. Pausing stops this timer; reloading restarts
the five-second interval. Candidate positions come from edits and blocks directly
below edits, including natural dirt uncovered by excavation. Unedited natural
surfaces already have cover. Each pulse applies up to 32 simultaneous conversions
so cover advances one step per pulse; ordinary block edits persist the result
and invalidate streamed meshes. No scan of all resident voxels is needed.

Grass-covered dirt wraps its grassy material around exposed one-block side
faces. A cliff is detected per face when the block below continues the solid
column but the neighboring block below that face is empty/non-solid: this is
at least a two-block drop. Cliff sides retain the existing thin grassy lip
and bare soil beneath. Meshing uses the chunk halo, including across boundaries;
changing adjacent terrain updates the classification. Isolated soil cubes also
get grassy sides, since they have no continuing cliff column.

Regression checks cover sparse/clustered grass distribution and height, soil
spread timing, covered/isolated soil rejection, slope and chunk-boundary spread,
natural dirt exposed by digging, saved cover, and step-versus-cliff materials.
The latest headless tests do not substitute for a native visual check.

## Shrub and turf appearance refinement

Shrubs now vary deterministically between four and ten leaf clusters, with
independent overall spread, stature, rotation, cluster size and olive-to-cool-green
color variation. Woody branches retain opaque shading. Shrub leaf geometry uses
material marker -4 for fine world-space cutout gaps in both color and shadow
passes, plus the existing reverse-side foliage sunlight transmission. This gives
partial visibility and backlit leaves without sorted alpha blending. Shapes stay
inside their plant voxel and existing selection bounds.

Grass-covered soil now uses native 32px overlapping narrow blade strokes with
varied length, bend, shade and highlighted tips, replacing broad two-pixel green
clusters. The same blade material extends through the uneven grassy cliff lip;
step sides continue to use the full turf texture. Other materials retain their
existing clustered grain and all tiles retain their generated mipmaps.


## Procedural skybox and day/night lighting

The existing saved clock drives a full-screen procedural skybox rendered before
world geometry without writing depth. Inverse camera projection reconstructs
world-space viewing directions; translation does not move the sky. No authored
cubemap assets are required. The sky blends horizon and zenith colors through
day, twilight and night, with a warm glow toward the rising/setting sun, visible
sun and moon discs, and fixed spherical star cells that fade in at night.
Distance fog samples the same atmosphere so terrain blends into the horizon.

One day remains 900 active simulation seconds. Phase zero is sunrise, 0.25 noon,
0.5 sunset and 0.75 midnight; new games and sleep retain their existing 0.28
phase. Saving/loading retains the clock, and pausing stops it. The sun moves
along a tilted orbit; the opposite moon provides weaker cool light at night.
The shadow camera follows the active celestial light, while ambient brightness
and directional color transition with daylight. Torch lighting stays additive,
so it becomes more prominent at night. This is raster lighting with one shadow
map, without atmospheric scattering or moon phases.

Validation: all 81 quiet nextest tests and short-format Clippy passed. The orbit
regression checks horizon crossings, noon/midnight brightness, unit directions
and repeatability across days. A native three-frame renderer check could not
start because no Wayland compositor is available in this session; GPU shader
validation and visual review require a desktop run.

## Offscreen headless runner

`tools/headless.rs`, included by main as a Rust module, implements `--headless`.
Main loads/creates the selected world and branches before creating the winit
event loop or audio. The renderer shares initialization and every world, shadow,
sky, held-item and UI render pass between surface-backed and offscreen targets.
The headless adapter request has no compatible surface; its framebuffer is an
RGBA8 sRGB texture with render-attachment/copy-source usage. Existing padded
GPU readback writes the final PPM. Both paths drain GPU work before teardown.
No new dependencies or save-format changes are needed.

Headless execution uses one fixed simulation step per frame, no player input,
and no realtime pacing; pack mode ticks only furnaces. Existing frame, capture,
pack, seed/save, tour and profile options apply. The default limit is 240 frames,
and zero frames are rejected. Terrain workers are polled until their queue and
active jobs empty at startup, tour relocations and the final frame; each wait
has a 120-second deadline and a 1ms polling delay. This avoids incomplete final
terrain from fast offscreen loops, though intermediate streaming is asynchronous.
CSV fields retain their existing meaning; streaming waits are included and
headless draw times have no compositor/VSync component. The runner flushes the
profile and saves on successful completion; render failures propagate to a
nonzero process exit. README.md documents usage and software Vulkan support.

Headless validation: with DISPLAY and WAYLAND_DISPLAY unset, a two-frame Vulkan
run completed, wrote a 1280×800 PPM with verified header/pixel count and varied
color data, flushed CSV timings, and saved the world. All 81 quiet nextest tests,
short-format Clippy, formatting, and gameplay/persistence smoke passed. Software
Vulkan and interactive presentation were not separately exercised for this change.

## Cave formations

Caves now use seed-hashed 160-block cells with three shapes: low winding
passages, tall narrow crevices, and chasms up to roughly 44 blocks wide and
48 blocks high. Elliptical ends and sinusoidal lateral bends shape each formation.
Their floors lie about 48 blocks below the cell's terrain anchor. Roughly a
third of candidates also carve a six-block-high descending entrance corridor;
entrances are omitted where the endpoint differs from the anchor by more than
12 blocks, keeping ramp steps at most one block. Openings cut through the surface
soil and vegetation, and trees cannot root over a carved opening. The inner
32-block spawn region, bottom four world layers, and authored ruin foundations
remain protected. Formations are local features; they do not guarantee a connected
worldwide cave network or introduce underground biomes.

Cave intervals are computed once per column in chunk halos, using the same rules
as direct terrain queries. Block edits still override generation. This intentionally
changes generated terrain in existing saves without changing the save format;
previous player edits persist. The expedition helper searches the full underground
range for cave collision fixtures. Regression coverage checks all formation scales,
surface openings, bedrock, spawn safety and halo/query agreement across three seeds.

## Cave darkness and moonlight

Terrain workers attach a sky exposure value to vertices using the highest opaque
voxel in each sampled vertical column, including saved edits above the chunk.
Leaves transmit sky light. Column results are cached per mesh. Edits invalidate
resident meshes throughout neighboring vertical columns so building or removing
a roof updates exposure below it. Dynamic world objects use the same calculation.
Ambient light, celestial light and atmospheric fog are multiplied by exposure;
sealed caves therefore have no illumination without torches. Torch flames remain
emissive and torch lighting remains additive. Midnight ambient is near zero and
moonlight peaks at 2.5% of sunlight, fading toward the horizon.
Nearby sky openings now contribute indirect light along sixteen horizontal or
upward rays, blocked by opaque voxels. Light fades quadratically over twelve
blocks into an entrance, reaching zero in deep caves or sealed rooms. Exposure,
occupancy and roof queries are cached per mesh; worker snapshots include edits
within that reach across chunk boundaries. This approximates entrance skylight;
multiple light bounces and torch occlusion through walls are not simulated.
Existing shadow maps continue to provide directional surface shadows.

## Darkness-gated monsters and movement controls

Husks and ruin guardians spawn only when sky exposure multiplied by the current
daylight is at most 0.1 and nearby held/placed torch light is at most 0.1. The
spawn check reuses the renderer's sky-access sampling, including roofs and nearby
openings; torch falloff uses the same nine-block range. Existing monsters remain
when light changes. Husk candidates search supported empty positions within eight
blocks vertically of the player before trying the surface, allowing cave spawns.

Double-tapping W within 300 ms starts sprinting until W is released. Key repeats
do not count as taps; focus/menu capture resets the gesture. Either Shift key
walks at 45% speed and overrides sprint. Sprint retains its hunger requirement.
Grounded movement automatically jumps toward one-block obstacles with headroom;
Shift disables this. Shift walking checks support at every horizontal collision
substep on both axes and stops at ledges, including diagonal approaches. Space
continues to provide manual jumping. Regression tests cover daylight, night, roofs,
torches, automatic climbing, and diagonal edge protection.

## Grassland detail

Grassland keeps open sightlines, with existing tall-grass patches supplemented by
meadow-clustered flowers and sparse berry shrubs between meadows. Existing flower
colors, harvesting and planting rules remain in use. Low stone outcrops use one
seeded candidate per 48-block cell (one third qualify), with centers inset ten
blocks to keep neighboring formations apart. Five families of elliptical lobes
produce rounded boulders, broad slabs, ridges, split rocks and uneven clusters,
with seeded size, rotation and reflection variations and heights of one to three
blocks. A gentle grassland surface check keeps them grounded. They use ordinary mineable stone;
direct queries and chunk halo generation share the same rules. As with earlier
terrain additions, untouched generated terrain changes while saved edits win.

Small groups of up to three sheep appear on clear grassland surfaces near the
player, checked every 18 seconds after the initial four-second delay. At most six
sheep are active, independently of the hostile cap. They graze, wander slowly,
avoid nearby players, step over terrain and never attack or block sleep. Sheep
have 16 health and male/female names; males have small horns. Their code-native
model has a woolly body, four animated legs and a lowered grazing head. They use
the existing meat drop and cooking loop. Sheep, like other active creatures, are
transient and despawn beyond 90 blocks; breeding, wool collection and persistent
livestock are future work. No external assets or helper tools are required.


## Plant soil support and future biome soils

Plants require a dirt block directly below them, whether bare or grass-covered.
Placement, direct sampling, chunk halos and support removal share this rule;
stone, wood, sand and snow do not support plants. Switching between bare and
covered dirt preserves plants.

Future biomes may have differently colored dirt blocks. These variants should
function like ordinary dirt, including plant support, grass cover, digging and
placement behavior. Color is a biome distinction rather than a separate soil
mechanic. Biome-specific dirt colors are planned, not implemented yet.


## Water, lava and buckets

`fluid` adds source-driven discrete fluids. Water and lava are non-solid blocks;
source variants and flowing variants (levels 1–7, or 8 for falling columns) are
ordinary serialized block edits. Each 0.2 active seconds, at most 512 nearby
frontier entries and their six neighbors are evaluated against the old world,
then changes commit together through `World::set`. Lava advances every third
pulse (0.6 seconds). Water loses one level per horizontal step, reaching seven
blocks; lava loses two, reaching three blocks. Downward flow retains its strength.
Horizontal flow needs support beneath its supplying neighbor, so waterfalls
continue down before spreading. Removing sources or building dams wakes adjacent
cells and disconnected flow decays away. Flow replaces plants and extinguishes
torches; solid blocks contain it and can replace it through normal placement.

Water becomes a source with at least two horizontal source neighbors and a solid
block or water source below. This includes filling an empty supported cell and
restoring a scooped corner of a 2×2 pool. Diagonal/vertical neighbors do not count;
lava never renews. Contact on any face converts a lava source to Obsidian and
flowing lava to Cobble. Obsidian is placeable and requires an iron pickaxe to
harvest. This is an approximation of Minecraft-style behavior: no conserved
volume, pressure, slope-search routing, fluid currents, waterlogging, fire
spread or exact edition-specific water/lava reaction rules.

The ordered frontier provides deterministic bounded work. Cells beyond 64 blocks
on any axis sleep until approached. Changed blocks wake themselves and six
neighbors; accepting streamed chunks seeds their fluid cells. Loading a save
rebuilds the frontier around saved edits. Settled fluids require no ongoing full
world scan. Streamed meshes retain ordinary edit revision checks. A growing flow
can invalidate in-flight mesh work; a pulse is not guaranteed to process every
active cell during large changes. Simulation timers and player breath reset on
reload; fluid blocks and bucket contents persist. Save version stays v3, legacy
26-entry craft histories expand, and invalid fluid levels are rejected.

Deterministic 64-block cells occasionally contain small flat, stone-lined pools
of water or lava on gentle surfaces. Pools remain outside the protected spawn
area, share direct/halo sampling, and use ordinary fluid sources. As with other
terrain additions, untouched terrain changes in existing worlds while edits win.
Tundra water ponds have one solid, mineable layer of pale blue, textured ice at
surface level and liquid source water underneath. The pond center's biome decides
the entire lid, so a biome boundary cannot leave half a pond frozen. Ice drops no
item and does not automatically regrow; breaking it exposes the water below.

Seeded 384-block cells can also contain two irregular inland lakes (roughly 40–60
blocks across) connected by a winding, roughly ten-block-wide river. Candidate
sites reject steep endpoint/river elevations and stay outside the starting area.
Each system shares a level water surface, has shallows and deeper lake centers,
and uses sand banks and a sealed bed down to eight blocks below the surface.
These larger bodies remain liquid in every biome, including tundra. Waterways
override small pools and caves; ruins retain priority. Trees whose roots would
intersect waterways or pools are suppressed. Streaming bounds include lake beds
and elevated water surfaces. Direct queries and chunk halos use identical seeded
sampling; player edits still override generated water and ice. Existing unedited
terrain gains these features on load. Rivers currently connect local lake pairs;
there is no global drainage network, current simulation, seasonal freeze/thaw,
ocean generation or underground lava-lake generation.

Tests cover frozen and warm ponds, breaking ice to expose liquid, liquid tundra
rivers/lakes, basin beds, and agreement between chunk halos and direct queries.

Crafting-table bucket recipes use three planks, copper ingots or iron ingots in
a V. Each bucket occupies one slot and carries one source block. Right-click an
exposed source to scoop it, or a surface to pour it next to the hit face. Flowing
blocks cannot be scooped. Wood and copper carry water; iron carries water or lava.
Bucket eligibility and empty/filled variants live in `items`, ready for future
metals when their material progression is added. Craft indices are appended.
Inventory icons and held geometry distinguish bucket material and contents.

Players move more slowly in fluid; drag limits falling speed, Space swims upward,
and landing in fluid avoids fall damage. Submersion follows the rendered fluid
height. Swimming toward a clear one-block bank applies an exit jump once the
eye is above the fluid, using the same obstacle and headroom checks as land
auto-jump. This lets the feet clear the shore despite swim drag.
An underwater eye consumes a 15-second air reserve; exhaustion deals
8 damage/second, and air recovers at five seconds per second above water. Lava
contact deals 25 damage/second. HUD overlays tint submerged views and show air.
Creature swimming, drowning and fluid effects on loose items remain future work.

Water uses a separate alpha-blended pass with depth testing and no depth writes,
after opaque terrain. Chunk buffers partition opaque and fluid vertex ranges;
fluid chunks draw from far to near. Faces shared by equal-height fluid cells are
culled using each cell's effective height: a fluid cell covered by the same
fluid fills its whole voxel. This also applies to neighboring columns across
chunk boundaries, preventing internal translucent strips beneath deep water.
Flow levels lower exposed surfaces. The shader adds ripples; lava is
opaque/emissive in this pass and supplies nearby warm lights through the existing
16-light budget. Exposed lava is grouped into four-block cells to avoid stacking
a light for every voxel; each group uses strength 0.22. Lava light also suppresses
nearby hostile spawning. Fluids transmit sky light and cast no shadow. Chunk sorting is
an approximation: transparent faces within a chunk are not sorted individually,
and there is no refraction or reflection simulation.

Validation: 101 quiet nextest tests pass, including seven-block range across
chunk boundaries, gravity, draining, dams, slower/shorter lava, contact reactions,
supported source regeneration and 2×2 refilling, sleeping distant frontiers,
height-aware submersion, natural-pool halo agreement, bucket gameplay for all
three materials, swimming/damage, mesh surfaces, saved fluids/buckets and legacy
craft-history migration. Short-format Clippy and the gameplay/save smoke pass.
The offscreen Vulkan renderer also completes with the new fluid pipelines.
A captured side-by-side water/lava basin was visually inspected, including the
transparent water, lava surface, bucket icon and held bucket. The first capture
exposed excessive light stacking; grouping/reducing lava lights corrected it in
a second inspected capture. Interactive swimming has not been manually played.


## Title screen, graphics settings and temporal reconstruction

Normal interactive startup opens the title screen with New game, Load game,
Settings and Quit over a frozen terrain vista. Title/menu screens release the
pointer and suspend simulation and autosaving. The preview is not an active save:
leaving the title never writes or replaces a world. New game creates a fresh
random seed in a separately reserved save file; returning from a running game
first saves successfully. Load game browses JSON files alongside the selected
`--save` path, excludes `settings.json`, pages six worlds at a time, and reports
malformed/missing saves without replacing the current game. The explicit selected
save appears first. Mouse buttons and Up/Down + Enter/Space navigate the title,
load and settings screens; Escape goes back. The pause menu offers settings and
Save and return to title. `--new`, `--pack`, and diagnostic `--frames` retain direct
world startup unless `--title` is requested.

`src/settings.rs` owns validated, default-compatible JSON preferences, atomically
written to `saves/settings.json` (override with `--settings PATH`). Controls cover
TAA, VSync, uncapped/30/60/90/120/144/165/240 FPS, native/720p/900p/1080p/1440p
world-render resolution, 50/67/75/85/100% render scale, borderless fullscreen and
sound. F11 also toggles and persists fullscreen. Resolution presets fit within
the selected pixel dimensions while preserving the output aspect ratio; render
scale multiplies that size. The settings screen displays actual world/output
sizes. Output and UI remain at window/desktop resolution. Fullscreen uses the
current monitor's desktop mode, not exclusive display-mode switching. `--mute`
still disables audio initialization for diagnostics.

`src/temporal.rs` and `temporal.wgsl` implement world-only TAA. The world and sky
render to a linear RGBA16F target with a 16-phase Halton (2,3) camera jitter.
Ping-pong output-resolution RGBA16F histories store color plus depth. Depth-based
camera reprojection, translation-free sky reprojection, depth rejection,
3x3 neighborhood clipping and color-dependent history weighting limit trails.
History resets on resize, resolution/AA/upscaler changes, world replacement,
surface loss and camera jumps over four blocks. The held item uses a separate,
unjittered camera and full-resolution depth buffer after resolve; GUI follows at
full output resolution. With TAA off and no SDK upscaler, presentation uses linear
spatial resampling. Built-in TAA can also accumulate a scaled input at output
resolution; it is not labeled DLSS or FSR.

### Native DLSS and FSR 2

`cargo run` remains SDK-free. `tools/run-native.sh` runs the same game with the
`native-upscalers` Cargo feature, enabling actual NVIDIA NGX DLSS Super Resolution
(and DLAA at 100%) and AMD FidelityFX FSR **2**, not frame generation or FSR 4.
Use `tools/run-native.sh --release` for a release-optimized build; without that
leading option the launcher uses the development profile. Remaining arguments
are passed to the game, for example `tools/run-native.sh --release --new`.
Selecting SDK upscalers in a build without the feature reports why unavailable
and retains the working renderer. Unsupported GPUs/runtimes and invalid SDK
render ranges are similarly reported instead of silently claiming SDK rendering.
The upscaler quality row cycles Quality (1/1.5 output dimensions), Balanced
(1/1.7), Performance (1/2), Native AA (1/1; DLAA for DLSS), and Custom.
Named presets derive world resolution directly from the output, rounding each
axis to the nearest pixel. They select the corresponding DLSS SDK mode; FSR 2
uses the resulting input dimensions. The manual resolution/scale rows are
marked Custom while a named preset is active; editing either selects Custom
and uses both saved manual values. Enabling an upscaler starts at Quality.
Legacy settings without a quality field load as Custom to preserve their size.
The quality row is disabled while upscaling is off. Changes recreate SDK
contexts and reset temporal history, including when pixel sizes happen to match.

An SDK upscaler replaces the built-in TAA resolve; the AA row displays the
selected SDK and disables the separate TAA toggle. Supersampling above output size is
not supported by these native paths.

The launcher caches official DLSS **v310.4.0** and Vulkan-Headers **v1.4.309** in
`target/native-sdk/`, builds native SDK bindings (requires git, network on first
run, a C/C++ compiler and libclang), and exposes the official release NGX runtime
through an unversioned library symlink. Existing `DLSS_SDK` and `VULKAN_SDK`
environment variables override downloads. DLSS SDK/runtime redistribution is
subject to NVIDIA's own license; SDK files are not committed. The wrapper is
vendored from `dlss_wgpu` 2.0.0 (wgpu 27) under its original MIT/Apache licenses,
with one Linux safety patch: zero required-extension count produces an empty
slice instead of calling `from_raw_parts` on a null pointer. The SDK version is
pinned because newer NGX headers change structures the wrapper initializes.
FSR 2 uses the `fsr`/`fsr-sys` bindings and their bundled native Vulkan backend
and shader permutations.

`src/native_upscale.rs` owns SDK contexts, validates render ranges, chooses a
DLSS quality mode from the selected preset (or infers it from Custom scale), dispatches SDK work inside the wgpu
Vulkan encoder and waits for GPU completion before context replacement or
destruction. Explicit wgpu resource transitions keep external SDK layouts
consistent. A full-screen pass copies world device depth to R32F so SDK input
uses an ordinary shader-readable color layout; RG16F motion vectors derive from
camera reprojection with jitter cancellation and sky translation removed.
SDKs receive per-frame jitter, reset flags, input/output size, and (FSR) camera
near/far/FOV and frame time in milliseconds. Both produce linear RGBA16F output
before the full-resolution held item and UI. Motion vectors currently represent
camera motion; moving creatures/particles and transparent fluids do not yet have
per-object previous transforms/reactive masks, so temporal trails can remain in
those areas. Longer motion/quality/performance evaluation is still needed.

Validation: 110 quiet nextest tests pass, including settings round-trip/index
validation, aspect-preserving scaling, title-only actions, disabled hit regions,
paged save access and minimum-window settings bounds. Offscreen Vulkan captures
at 1280x800 completed for TAA/title and actual native FSR 2 and DLSS at 67% input
scale. Both SDK screenshots were inspected; these are short functional checks,
not a performance benchmark. A real Wayland window also completed an automatic
exit from the title at 2560x1440 borderless fullscreen, with VSync off, a 30 FPS
cap, 720p preset and 50% world scale; the preview save path remained absent. `--headless --title --frames 2 --capture PATH` previews
the title without advancing or saving the preview world. Combine `--settings`
with an isolated preferences file to exercise graphics settings without changing
normal preferences. Native debugging required an unrestricted driver run; a
sandboxed native attempt crashed, while unrestricted FSR and DLSS completed.

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

### Skylight CPU optimization

`World::sky_roof` computes height, tree, ruin and cave metadata once per roof
column, then reuses it while scanning downward. It preserves edit/resident-chunk
precedence and the previous scan bounds. Edits are searched by the BTreeMap's
x-coordinate range instead of scanning all edits for every roof. Plant support
resolution is unnecessary here because plants cannot block skylight. Roofs remain
cached only within one lighting operation, so edits cannot leave stale light.

Indirect rays stop once their remaining distance-based contribution cannot
exceed the brightest opening already found. Ray positions, wall checks and
brightness rules are otherwise unchanged. A regression compares optimized roof
queries and exposure against the original direct sampling and full ray traversal
across three seeds, negative coordinates, cave depths, roof openings, leaves and
removed high edits. All 111 quiet nextest tests, formatting and short Clippy pass.

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

### Shared occupancy column metadata

Skylight now stores `World::SkyColumn` values per horizontal coordinate for each
lighting operation. Each value holds height, tree, ruin and cave inputs plus the
roof result. Both roof scans and uncached ray occupancy cells use these inputs,
avoiding repeated procedural metadata generation at different heights in the
same column. Occupancy still checks authoritative edits first, resident chunks
second, and procedural terrain last. Plants and leaves retain their previous
light-transmission behavior. Caches are discarded after each operation, so this
adds no persistent invalidation requirements or shared worker synchronization.

The original roof/exposure equivalence test still passes. A new occupancy test
compares direct world queries at negative, boundary and distant coordinates
across three seeds, generated terrain, deliberately different resident chunks,
and stone/air/leaf/water/plant edits. All 112 quiet nextest tests, short Clippy,
formatting and diff checks pass.

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

### Profiling rerun (2026-11-09)

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

### Per-pulse fluid query cache

`World::tick_fluids` uses a fresh `world::BlockQueries` cache during its read
phase. Each queried cell is resolved once, including plant support; uncached
procedural cells share height/tree/ruin/cave inputs through `TerrainColumn` per
horizontal coordinate. Edits still override resident chunks, which override
procedural terrain. Skylight uses the same column-input constructor without
changing its roof/opacity rules. Fluid frontier ordering, the 512-cell budget,
lava scheduling and simultaneous commits are unchanged. The cache is dropped
before committing changes, so no persistent edit invalidation is required.
Its size follows cells queried by this pulse, rather than world travel history.

A regression compares fresh and repeated cached reads with `World::get` across
three seeds, negative/chunk-boundary/distant coordinates, cave and surface
heights, world bounds, deliberately different resident chunks, fluid/air/leaf/
plant edits and supported/unsupported plants. All 113 quiet nextest tests,
short Clippy, formatting and diff checks pass.

Fresh release measurements (48 repetitions, seed 314159, one excluded warmup)
are in `target/performance-rerun/fluid-{before,after}.{csv,txt}`. Twelve-pulse
median/p95 fell from 91.712/126.552 to 23.315/24.722 ms (75%/80% reduction).
Every workload's output units match between runs. Other workloads were slower
in the after run, consistent with machine-load variation; settled streaming
changed from 93.534/109.694 to 111.570/114.769 ms. This demonstrates faster
fluid stress updates, not a streaming or general frame-rate improvement.
