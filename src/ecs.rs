use glam::Vec3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entity {
    index: usize,
    generation: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MobKind {
    Husk,
    Sheep { male: bool },
    Penguin { male: bool },
    PolarBear { male: bool, provoked: bool },
    Guardian([i32; 3]),
}
impl MobKind {
    pub fn hostile(self) -> bool {
        matches!(
            self,
            Self::Husk | Self::Guardian(_) | Self::PolarBear { provoked: true, .. }
        )
    }
    pub fn body(self) -> (f32, f32) {
        match self {
            Self::Penguin { .. } => (0.32, 0.90),
            Self::PolarBear { .. } => (0.57, 1.37),
            _ => (0.29, 1.79),
        }
    }
    pub fn target_height(self) -> f32 {
        match self {
            Self::Penguin { .. } => 0.48,
            Self::PolarBear { .. } => 0.83,
            _ => 1.,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Husk => "Husk",
            Self::Sheep { male: true } => "Sheep (male)",
            Self::Sheep { male: false } => "Sheep (female)",
            Self::Penguin { male: true } => "Penguin (male)",
            Self::Penguin { male: false } => "Penguin (female)",
            Self::PolarBear { male: true, .. } => "Polar bear (male)",
            Self::PolarBear { male: false, .. } => "Polar bear (female)",
            Self::Guardian(_) => "Ruin guardian",
        }
    }
}
pub struct Mob {
    pub kind: MobKind,
    pub pos: Vec3,
    pub health: f32,
    pub cooldown: f32,
    pub level: u32,
    pub heading: f32,
    pub stride: f32,
}
struct Slot {
    generation: u32,
    mob: Option<Mob>,
}
#[derive(Default)]
pub struct Entities {
    slots: Vec<Slot>,
    free: Vec<usize>,
}
impl Entities {
    pub fn spawn(&mut self, mob: Mob) -> Entity {
        let index = if let Some(i) = self.free.pop() {
            self.slots[i].mob = Some(mob);
            i
        } else {
            self.slots.push(Slot {
                generation: 0,
                mob: Some(mob),
            });
            self.slots.len() - 1
        };
        Entity {
            index,
            generation: self.slots[index].generation,
        }
    }
    pub fn get_mut(&mut self, id: Entity) -> Option<&mut Mob> {
        self.slots
            .get_mut(id.index)
            .filter(|s| s.generation == id.generation)
            .and_then(|s| s.mob.as_mut())
    }
    pub fn despawn(&mut self, id: Entity) {
        if let Some(s) = self.slots.get_mut(id.index)
            && s.generation == id.generation
            && s.mob.take().is_some()
        {
            s.generation = s.generation.wrapping_add(1);
            self.free.push(id.index);
        }
    }
    pub fn iter(&self) -> impl Iterator<Item = (Entity, &Mob)> {
        self.slots.iter().enumerate().filter_map(|(i, s)| {
            s.mob.as_ref().map(|m| {
                (
                    Entity {
                        index: i,
                        generation: s.generation,
                    },
                    m,
                )
            })
        })
    }
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Mob> {
        self.slots.iter_mut().filter_map(|s| s.mob.as_mut())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stale_entity_cannot_access_reused_slot() {
        let mut e = Entities::default();
        let a = e.spawn(Mob {
            kind: MobKind::Husk,
            pos: Vec3::ZERO,
            health: 10.,
            cooldown: 0.,
            level: 1,
            heading: 0.,
            stride: 0.,
        });
        e.despawn(a);
        let b = e.spawn(Mob {
            kind: MobKind::Husk,
            pos: Vec3::ONE,
            health: 10.,
            cooldown: 0.,
            level: 1,
            heading: 0.,
            stride: 0.,
        });
        assert_ne!(a, b);
        assert!(e.get_mut(a).is_none());
        assert!(e.get_mut(b).is_some());
    }
}
