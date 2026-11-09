# Voxel Adventure

A Linux voxel adventure built in Rust with an owned engine and GUI. This first playable slice follows [PROPOSAL.md](PROPOSAL.md); architecture, decisions, and current limits are in [docs/DESIGN.md](docs/DESIGN.md).

Requires a Rust toolchain supporting edition 2024, a Linux desktop (Wayland or X11), and a Vulkan-capable GPU/driver. No downloaded game assets are required.

```sh
cargo run --release
```

The first release build takes a while. `cargo run` also works for development. By default the game loads `saves/world.json`, or creates a world with seed `314159`. Use `--save saves/another.json --seed 42` for a new world; an existing save supplies its own seed. Save files are versioned JSON and are excluded from Git. Failed or incompatible loads stop with an error and preserve the original file.

| Control | Action |
| --- | --- |
| WASD / mouse | Walk / look |
| Space / Left Shift | Jump / sprint |
| Hold left mouse | Mine or attack |
| Right mouse | Place selected block, eat selected food, or use bed |
| 1–9 / mouse wheel | Select hotbar slot |
| E | Open/close pack, recipes, and attributes |
| Left-click / drag a slot | Pick up, move, merge, or swap stacks |
| Right-click a slot | Pick up half a stack / place one item |
| Click / drag crafting output | Take the result and consume arranged ingredients |
| Right-click a table or furnace | Open its crafting or processing screen |
| R | Sleep near a bedroll, away from enemies |
| F5 | Save |
| Esc | Pause menu; save and quit |

Losing window focus pauses the game. The pack pauses movement and survival while furnaces continue processing. Autosave runs every 30 seconds of gameplay; closing the window saves too.

## First expedition

You start with a wooden axe, a wooden pickaxe, and berries. Chop logs with the axe. Open the pack and turn logs into planks, then craft a table. Drag it into your hotbar and place it against a nearby block. Right-click the table to open its 3x3 grid. Stations work within three blocks.

The pack has 27 backpack slots, nine hotbar slots, and a 2x2 personal crafting grid. Arrange ingredients to match the recipe guide; taking the output consumes them. Materials stack to 64, and tools have individual durability. Crafting grids and carried items are saved with the player. Each placed furnace stores its own input, fuel, output, heat, and processing progress.

Mine through the dirt to granite with the pickaxe. Make sticks and stone tools at the table. Stone pickaxes can harvest copper ore. Craft a furnace from eight cobblestone at the table, place it, and use coal plus copper ore to smelt ingots. Make copper tools and a sword at the table. In the furnace tab, put ore or raw meat in INPUT and coal in FUEL, then take the output. Each item takes five seconds to smelt; one coal fuels four items. Furnaces continue working while you explore, including outside loaded terrain, and resume from saved progress after loading. Empty all item slots before mining a furnace; breaking it discards any remaining heat.

Existing saves migrate automatically and retain your inventory and tools. Old shared furnace contents move to the placed furnace nearest your saved position. If none remains, those contents are preserved for the next furnace you place.

Leaves yield berries; eat them with right mouse while selected. Husks pursue and attack, and drop meat that can be cooked in the furnace. Bedrolls restore rest and advance to dawn; nearby enemies prevent sleep. Torches provide warm light while held or placed; placed torches burn out after ten minutes of active gameplay. Death returns you to spawn with your inventory.

Discovering biomes and resources, crafting, and defeating sufficiently strong enemies award XP. Repeated crafting awards one quarter as much XP each time, rounding down to zero. Spend level points on health, melee damage, attack speed, mobility, or crafting yield. Wisdom and Willpower are reserved for the future magic system and cannot consume points yet.

The terrain streams in chunks and supports a vertical range from **-512 to 1535**. Terrain streams up to 160 blocks horizontally, with full-height mountain surfaces, refresh-synchronized presentation, and camera/shadow chunk culling. Venture far from spawn for large mountains. Grasslands, oak forests, caves, coal and copper are implemented; the broader biomes, metallurgy, magic, and automation remain future milestones.

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

`cargo run -- --expedition` runs a controlled headless expedition through copper
gear, combat, food, sleep, forest/cave checks, and reload. It mines fixture
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
