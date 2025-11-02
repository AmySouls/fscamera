use std::mem::transmute;
use std::{borrow::Cow, fmt::Display, ptr::NonNull, sync::LazyLock};

use eldenring::{
    cs::CSPersCam,
    position::{BlockPosition, HavokPosition},
    Tree,
};
use fromsoft_shared::{OwnedPtr, Program};
use pelite::pe::Pe;
use protocol::RemoteError;

pub(crate) struct GameOffsets {
    pub move_map_step: u32,
    pub field_area: u32,
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
            field_area: 0x3b07678,
            scaleform_update_b: 0xe26460,
            no_dead_flag: 0x3b045c4,
        },
        ("ELDEN RING NIGHTREIGN", "1.1.5.0") => GameOffsets {
            move_map_step: 0xba8a70,
            field_area: 0x3b07678,
            scaleform_update_b: 0xe26460,
            no_dead_flag: 0x3b045c4,
        },
        ("ELDEN RING NIGHTREIGN", "1.2.0.0") => GameOffsets {
            move_map_step: 0xbe9ff0,
            field_area: 0x3b7e6f8,
            scaleform_update_b: 0xe6b8c0,
            no_dead_flag: 0x3b7b604,
        },
        ("ELDEN RING NIGHTREIGN", "1.2.1.0") => GameOffsets {
            move_map_step: 0xbea7e0,
            field_area: 0x3b7e6f8,
            scaleform_update_b: 0xe6c0b0,
            no_dead_flag: 0x3b7b604,
        },
        ("ELDEN RING NIGHTREIGN", "1.2.2.0") => GameOffsets {
            move_map_step: 0xbee8c0,
            field_area: 0x3b91710,
            scaleform_update_b: 0xe701e0,
            no_dead_flag: 0x3b8e624,
        },
        ("ELDEN RING NIGHTREIGN", "1.2.3.0") => GameOffsets {
            move_map_step: 0xbf0270,
            field_area: 0x3b9dc10,
            scaleform_update_b: 0xe71e50,
            no_dead_flag: 0x3b9ab24,
        },
        ("ELDEN RING NIGHTREIGN", "1.2.4.0") => GameOffsets {
            move_map_step: 0xbf0260,
            field_area: 0x3b9dc10,
            scaleform_update_b: 0x56be3af,
            no_dead_flag: 0x3b9ab24,
        },
        _ => return Err(RemoteError::UnknownGame),
    })
});

pub(crate) fn get_offsets(program: &Program) -> Result<&'static GameOffsets, &'static RemoteError> {
    (*GAME_OFFSETS).as_ref()
}

pub fn physics_coords_to_block_coords(
    field_area: &FieldArea,
    map_id: &MapId,
    physics_coords: &HavokPosition,
) -> Option<BlockPosition> {
    let program = Program::current();
    let offsets = get_offsets(&program).unwrap();

    let Some(world_block_info) = field_area.world_info_owner.world_block_info_by_map(map_id) else {
        return None;
    };

    let block_coords = BlockPosition {
        x: physics_coords.0 - world_block_info.physics_center.0,
        y: physics_coords.1 - world_block_info.physics_center.1,
        z: physics_coords.2 - world_block_info.physics_center.2,
        yaw: 0.0,
    };

    Some(block_coords)
}

pub fn block_coords_to_physics_coords(
    field_area: &FieldArea,
    map_id: &MapId,
    block_coords: &BlockPosition,
) -> Option<HavokPosition> {
    let program = Program::current();
    let offsets = get_offsets(&program).unwrap();

    let Some(world_block_info) = field_area.world_info_owner.world_block_info_by_map(map_id) else {
        return None;
    };

    let physics_coords = HavokPosition(
        block_coords.x + world_block_info.physics_center.0,
        block_coords.y + world_block_info.physics_center.1,
        block_coords.z + world_block_info.physics_center.2,
        0.0,
    );

    Some(physics_coords)
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

    unk2c: u32,
    unk30: usize,
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
    unk0: [u8; 0xF8],
    pub field_area: OwnedPtr<FieldArea>,
    unk100: [u8; 0x30],
    pub debug_pause: bool,
}

