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

Advanced metallurgy, magic/gems, breeding, quests, fluid simulation, logistics, electricity, automation, extra biomes and multiplayer are future work. Existing station/recipe/item registries and separate simulation should accommodate them. The slice has finite render distance but not a finite horizontal map. Terrain generation and CPU meshing run on bounded background workers using immutable jobs. Stream planning and GPU uploads remain on the main thread. Save format versions reject unsupported data rather than silently discarding it.

## Validation

Use quiet nextest tests for determinism, negative coordinate chunk mapping, height limits, cross-chunk edits, voxel ray casting/collision, generation-safe entity reuse, recipe transactions, XP decay, save round trips and malformed saves. Run clippy with short diagnostics. A headless deterministic simulation smoke command exercises gameplay and persistence without a display. GPU validation requires a real window or supported offscreen adapter; record actual checks and limitations here. Helpers belong in `tools/`.

## Implemented decisions and limits

Presentation uses strict FIFO VSync, supported by every wgpu surface. Unlike `AutoVsync`, this does not select relaxed FIFO with immediate presentation on missed refreshes. There is no software FPS cap: redraws are requested continuously while the event loop waits for events, and FIFO presentation provides GPU backpressure. Immediately before presenting, `Window::pre_present_notify` enables compositor frame callbacks on Wayland so redraws follow the display refresh rate. Simulation remains independently fixed at 60 Hz. Actual pacing depends on the display driver/compositor; VSync does not remove terrain generation, meshing or rendering costs.

The slice uses `winit 0.30`, `wgpu 27`, `glam` for primitive vector/matrix math, `bytemuck` for GPU byte layouts, `pollster` for startup, and `serde`/`serde_json`. Cargo.lock is checked in for reproducibility. GPU initialization explicitly selects Vulkan on Linux. The native graphics check exposed an NVIDIA EGL/Wayland teardown crash when probing all backends; Vulkan selection and releasing renderer/window resources before ending the event loop produce clean exits. GPU work is drained before destruction.

The initial ECS is a small, single-archetype component store: generational entity IDs index reusable slots containing position, health, level and AI cooldown. Creature behavior and physics run in explicit simulation systems. This is deliberately smaller than a general archetype/query framework. More component stores and scheduling can be added when multiple entity classes require them. Creatures are transient and respawn after loading; player state, discoveries, recipe counts, tool wear, torches, time and block edits persist.

Terrain chunks are 16x16x16. Stream radius is ten chunks horizontally (160 blocks along each axis). Each horizontal chunk column samples all 256 terrain heights and streams from one chunk below its lowest surface through its highest surface plus tree canopy, without clamping mountains to player elevation. A five-by-five horizontal area additionally streams two chunks above/below the player for cave exploration. Streaming replans on player chunk or radius changes and evicts chunks outside the desired set. Jobs are ordered by distance with deterministic ties; obsolete mesh jobs are discarded and remaining mesh jobs reprioritized near the player. Two persistent CPU workers generate chunk blocks and meshes together from a shared one-block halo. At most two jobs, including completed results, are outstanding per world; thus a redraw accepts/uploads at most two meshes. Workers receive immutable seed/local-edit snapshots and never access authoritative state or GPU resources. The main thread polls without waiting, accepts only still-desired results matching the current edit revision, and retries invalidated jobs. Revision checks are conservatively world-wide: unrelated edits can also trigger retries. Evicted results are discarded. Pending resident remeshes survive stream replanning; boundary edits invalidate diagonal neighbors for AO. Worker channels close and threads join on world destruction, bounded by outstanding jobs. Stream planning, priority sorting, edit snapshot copying and GPU uploads remain synchronous. Sampling is deterministic whether or not a chunk is resident. Surface heights are roughly 24–800, with mountain growth suppressed near spawn. The full legal range is -512..1535. Mountain surfaces within the view radius can render at any generated elevation. Horizontal fog fades between 112 and 160 blocks; the camera far plane is 1,024 blocks to include tall nearby peaks. Conservative chunk AABB tests independently cull camera and sun-shadow draws. CPU meshing uses a one-block halo with shared column height/tree data (324 column evaluations instead of up to 5,832 for an unloaded halo), shared-face culling and diagonal-aware corner AO. LOD and greedy meshing remain future performance work; this iteration retains exact voxel geometry and existing terrain generation/save compatibility. Rendering uses a 2048-square sun depth map with 3x3 comparison filtering, a day/night intensity cycle, fog, and the nearest sixteen warm torch lights. Held torches occupy one light slot. Placed torches expire after 600 simulation seconds, and placement is capped at 256 active torches.

The owned GUI is an immediate-mode triangle builder with rectangle buttons, pointer hit regions, a built-in 5x7 font and resolution-aware scaling. The pack has a 27-slot backpack, nine real hotbar slots, shaped crafting, a recipe pattern guide, a furnace view and attribute allocation. Menus and lost focus pause simulation. Recipes are instant transactional operations; furnace recipes consume ore and coal directly rather than having processing queues. Tables and furnaces must be within three blocks. A wood bedroll recipe substitutes for the later wool economy. Leaves supply berries as a renewable slice food source. Husks provide combat and meat; animals and audio are deferred. All visuals are code-native voxel geometry and glyphs, so no asset authoring tools or generated raster assets were used.

Vigor adds 3 health per point; Strength adds 2.5% melee damage; Dexterity reduces the attack interval through a 1.5%-per-point speed multiplier; Agility modestly improves walking/jumping and reduces falls. Ingenuity grants one bonus output every `24 - points` crafts (only when allocated). All stats cap at 20. Wisdom and Willpower remain visible but cannot consume points before magic is implemented. First recipe XP is quartered per repeat to zero. First biome/resource discoveries grant XP once. Kill XP falls with player level and reaches zero for sufficiently weak enemies. Death restores the player at spawn and retains inventory. Temperature and magic are deferred.

Saves use a synced temporary file followed by atomic rename. Unsupported versions, malformed JSON, invalid positions, inventory selection/count/wear, stat bounds and torch timer inconsistencies are rejected. Load failures preserve existing files. A failed save keeps the quit action from closing the window. Save format v2 requires the current recipe count. Version 1 aggregate inventories migrate to real stacks on load; changed recipe registries will still require explicit migration. Concurrent writers to the same save are not supported.

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
plus coal previews an ingot or cooked meat. Processing stays instant and consumes
one input and one coal when output is taken; timed queues and persistent
per-world-block furnace inventories remain future work. These input slots belong
to the player, shared across furnace screens, rather than each placed furnace.

Save v2 validates slot count, stack sizes and individual wear, including grid,
furnace and carried stacks. V1 counts split into stacks, old hotbar preferences
are mapped onto real items, and old wear is attached to the first matching tool.
If the old inventory exceeds 36 slots, migration returns an explicit error and
preserves the source file. Mining refuses to remove a resource when its drop
cannot fit. Craft transactions refuse full output inventories without consumption.

Validation: formatting, short Clippy with warnings denied, quiet nextest, smoke,
and the expedition. The expedition now arranges actual crafting/furnace inputs
and takes outputs through the same transaction as the UI. Regression coverage
includes every recipe pattern/yield, offsets/mirroring/extra ingredients,
stack splitting/merging/swapping, independent tool wear, full-output atomicity,
furnace fuel validation, legacy migration and overflow preservation, held/input
save roundtrips, invalid stacks, and scaled UI hit regions. A bounded native
Vulkan pack capture rendered and exited cleanly; its layout was inspected.
Live mouse ergonomics and prolonged play remain manual validation.
