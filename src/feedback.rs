//! Transient presentation state, deliberately excluded from saves.
use glam::Vec3;
#[derive(Clone, Copy, Debug)]
pub enum Cue {
    Stone,
    Wood,
    Earth,
    Break,
    Hit,
    Hurt,
    Swing,
    Pickup,
}
pub struct Particle {
    pub pos: Vec3,
    velocity: Vec3,
    pub color: [f32; 3],
    pub life: f32,
}
#[derive(Default)]
pub struct Feedback {
    pub particles: Vec<Particle>,
    pub sounds: Vec<Cue>,
    pub swing: f32,
    pub hit: f32,
    pub hurt: f32,
    pub clock: f32,
}
impl Feedback {
    pub fn tick(&mut self, dt: f32) {
        self.clock += dt;
        self.swing = (self.swing - dt).max(0.);
        self.hit = (self.hit - dt).max(0.);
        self.hurt = (self.hurt - dt).max(0.);
        for p in &mut self.particles {
            p.life -= dt;
            p.velocity.y -= dt * 9.;
            p.pos += p.velocity * dt;
        }
        self.particles.retain(|p| p.life > 0.);
    }
    pub fn sound(&mut self, cue: Cue) {
        if self.sounds.len() < 8 {
            self.sounds.push(cue);
        }
    }
    pub fn burst(&mut self, pos: Vec3, color: [f32; 3], count: usize) {
        for i in 0..count.min(192usize.saturating_sub(self.particles.len())) {
            let angle = i as f32 * 2.39996 + self.clock;
            self.particles.push(Particle {
                pos,
                color,
                life: 0.3 + (i % 4) as f32 * 0.07,
                velocity: Vec3::new(
                    angle.cos() * 1.6,
                    1. + (i % 3) as f32 * 0.7,
                    angle.sin() * 1.6,
                ),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn effects_are_bounded_and_expire() {
        let mut feedback = Feedback::default();
        for _ in 0..100 {
            feedback.burst(Vec3::ZERO, [1.; 3], 20);
            feedback.sound(Cue::Stone);
        }
        assert_eq!(feedback.particles.len(), 192);
        assert_eq!(feedback.sounds.len(), 8);
        feedback.swing = 0.32;
        feedback.hit = 0.18;
        feedback.hurt = 0.3;
        feedback.tick(1.);
        assert!(feedback.particles.is_empty());
        assert_eq!((feedback.swing, feedback.hit, feedback.hurt), (0., 0., 0.));
    }
}
