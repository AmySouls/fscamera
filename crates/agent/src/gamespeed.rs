use crate::game_compat::CSFlipperImp;

pub struct GameSpeed {
    enabled: bool,
    playback_multiplier_enabled: bool,
    multiplier: f32,
    playback_multiplier: f32,
}

impl Default for GameSpeed {
    fn default() -> Self {
        Self {
            enabled: false,
            playback_multiplier_enabled: false,
            multiplier: 1.0,
            playback_multiplier: 1.0,
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

    pub fn set_playback_multiplier(&mut self, multiplier: f32) {
        self.playback_multiplier = multiplier;
    }

    pub fn set_playback_multiplier_enabled(&mut self, enabled: bool) {
        self.playback_multiplier_enabled = enabled;
    }

    pub fn apply(&self, flipper: &mut CSFlipperImp) {
        if self.playback_multiplier_enabled {
            flipper.game_speed = self.playback_multiplier;
        } else if self.enabled {
            flipper.game_speed = self.multiplier;
        } else if !self.enabled && flipper.game_speed != 1.0 {
            flipper.game_speed = 1.0;
        }
    }
}
