use std::fmt::Display;

use dll_syringe::{process::OwnedProcess, Syringe};
use protocol::{CameraState, InboundGameControlEvent, OutboundGameControlEvent, RemoteError, SettingsData};
use sysinfo::{Pid, System};

const SUPPORTED_GAMES: &[&str] = &[
    // "eldenring.exe",
    // "armoredcore6.exe",
    // "sekiro.exe",
    "eldenring.exe",
    "nightreign.exe",
    "start_protected_game.exe",
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

const DLL_PATH: &str = "agent.dll";

pub struct RemoteGame {
    syringe: Syringe,
}

impl RemoteGame {
    pub fn new(process: &GameProcess) -> Result<Self, Box<dyn std::error::Error>> {
        let process = OwnedProcess::from_pid(process.pid.as_u32())?;
        let syringe = Syringe::for_process(process);

        Ok(Self { syringe })
    }

    pub fn initialize(&self, settings: &SettingsData) -> Result<(), RemoteError> {
        let agent = self
            .syringe
            .find_or_inject(DLL_PATH)
            .map_err(|_| RemoteError::AcquireAgent)?;

        let procedure = unsafe {
            self.syringe
                .get_payload_procedure::<fn(SettingsData) -> Result<(), RemoteError>>(
                    agent,
                    "initialize",
                )
                .ok()
        }
        .flatten()
        .ok_or(RemoteError::AcquireProcedure(
            "initialize".to_string(),
        ))?;

        procedure.call(settings).unwrap()
    }

    pub fn snapshot_camera_state(&self) -> Result<CameraState, RemoteError> {
        let agent = self
            .syringe
            .find_or_inject(DLL_PATH)
            .map_err(|_| RemoteError::AcquireAgent)?;

        let procedure = unsafe {
            self.syringe
                .get_payload_procedure::<fn() -> Result<CameraState, RemoteError>>(
                    agent,
                    "snapshot_camera_state",
                )
                .ok()
        }
        .flatten()
        .ok_or(RemoteError::AcquireProcedure(
            "snapshot_camera_state".to_string(),
        ))?;

        procedure.call().unwrap()
    }

    pub fn post_event(&self, event: InboundGameControlEvent) -> Result<(), RemoteError> {
        let agent = self
            .syringe
            .find_or_inject(DLL_PATH)
            .map_err(|_| RemoteError::AcquireAgent)?;

        let procedure = unsafe {
            self.syringe
                .get_payload_procedure::<fn(InboundGameControlEvent) -> Result<(), RemoteError>>(
                    agent,
                    "post_event",
                )
                .ok()
        }
        .flatten()
        .ok_or(RemoteError::AcquireProcedure(
            "post_event".to_string(),
        ))?;

        procedure.call(&event).unwrap()
    }

    pub fn poll_events(&self) -> Result<Vec<OutboundGameControlEvent>, RemoteError> {
        let agent = self
            .syringe
            .find_or_inject(DLL_PATH)
            .map_err(|_| RemoteError::AcquireAgent)?;

        let procedure = unsafe {
            self.syringe
                .get_payload_procedure::<fn() -> Vec<OutboundGameControlEvent>>(
                    agent,
                    "poll_events",
                )
                .ok()
        }
        .flatten()
        .ok_or(RemoteError::AcquireProcedure(
            "poll_events".to_string(),
        ))?;

        Ok(procedure.call().unwrap())
    }
}
