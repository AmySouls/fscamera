use std::fmt::Display;
use sysinfo::{Pid, System};

const SUPPORTED_GAMES: &[&str] = &[
    // "armoredcore6.exe",
    // "sekiro.exe",
    #[cfg(not(any(feature = "nightreign", feature = "darksouls3")))]
    "eldenring.exe",
    #[cfg(feature = "nightreign")]
    "nightreign.exe",
    #[cfg(feature = "darksouls3")]
    "DarkSoulsIII.exe",
];

/// Retrieves a list of running games that we should support
pub(crate) fn get_running_games() -> Vec<GameProcess> {
    let mut system = System::new();
    system.refresh_all();

    let mut processes = system
        .processes()
        .iter()
        .map(|x| GameProcess {
            pid: *x.0,
            name: x.1.name().to_string_lossy().into_owned(),
        })
        .filter(|p| SUPPORTED_GAMES.contains(&p.name.as_str()))
        .collect::<Vec<GameProcess>>();

    processes.sort_by(|a, b| b.pid.as_u32().cmp(&a.pid.as_u32()));

    processes
}

#[derive(Debug, Clone, Eq)]
pub(crate) struct GameProcess {
    pub pid: Pid,
    pub name: String,
}

impl Display for GameProcess {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.name, self.pid.as_u32())
    }
}

impl PartialEq for GameProcess {
    fn eq(&self, other: &Self) -> bool {
        self.pid == other.pid
    }
}
