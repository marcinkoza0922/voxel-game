use glam::Vec3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entity {
    index: usize,
    generation: u32,
}
pub struct Mob {
    pub pos: Vec3,
    pub health: f32,
    pub cooldown: f32,
    pub level: u32,
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
            pos: Vec3::ZERO,
            health: 10.,
            cooldown: 0.,
            level: 1,
        });
        e.despawn(a);
        let b = e.spawn(Mob {
            pos: Vec3::ONE,
            health: 10.,
            cooldown: 0.,
            level: 1,
        });
        assert_ne!(a, b);
        assert!(e.get_mut(a).is_none());
        assert!(e.get_mut(b).is_some());
    }
}
