# Voxel Adventure

A Linux voxel adventure built in Rust with an owned engine and GUI. This first playable slice follows [PROPOSAL.md](PROPOSAL.md); architecture, decisions, and current limits are in [docs/DESIGN.md](docs/DESIGN.md).

Requires a Rust toolchain supporting edition 2024, a Vulkan-capable GPU/driver (including software Vulkan), and a Linux desktop (Wayland or X11) for interactive play. No downloaded game assets are required.

```sh
cargo run --release
```

The first release build takes a while. `cargo run` also works for development. By default the game loads `saves/world.json`, or creates a world with seed `314159`. Use `--save saves/another.json --seed 42` for a new world; an existing save supplies its own seed. Save files are versioned JSON and are excluded from Git. Failed or incompatible loads stop with an error and preserve the original file.

To start fresh, press **Esc → New world (saves current)**. This saves your current
world and switches to a fresh seed and separate `world-N.json` file in the same
save directory. The window title shows the active save path. Resume that world
later with `cargo run --release -- --save saves/world-N.json`.

You can also launch with `cargo run --release -- --new`. Add `--seed 42` for a
specific seed; `--save PATH` selects the directory beside PATH for the new file.
Existing files are preserved.

| Control | Action |
| --- | --- |
| WASD / mouse | Walk / look |
| Space / Left Shift | Jump / sprint |
| Hold left mouse | Mine or attack |
| Right mouse | Place a block, scoop/pour a bucket, eat food, or use a bed |
| 1–9 / mouse wheel | Select hotbar slot |
| E | Open/close pack, recipes, and attributes |
| Left-click / drag a slot | Pick up, move, merge, or swap stacks |
| Shift-left-click a slot / output | Move directly into crafting/furnace slots or back to inventory |
| Right-click a slot | Pick up half a stack / place one item |
| Click / drag crafting output | Take the result and consume arranged ingredients |
| Right-click a table or furnace | Open its crafting or processing screen |
| R | Sleep near a bedroll, away from enemies |
| F5 | Save |
| Esc | Pause menu; save and quit |

Losing window focus pauses the game. The pack pauses movement and survival while furnaces continue processing. Autosave runs every 30 seconds of gameplay; closing the window saves too.

Water and lava form small natural pools. At a crafting table, arrange three
planks, copper ingots or iron ingots in a V to make a bucket. Right-click a
source to collect it and right-click a surface to pour it. All three buckets
carry water; lava requires iron. Flowing fluid cannot be collected. A supported
2×2 water pool refills after scooping a corner. Hold Space to swim up, watch your
air underwater, and avoid lava.

## First expedition

You start with a wooden axe, a wooden pickaxe, and berries. Chop logs with the axe. Open the pack and turn logs into planks, then craft a table. Drag it into your hotbar and place it against a nearby block. Right-click the table to open its 3x3 grid. Stations work within three blocks.

The pack has 27 backpack slots, nine hotbar slots, and a 2x2 personal crafting grid. Arrange ingredients to match the recipe guide; taking the output consumes them. Materials stack to 64, and tools have individual durability. Crafting grids and carried items are saved with the player. Each placed furnace stores its own input, fuel, output, heat, and processing progress.

Mine through the dirt to granite with the pickaxe. Make sticks and stone tools at the table. Stone pickaxes can harvest copper ore. Craft a furnace from eight cobblestone at the table, place it, and use coal plus copper ore to smelt ingots. Make copper tools and a sword at the table. In the furnace tab, put ore or raw meat in INPUT and coal in FUEL, then take the output. Each item takes five seconds to smelt; one coal fuels four items. Furnaces continue working while you explore, including outside loaded terrain, and resume from saved progress after loading. Empty all item slots before mining a furnace; breaking it discards any remaining heat.

After copper, mine iron ore with a copper pickaxe. Iron occurs deeper than
copper, beneath at least twelve blocks of surface cover. At a crafting table,
make a **crude smelter** from six cobblestone and two copper ingots (see the recipe
guide). Place and right-click it like a furnace. It smelts iron as well as copper
and food; ordinary furnaces cannot smelt iron. Craft iron axes, pickaxes and
swords with the same patterns as copper tools. Iron tools last longer and work
faster, and the sword deals more damage.

Explore beyond the starting region for **tundra**: snowy ground, open hills and
mountains with scattered ancient ruins. Bring food, torches, a bedroll and a
good weapon. Blue-gray ruin guardians are tougher and faster than husks. Defeat
a ruin's guardian to unseal its blue cache, then mine the cache with a pickaxe
for six iron ingots. Discovery and guardian victory award XP. Defeated guardians
and collected caches stay completed after saving and reloading.

Existing saves migrate automatically and retain your inventory and tools. Old shared furnace contents move to the placed furnace nearest your saved position. If none remains, those contents are preserved for the next furnace you place.

