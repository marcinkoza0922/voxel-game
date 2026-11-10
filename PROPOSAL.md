# Voxel Procedurealy Generated Adventure Game

A game similar to Minecraft. Built in Rust, with a bespoke, fit-to-purpose ECS game engine

## Visual style

Lo-fi voxel blocks. However, I'd like relatively realistic lighting. Arguable, something raytraced if possible.

## Features
First person perspective
Procedural terrain generation
Cubic block voxels
RPG Systems -- enemies + certain activities give you experience that can be used to level up stats.
Lite-survival--you need to eat and sleep periodically.
Combat, albeit simplistic
Exploration--multiple different biomes 
Increasingly complex crafting, eventually automation.

### RPG Systems

Should not overpower the game. They should give you an edge compared to the early game, but not drastically so.

Two ways of gaining experience:
1) Killing enemies.Lower level enemies give fewer experience when you're high level, eventually not giving any experience at all
2) Crafting items--crafting something for the first time gives a big boost, each subsequent craft is 1/4 or so of that original amount, rounded down (so it should reach 0 pretty quickly). This can either be a simple formula, or maybe each item can specify how much xp the first n crafts give 
3) Entering a new biome for the first time
4) Acquiring a new resource for the first time
5) Quests or other progression activities, not sure yet.

Stats are
Vigor (health)
Strength (STR based weapons)
Dexterity (DEX based weapons--weapons can be DEX/STR mixtures)
Wisdom (for magic strength)
Willpower (amount of mana)
Agility (movement speed, jumping, how much fall damage you take)
Ingenuity (boosts crafting efficiency and quality)

### Biomes
Should be a mix of normal, exotic, fantastic, and scary
- Grasslands (plains, hills, mountains)
- Tundra (plains, hills, mountains)
- Forests (with occasional groves)
- Mushroom forests
- Enchanted forests (should be very, very rare). Purple trees and magic springs and what not.
- Volcanic lands / ashlands
- Deserts and dunes
- Marshes
- Swamps

You should be able to find temples, ruins, labyrinths, and mysterious structures from time to time.

Also underground biomes
- Crystal caves
- Underground forests with special trees and plants
- Ice caverns
- Ancient ruins of a lost civilization

Also underwater biomes
- Kelp forests
- Coral reefs
- Sunken ships and ruins

Also, harder to reach---floating islands. These shouldn't spawn in areas by the game's starting area, though. You need to venture out quite a bit before you can encounter them

Should have many different kinds of stone (granite, limestone, sandstone, etc.) and trees (oak, birch, fir, etc.). Different stone and trees should have slightly different material properties, but for the most part they are identical in purpose--the differences are largely there for flavor. 
Breaking stone should give you cobblestone for that type of stone. Trees give you logs, which can be crafted into associated planks. Obsidian should be a stone as well--unlike Minecraft, here it'd just be a more exotic, harder to find type of stone whose main appeal is aesthetic. 

### Metals

Several types of metal, which ought to correspond to different stages in game progression.

- Copper
- Iron
- Tin
- Gold
- Tungsten (purple tint to distinguish from iron)
- Titanium
- Bronze (made from copper + tin)
- Steel (made from iron + coal)

Copper should be the most accesible and should be smelted in the first crude furnace you can build.
Later on, you should be able to build a smelter, and there you can forge your first iron tools.
A more advanced smelter should allow you to create alloys--you can combine tin and copper to create Bronze
Gold is special--it should be the worst metal with the weakest stats and lowest durability. However, it should be the best conduit for magic gems.
Tungsten requires an advanced furnace. Tungsten smelters need to be built in special locations underground.
Titanium requires electricity to smelt.

Metals should be findable in any biome, though sometimes the rate at which they are present is different. 

Coal is also found throughout caves and mines

### Gems

Occasionally, you will find gemstones:

Diamonds
Rubies
Emeralds
Sapphires
Topaz
Opal
Amethysts
Pearls, including Black Pearls

Gems are rare, and unlike metals they do not spawn in every biome. They are more common in "hostile" biomes than friendly ones, and more common deep underground, as well as on floating islands.

Gems can be inserted into equipment. They cannot be removed. Once inserted, they give great bonuses.

Diamonds be inserted in everything, and they greatly increase durability. 
Rubies can only be inserted in weapons and tools, and they increase damage.
Sapphires can be inserted in armor to increase protection.
Emeralds can be inserted into tools and weapons to increase their reach.
Topaz can be placed in tools to increase their yields (by increase by some n% that harvesting a resource will produce double resources)
Opal can be placed in boots to increase jump height and reduce fall damage.
Amethysts can be placed in staves and wands to increase the effectiveness of spells.
Pearls can be placed in helmets to give waterbreathing capability.
Black pearls are special--they can show the "hidden" things of this world, and reveal things that might normally be unaccessible and invisible to the player, things that were there all along... 

Gems might also be used in special crafting recipes

### Tools