#[repr(C)]
pub struct FieldArea {
    unk0: [u8; 0x18],
    pub world_info_owner: OwnedPtr<WorldInfoOwner>,
    pub game_rend: OwnedPtr<GameRend>,
    unk28: u32,
    pub map_id: MapId,
    // TODO: more
}

#[derive(PartialEq)]
#[repr(u8)]
pub enum FreecamMode {
    Inactive = 0x0,
    ActivePaused = 0x1,
    Active = 0x2,
    Stationary = 0x3,
}

#[repr(C)]
pub struct GameRend {
    unk0: [u8; 0xa4],
    // 0 = inactive, 1 = active and paused, 2 = active and running world, 3 = stationary
    pub freecam_mode: FreecamMode,
}

#[repr(C)]
pub struct WorldInfoOwner {
    vtable: usize,
    /// Count of legacy + ordinary dungeons area infos.
    pub world_area_info_count: u32,
    _padc: u32,
    /// Pointer to start of list of world area infos for legacy + ordinary dungeons.
    pub world_area_info_list_ptr: NonNull<WorldAreaInfo>,
    /// Count of overworld area infos.
    pub world_grid_area_info_count: u32,
    _pad1c: u32,
    /// Pointer to start of list of world area infos for overworld areas.
    pub world_grid_area_info_list_ptr: NonNull<WorldGridAreaInfo>,
    /// Count of combined dungeon + overworld area infos.
    pub world_area_info_all_count: u32,
    _pad2c: u32,
    /// Combined list of pointers to all overworld and dungeon world area infos.
    pub world_area_info_all: [Option<NonNull<WorldAreaInfoBase>>; 30],
    /// Count of block infos.
    pub world_block_info_count: u32,
    _pad3c: u32,
    /// Pointer to start of list of world block infos.
    pub world_block_info_list_ptr: NonNull<WorldBlockInfo>,
    unk130: u32,
    unk134: u32,
    unk138: u64,
    _world_area_info: [WorldAreaInfo; 20],
    _world_block_info: [WorldBlockInfo; 128],
    _world_grid_area_info: [WorldGridAreaInfo; 6],
    // TODO: Add resource stuff
}

impl WorldInfoOwner {
    pub fn world_area_info(&self) -> &[WorldAreaInfo] {
        &self._world_area_info[0..self.world_area_info_count as usize]
    }

    pub fn world_grid_area_info(&self) -> &[WorldGridAreaInfo] {
        &self._world_grid_area_info[0..self.world_grid_area_info_count as usize]
    }

    pub fn world_block_info(&self) -> &[WorldBlockInfo] {
        &self._world_block_info[0..self.world_block_info_count as usize]
    }

    pub fn world_block_info_by_map(&self, block_id: &MapId) -> Option<&WorldBlockInfo> {
        let mut block_id = *block_id;

        // Figure out overworld map ID to prevent storing data reliant on randomized features.
        if block_id.is_small_base_map() {
            // Figure out what grid area info stores the small bases
            let world_area_info = self
                .world_grid_area_info()
                .iter()
                .find(|w| w.base.hosts_small_bases)?;

            for small_base in world_area_info.small_bases.iter() {
                if small_base.block.small_base_block_id == block_id {
                    block_id = small_base.block.small_base_parent_block_id;
                }
            }
        }

        match block_id.is_overworld() {
            true => self
                .world_grid_area_info()
                .iter()
                .flat_map(|a| a.blocks.iter())
                .find(|b| b.map_id == block_id)
                .map(|b| b.block.as_ref()),
            false => self
                .world_block_info()
                .iter()
                .find(|b| b.map_id == block_id),
        }
    }
}

// Source of name: RTTI
#[repr(C)]
pub struct WorldAreaInfoBase {
    vtable: usize,
    pub map_id: MapId,
    pub area_id: u32,
    pub world_info_owner: NonNull<WorldInfoOwner>,
    /// Points to _99 MSB for this area.
    overlay_msb_res_cap: Option<NonNull<()>>,
    unk20: u64,
    unk28: u64,
    pub hosts_small_bases: bool,
    _pad31: [u8; 0x7],
}

