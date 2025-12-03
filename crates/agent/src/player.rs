use crate::game::get_offsets;
use nightreign::cs::WorldChrMan;
use fromsoftware_shared::Program;
use pelite::pe64::{Pe, Va};

pub struct Player {
    pub no_dead: bool,
    pub no_move: bool,

    no_dead_va: Va,
}

impl Default for Player {
    fn default() -> Self {
        let offsets = get_offsets().map_err(|e| e.clone()).unwrap();
        let program = Program::current();
        let no_dead_va = program.rva_to_va(offsets.no_dead_flag).unwrap();

        Self {
            no_dead: false,
            no_move: false,

            no_dead_va,
        }
    }
}

impl Player {
    pub fn apply(&self, world_chr_man: &mut WorldChrMan) {
        if let Some(player) = world_chr_man.main_player.as_mut() {
            let mut chr_control_flags = player.chr_ctrl.flags & 0b11011111;
            if !self.no_move {
                chr_control_flags |= 0b00100000;
            }

            player.chr_ctrl.flags = chr_control_flags;
            unsafe { *(self.no_dead_va as *mut bool) = self.no_dead };
        }
    }
}
