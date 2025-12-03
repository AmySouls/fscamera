use std::sync::LazyLock;

use fromsoftware_shared::Program;
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
pub struct MoveMapStep {
    unk0: [u8; 0x130],
    pub debug_pause: bool,
}
