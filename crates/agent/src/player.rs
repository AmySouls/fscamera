use crate::game::get_offsets;
use crate::game_compat::WorldChrMan;
use fromsoftware_shared::Program;
use pelite::pe64::{Pe, Va};

pub struct Player {
    pub no_dead: bool,
    pub no_move: bool,
}

impl Default for Player {
    fn default() -> Self {
        Self {
            no_dead: false,
            no_move: false,
        }
    }
}

impl Player {
    pub fn apply(&self, world_chr_man: &mut WorldChrMan) {
        if let Some(player) = world_chr_man.main_player.as_mut() {
            #[cfg(feature = "nightreign")]
            {
                if self.no_move { player.chr_ins.chr_ctrl.flags |= 0b00100000 };
                //else { player.chr_ins.chr_ctrl.flags &= !0b11111011; };
                if self.no_dead { player.chr_ins.module_container.data.debug_flags |= 0b00000100 }
                else { player.chr_ins.module_container.data.debug_flags &= !0b11111011; };
            }

            #[cfg(not(any(feature = "nightreign", feature = "darksouls3")))]
            {
                player.chr_ins.debug_flags.set_disabled_movement(self.no_move);
                if self.no_dead { player.chr_ins.module_container.data.debug_flags |= 0b00000001 }
                else { player.chr_ins.module_container.data.debug_flags &= !0b11111110 }
            }

            #[cfg(feature = "darksouls3")]
            {
                if self.no_move { player.chr_ins.debug_flags |= 0b10000000 }
                else { player.chr_ins.debug_flags &= !0b01111111; };
                if self.no_dead { player.chr_ins.modules.data.debug_flags |= 0b00000100 }
                else { player.chr_ins.modules.data.debug_flags &= !0b11111011; };
            }
        }
    }
}