// Source of name: RTTI
#[repr(C)]
pub struct WorldAreaInfo {
    pub base: WorldAreaInfoBase,
    /// List index in the WorldInfoOwner
    pub list_index: u32,
    /// Starting offset of the areas blocks in the block list in WorldInfoOwner
    pub block_list_start_index: u32,
    /// Amount of blocks associated with this area in the blocks list.
    pub block_count: u32,
    _pad44: u32,
    /// Pointer to start of the areas block in the WorldInfoOwner block list.
    blocks: *const WorldBlockInfo,
}

// Source of name: RTTI
#[repr(C)]
pub struct WorldGridAreaInfo {
    pub base: WorldAreaInfoBase,
    unk38: [u32; 3],
    unk44: u32,
    unk48: u32,
    unk4c: u32,
    unk50: [u32; 3],
    unk5c: [f32; 4],
    unk6c: [f32; 4],
    pub skybox_map_id: MapId,
    pub skybox_block_info: NonNull<WorldBlockInfo>,
    pub blocks: Tree<WorldGridAreaInfoBlockElement>,
    unka0: Tree<()>,
    unkb8: u64,
    unkc0: u64,
    pub small_bases: Tree<WorldGridAreaInfoSmallBaseBlockElement>,
}

#[repr(C)]
pub struct WorldGridAreaInfoBlockElement {
    pub map_id: MapId,
    _pad4: u32,
    pub block: OwnedPtr<WorldBlockInfo>,
}

#[repr(C)]
pub struct WorldGridAreaInfoSmallBaseBlockElement {
    pub map_id: MapId,
    _pad4: u32,
    // TODO: Might be a struct here instead of a pointer pointer.
    pub block: OwnedPtr<OwnedPtr<WorldBlockInfo>>,
}

// Source of name: RTTI
#[repr(C)]
pub struct WorldBlockInfo {
    vtable: usize,
    pub map_id: MapId,
    unkc: [u8; 0x28],
    pub small_base_block_id: MapId,
    pub small_base_parent_block_id: MapId,
    unk3c: [u8; 0x44],
    pub physics_center: HavokPosition,
    unk90: [u8; 0x60],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MapId(pub i32);

impl MapId {
    /// MapId -1 indicating that some entity is global or not segregated by map.
    pub const fn none() -> Self {
        Self::from_parts(-1, -1, -1, -1)
    }

    /// Constructs a MapId from seperate parts.
    pub const fn from_parts(area: i8, block: i8, region: i8, index: i8) -> Self {
        Self((index as i32) | (region as i32) << 8 | (block as i32) << 16 | (area as i32) << 24)
    }

    pub const fn area(&self) -> i32 {
        self.0 >> 24 & 0xFF
    }

    pub const fn block(&self) -> i32 {
        self.0 >> 16 & 0xFF
    }

    pub const fn region(&self) -> i32 {
        self.0 >> 8 & 0xFF
    }

    pub const fn index(&self) -> i32 {
        self.0 & 0xFF
    }

    pub const fn is_overworld(&self) -> bool {
        self.area() >= 50 && self.area() < 89
    }

    pub const fn is_small_base_map(&self) -> bool {
        self.area() == 89 || (self.area() >= 20 && self.area() < 49)
    }
}
impl From<MapId> for i32 {
    fn from(val: MapId) -> Self {
        val.0
    }
}

impl From<i32> for MapId {
    fn from(value: i32) -> Self {
        Self(value)
    }
}

impl Display for MapId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "m{:0>2}_{:0>2}_{:0>2}_{:0>2}",
            self.area(),
            self.block(),
            self.region(),
            self.index()
        )
    }
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
    // Both of these seemingly switch between some variation map ID and overworld ID?
    pub current_map_id: MapId,
    pub previous_map_id: MapId,
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
