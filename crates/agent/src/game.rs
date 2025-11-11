use std::sync::LazyLock;

use eldenring::cs::CSPersCam;
use fromsoft_shared::{OwnedPtr, Program};
use pelite::pe::Pe;
use protocol::RemoteError;

pub(crate) struct GameOffsets {
    pub move_map_step: u32,
    pub scaleform_update_b: u32,
    pub no_dead_flag: u32,
}

static GAME_OFFSETS: LazyLock<Result<GameOffsets, RemoteError>> = LazyLock::new(|| {
    let program = Program::current();
    // Detect running game using PE header
    let resources = program.resources().map_err(|_| RemoteError::UnknownGame)?;

    let (product, version) = {
        let info = resources
            .version_info()
            .map_err(|_| RemoteError::UnknownGame)?;

        let product_version = info
            .fixed()
            .ok_or(RemoteError::UnknownGame)?
            .dwProductVersion;

        let version = format!(
            "{}.{}.{}.{}",
            product_version.Major,
            product_version.Minor,
            product_version.Patch,
            product_version.Build,
        );

        let mut product: Option<String> = None;
        let language = info.translation().first().ok_or(RemoteError::UnknownGame)?;
        info.strings(*language, |k, v| {
            if k == "ProductName" {
                product = Some(v.to_string())
            }
        });

        (product.ok_or(RemoteError::UnknownGame)?, version)
    };

    Ok(match (product.as_str(), version.as_str()) {
        ("ELDEN RING NIGHTREIGN", "1.1.4.0") => GameOffsets {
            move_map_step: 0xba8a70,
            scaleform_update_b: 0xe26460,
            no_dead_flag: 0x3b045c4,
        },
        ("ELDEN RING NIGHTREIGN", "1.1.5.0") => GameOffsets {
            move_map_step: 0xba8a70,
            scaleform_update_b: 0xe26460,
            no_dead_flag: 0x3b045c4,
        },
        ("ELDEN RING NIGHTREIGN", "1.2.0.0") => GameOffsets {
            move_map_step: 0xbe9ff0,
            scaleform_update_b: 0xe6b8c0,
            no_dead_flag: 0x3b7b604,
        },
        ("ELDEN RING NIGHTREIGN", "1.2.1.0") => GameOffsets {
            move_map_step: 0xbea7e0,
            scaleform_update_b: 0xe6c0b0,
            no_dead_flag: 0x3b7b604,
        },
        ("ELDEN RING NIGHTREIGN", "1.2.2.0") => GameOffsets {
            move_map_step: 0xbee8c0,
            scaleform_update_b: 0xe701e0,
            no_dead_flag: 0x3b8e624,
        },
        ("ELDEN RING NIGHTREIGN", "1.2.3.0") => GameOffsets {
            move_map_step: 0xbf0270,
            scaleform_update_b: 0xe71e50,
            no_dead_flag: 0x3b9ab24,
        },
        ("ELDEN RING NIGHTREIGN", "1.2.4.0") => GameOffsets {
            move_map_step: 0xbf0260,
            scaleform_update_b: 0x56be3af,
            no_dead_flag: 0x3b9ab24,
        },
        _ => return Err(RemoteError::UnknownGame),
    })
});

pub(crate) fn get_offsets() -> Result<&'static GameOffsets, &'static RemoteError> {
    (*GAME_OFFSETS).as_ref()
}

#[repr(C)]
/// Source of name: RTTI
#[fromsoft_shared::singleton("CSCamera")]
pub struct CSCamera {
    pub pers_cam_1: OwnedPtr<CSPersCam>,
    pub pers_cam_2: OwnedPtr<CSPersCam>,
    pub pers_cam_3: OwnedPtr<CSPersCam>,
    pub pers_cam_4: OwnedPtr<CSPersCam>,

    // 0b00100000 // Copy from pers_cam_4 into pers_cam_1
    // 0b00010000 // Copy from pers_cam_3 into pers_cam_1
    // 0b00001000 // Copy from pers_cam_2 into pers_cam_1
    // 0b00000100 // Copy from pers_cam_4 into pers_cam_1
    // 0b00000010 // Copy from pers_cam_4 into pers_cam_1
    // 0b00000001 // Copy from pers_cam_2 into pers_cam_1
    pub camera_mask: u32,

    unk24: u32,
    unk28: usize,
}

#[repr(C)]
#[fromsoft_shared::singleton("CSFlipper")]
pub struct CSFlipperImp {
    unk0: [u8; 0x2D4],
    pub time_multiplier: f32,
}

#[repr(C)]
#[fromsoft_shared::singleton("WorldAreaTime")]
pub struct WorldAreaTime {
    unk0: [u8; 0x28],
    pub request_hour: u32,
    pub request_minute: u32,
    pub request_second: u32,
}

#[repr(C)]
pub struct MoveMapStep {
    unk0: [u8; 0x130],
    pub debug_pause: bool,
}

#[repr(C)]
#[fromsoft_shared::singleton("WorldChrMan")]
pub struct WorldChrMan {
    unk0: [u8; 0x174e8],
    pub main_player: Option<OwnedPtr<ChrIns>>,
}

#[repr(C)]
pub struct ChrIns {
    unk0: [u8; 0x38],
    pub current_map_id: i32,
    pub previous_map_id: i32,
    unk40: [u8; 0x20],
    pub chr_ctrl: OwnedPtr<ChrCtrl>,
    unk68: [u8; 0x150],
    pub modules: OwnedPtr<ChrModules>,
}

#[repr(C)]
pub struct ChrCtrl {
    unk0: [u8; 0xf0],
    pub flags: u8,
}

#[repr(C)]
pub struct ChrModules {
    pub data: OwnedPtr<ChrDataModule>,
    unk8: [u8; 0x68],
    pub fall: OwnedPtr<CSChrFallModule>,
}

#[repr(C)]
pub struct ChrDataModule {
    unk0: [u8; 0x189],
    pub no_dead: bool,
}

#[repr(C)]
pub struct CSChrFallModule {
    vtable: i64,
    unk8: [u8; 0x10],
    pub fall_timer: f32,
    unk1c: u32,
}