Leaves yield berries; eat them with right mouse while selected. Husks pursue and attack, and drop meat that can be cooked in the furnace. Bedrolls restore rest and advance to dawn; nearby enemies prevent sleep. Torches provide warm light while held or placed; placed torches burn out after ten minutes of active gameplay. Death returns you to spawn with your inventory.

Discovering biomes and resources, crafting, and defeating sufficiently strong enemies award XP. Repeated crafting awards one quarter as much XP each time, rounding down to zero. Spend level points on health, melee damage, attack speed, mobility, or crafting yield. Wisdom and Willpower are reserved for the future magic system and cannot consume points yet.

Held axes, pickaxes and swords now swing in first person, with movement bob,
material-colored mining chips, block-break bursts, melee hit markers and a damage
flash. Sound effects use optional `paplay` (PulseAudio/PipeWire); install your
distribution's PulseAudio utilities for playback, or use `--mute` to disable it.

The terrain streams in chunks and supports a vertical range from **-512 to 1535**. Terrain streams up to 160 blocks horizontally, with full-height mountain surfaces, refresh-synchronized presentation, and camera/shadow chunk culling. Venture far from spawn for large mountains. Grasslands, oak forests, tundra, caves, coal, copper, iron and guarded ruins are implemented; the broader biomes, metallurgy, magic, and automation remain future milestones.

## Development

Install `cargo-nextest` if needed, then:

```sh
cargo nextest run
cargo clippy --message-format=short
./tools/verify.sh
```

Cargo output and nextest are configured to be quiet. The verification helper prints diagnostics only on failure. A display-free gameplay/persistence check is available with `cargo run -- --smoke` and uses a temporary save, leaving your world untouched.

For reproducible GPU checks, `--frames 180 --capture /tmp/frame.ppm --save /tmp/test-world.json` renders a fixed number of frames, captures the final framebuffer, saves, and exits. Add `--pack` to inspect the GUI. `--capture` defaults to 240 frames if no frame count is supplied. PPM is intentionally dependency-free and readable by standard image viewers/converters.

For frame-time diagnostics, add `--profile /tmp/frames.csv`, then run
`python3 tools/analyze-profile.py /tmp/frames.csv`. The report gives median,
95th percentile, worst frame, and peak queued/active/resident chunks. These are
wall times, including presentation waits, rather than GPU execution timings.
The first frame has no interval; `--skip 180` can exclude startup.

`cargo run -- --expedition` runs a controlled headless expedition through copper and iron
gear, combat, food, sleep, forest/cave checks, a natural tundra ruin and guardian,
cache rewards, and reload. It mines fixture
resources through actual gameplay actions; forest/cave visits use relocations.
It does not measure how easily a player can find resources in a natural world.

A repeatable windowed streaming tour crosses adjacent chunks and relocates to
distant terrain every 180 frames, rotating the camera throughout:

```sh
cargo run --release -- --tour --frames 1800 --profile /tmp/tour.csv --save /tmp/new-tour-world.json
python3 tools/analyze-profile.py /tmp/tour.csv
```

The tour requires an explicit save path that does not already exist. Use a fresh
path for each run. See [playtest results](docs/PLAYTEST.md) for measured results
and remaining manual checks.

`cargo run -- --playtest` runs the deterministic headless mechanics course and
progression expedition. Both also run automatically under `cargo nextest run`.
They check game rules using controlled fixtures without touching your save;
see [the playtest guide](docs/PLAYTEST.md) for coverage and limits.

## Headless rendering

Run the actual GPU renderer without a window, display server, or audio:

```sh
cargo run --release -- --headless --frames 120 --capture /tmp/frame.ppm --save /tmp/render-world.json
```

This renders at 1280×800, advances simulation by one fixed 1/60-second step per
frame with no movement input, writes the final image as PPM, saves the world,
and exits. `--headless` defaults to 240 frames; `--frames` must be positive.
Use a separate `--save` path for diagnostics: an existing save is loaded and
updated, just as in interactive play. Add `--seed 42` for a reproducible new
world or `--new --seed 42` to create a separate save beside the selected path.
The capture's parent directory must already exist.

Add `--pack` to capture the inventory UI (only furnaces advance while open),
`--profile /tmp/frames.csv` for timings, or `--tour` for scripted camera
relocations. Tours require `--frames` and an explicit save path that does not
exist. Headless runs wait for terrain streaming at startup, after tour
relocations, and before the final frame, with a 120-second timeout per wait.
Those waits appear in profile streaming times; these runs do not measure display
pacing. Simulation uses fixed steps, but asynchronous streaming and hardware
can affect intermediate images and timings.

Vulkan is still required. A machine without a GPU can use an installed software
Vulkan implementation such as Mesa lavapipe; no Xvfb is needed. Adapter/device
errors are reported with a nonzero exit status. For simulation checks requiring
no graphics driver, use `cargo run -- --smoke` instead.
