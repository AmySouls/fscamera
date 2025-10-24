use crate::game::CSFlipperImp;

pub struct GameSpeed {
    enabled: bool,
    multiplier: f32,
}

impl Default for GameSpeed {
    fn default() -> Self {
        Self {
            enabled: false,
            multiplier: 1.0,
        }
    }
}

impl GameSpeed {
    pub fn set_multiplier(&mut self, multiplier: f32) {
        self.multiplier = multiplier;
    }

    pub fn set_multiplier_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn apply(&self, flipper: &mut CSFlipperImp) {
        if self.enabled {
            flipper.time_multiplier = self.multiplier;
        } else if self.enabled && flipper.time_multiplier != 1.0 {
            flipper.time_multiplier = 1.0;
        }
    }
}
