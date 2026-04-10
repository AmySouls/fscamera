#[cfg(feature = "nightreign")]
pub use nightreign::cs::{
    WorldChrMan,
    CSCamera,
    CSPersCam, 
    CSTaskGroupIndex, 
    CSTaskImp, 
    CSFlipperImp,
    CSWindowImp,
    WorldAreaTime,
    MoveMapStep
};

#[cfg(feature = "nightreign")]
pub use nightreign::fd4::{
    FD4TaskData
};

#[cfg(feature = "darksouls3")]
pub use darksouls3::sprj::{
    WorldChrMan,
    SprjCamera as CSCamera,
    SprjPersCam as CSPersCam, 
    SprjTaskGroupIndex as CSTaskGroupIndex, 
    SprjTaskImp as CSTaskImp, 
    SprjFlipperImp as CSFlipperImp,
    SprjWindowImp as CSWindowImp,
    MoveMapStep,
};

#[cfg(feature = "darksouls3")]
pub use darksouls3::fd4::FD4TaskData;

#[cfg(not(any(feature = "nightreign", feature = "darksouls3")))]
pub use eldenring::cs::{
    WorldChrMan,
    CSCamera,
    CSPersCam, 
    CSTaskGroupIndex, 
    CSTaskImp, 
    CSFlipper as CSFlipperImp,
    CSWindowImp,
    WorldAreaTime,
    MoveMapStep,
    
};

#[cfg(not(any(feature = "nightreign", feature = "darksouls3")))]
pub use eldenring::fd4::{
    FD4TaskData
};