You start with only an axe and pickaxe.
You can craft:
- Axes (needed to chop trees)
- Pickaxe (For stone)
- Hoes (for tilling the soil)
- Shovel (efficient for dirt, sand, and gravel)
- Scythe (increased yields when harvesting crops)
- Fishing rod
- Flint and steel (for starting fires)
- Shears

### Dyes

Made with flowers and assorted objects found throughout the environment. Dyes can be combined to create other dyes. Sheep can be dyed to efficiently create dyed wools.

### Weapons

- Swords--the standard
- Greatswords--require STR. Slower to swing with low STR, but do great damage.
- Daggers. Require DEX. Lower reach, but have fast attack speed
- Bow--gets better with DEX.
- Crossbow--requires an advanced crafting table to build, but it's 

Axes and pickaxes do better-than-nothing damage in a pinch.

### Mobs

#### Non-hostile

Unlike minecraft, animal mobs do have sexes. When looking at a mob, the player should see the name of the mob and its sex. You need at least two members of each species, male and female, to start husbandry. 

Cattle (can be milked, killing provides beef and leather)
Pigs (provides lots of pork and leather)
Chickens (drop eggs and feathers, killing provides chicken and feathers)
Sheep (can be sheared for wool. Wool can be dyed, like in minecraft, and children inherit the dyed wool. Killing provides wool and mutton)

Also:
Waterbuffalo--similar to cattle, but only found in more tropical biomes.
Goats (Similar to sheep. Found only in mountainous biomes. Killing provides chevon and wool.)
Turkeys (similar to chickens)

#### Semi hostile

Only attack when attacked

Wolves. Can be tamed and turned into dogs. Hunt other animals.
Wildcat. Can be tamed and turn into cats. Hunt other animals
Bears--cannot be tamed, but usually leave you alone. 

#### Hostile
Zombies
Skeletons
Goblins--often found in groups with camps.
Liches--only in far away biomes
Werewolves--far away biomes
Minotaurs--only found in labyrinths

### Crafting

Similar to minecraft

Player can craft with a 2x2 grid.
A crafting table gives you 3x3 grid.
A later advanced crafting table gives you a 6x6 grid, enabling some complex recipes.
Each table can craft the same things as all previous tables

Special crafting stations
- Furnace--for cooking and basic copper working.
- Crude Smelter--for basic metallurgy. 
- Smelter--Requires a bellows adjacent. Allows for creation of alloys like bronze and steel. STR + INGenuity boost production efficiency here.
- Gem cutting table--for creating more refined versions of gems. DEX + ING boost production efficiency.
- Fletching table--for efficient arrow making 
- Cauldron -- for brewing potions

### Fluids and buckets

Water and lava use Minecraft-style source blocks and discrete spreading flow.
Water spreads up to seven blocks horizontally, falls downward, drains when its
supply is removed, and regenerates sources when supported between at least two
horizontal water sources. Lava spreads more slowly and less far, cannot renew,
and solidifies on contact with water. Include natural pools, swimming, drowning
and lava damage.

Buckets can be crafted from wood or any available metal. All carry one water
source; carrying lava requires iron or a stronger metal. Wood, copper and iron
are the currently available bucket materials.

### Energy and Automation

You can burn coal and other fuels to generate power.

- Coal / charcoal
- Wood
- Plant matter

 These suffice for basic plants.

Eventually, you can unlock coal liquification and biodiesel makers to turn oil and biodiesel respectively. I eventually want some fun and challenging ways to create power.

No "passive" energy generation. Stuff like set-it-and-forget it solar should not be allowed. Every power generation source require cleverness to fully automate.

Power can be hooked up with copper wires.

You can find pools of oil underground as well. There are also canola plants that can be pressed into oil. Both of these can be used as fuel.

### Automation

Eventually, you'd be able to have special powered apparati for automating construction of goods. These devices would be connected by tubes.

### Logistics network

Eventually, you should be able to connect storage and powered apparati with tubes to create a logistics network. There should be terminals to withdraw and deposit items into the network.

### Logic

Wires can be craft from copper. These wires can be connected to devices and storage, and also to logic gates, sensors, and switches.

You can combine copper wire with wool to create insulated wire. Dyed wool creates separate wires. Different colored wires can sit on the same block without interfering with each other.

### Teleporters

Much, much later on, you should be able to create teleporters to be able to move around the world quickly.

### Lighting

Basic torches made with sticks + coal last for a good bit but eventually extinguish.

Lanterns made of metal (any metal) can be filled with 

### Magic

You can craft staves and gloves that allow for casting magic. These are powered by gemstones.

Possible spells

- Fireball / Icicle / Lightning bolt
- Fling (throws your body way forward. Useful for travel. Does not protect against fall damage)
- Levitate
- Invisibility
- Fall damage immunity (use together with fling!)
- Fire / ice resistance
- Damage resistance
- Damage increase
- Berserk (use on mobs to make them attack whatever is in sight)
- Charm
- Control (take over a mob!)

### Temperature

A lightweight temperature system. Being in hot or cold biomes without having proper gear, potions, magic, or tools can be lethal.

