use dll_syringe::{process::OwnedProcess, Syringe};
use futures::channel::oneshot;
use futures::{pin_mut, select, FutureExt};
use protocol::keyframe::Keyframe;
use protocol::{CameraMode, CameraState, InboundGameControlEvent, OutboundGameControlEvent, RemoteError, SettingsData};
use smol::Timer;
use std::sync::mpsc;
use std::{
    thread,
    time::Duration,
};

use crate::process::GameProcess;

const DLL_PATH: &str = "agent.dll";

pub struct RemoteGame {
    syringe: Syringe,

    /// Thread hosting a smol runtime for serving a remote game.
    poller_thread: Option<thread::JoinHandle<()>>,
    /// Close down polling thread?
    poller_cancel_tx: Option<futures::channel::oneshot::Sender<()>>,
    events_rx: mpsc::Receiver<OutboundGameControlEvent>,
}

impl RemoteGame {
    pub fn connect(gp: &GameProcess) -> Result<Self, Box<dyn std::error::Error>> {
        let process = OwnedProcess::from_pid(gp.pid.as_u32())?;
        let syringe = Syringe::for_process(process);
        let (events_tx, events_rx) = mpsc::channel();

        // Oneshot channel indicating if the game process should be detached.
        let (poller_cancel_tx, poller_cancel_rx) = oneshot::channel::<()>();

        // Use smol to pump events from game to GUI state
        let poller_thread = {
            let gp = gp.clone();

            thread::Builder::new()
                .name("game_poller".into())
                .spawn(move || {
                    smol::block_on(async move {
                        let process = OwnedProcess::from_pid(gp.pid.as_u32()).unwrap();
                        let syringe = Syringe::for_process(process);

                        let shutdown = poller_cancel_rx.fuse();

                        pin_mut!(shutdown);

                        loop {
                            let timeout = Timer::after(Duration::from_millis(60)).fuse();

                            pin_mut!(timeout);

                            select! {
                                _ = shutdown => {
                                    return;
                                }
                                _ = timeout => {
                                    let Ok(events) = Self::poll_events(&syringe) else {
                                        println!("Could not poll events");
                                        continue;
                                    };

                                    if events.is_empty() {
                                        continue;
                                    }

                                    for event in events {
                                        events_tx.send(event).unwrap();
                                    }
                                }
                            }
                        }
                    });
                })?
        };

        Ok(Self {
            syringe,
            events_rx,
            poller_thread: Some(poller_thread),
            poller_cancel_tx: Some(poller_cancel_tx),
        })
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
        .ok_or(RemoteError::AcquireProcedure("initialize".to_string()))?;

        procedure.call(settings).unwrap()
    }

    pub fn receive_event(&mut self) -> Option<OutboundGameControlEvent> {
        self.events_rx.try_recv().ok()
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

    pub fn set_settings(&self, settings: SettingsData) -> Result<(), RemoteError> {
        self.post_event(InboundGameControlEvent::Settings { settings })
    }

    pub fn set_camera_mode(&self, mode: CameraMode) -> Result<(), RemoteError> {
        self.post_event(InboundGameControlEvent::SetCameraMode { mode })
    }

    pub fn set_hud_disabled(&self, disabled: bool) -> Result<(), RemoteError> {
        self.post_event(InboundGameControlEvent::SetHudDisabled { disabled })
    }

    pub fn set_freecam_locked(&self, locked: bool) -> Result<(), RemoteError> {
        self.post_event(InboundGameControlEvent::SetFreecamLocked { locked })
    }

    pub fn set_freecam_movement_speed(&self, value: f32) -> Result<(), RemoteError> {
        self.post_event(InboundGameControlEvent::SetFreecamMovementSpeed { value })
    }

    pub fn set_freecam_rotation_speed(&self, value: f32) -> Result<(), RemoteError> {
        self.post_event(InboundGameControlEvent::SetFreecamRotationSpeed { value })
    }

    pub fn set_debug_pause_enabled(&self, enabled: bool) -> Result<(), RemoteError> {
        self.post_event(InboundGameControlEvent::SetDebugPauseEnabled { enabled })
    }

    pub fn set_time_of_day(&self, hours: u8, minutes: u8, seconds: u8) -> Result<(), RemoteError> {
        self.post_event(InboundGameControlEvent::SetTimeOfDay { hours, minutes, seconds })
    }

    pub fn set_character_no_dead(&self, enabled: bool) -> Result<(), RemoteError> {
        self.post_event(InboundGameControlEvent::SetCharacterNoDead { enabled })
    }

    pub fn set_character_no_move(&self, enabled: bool) -> Result<(), RemoteError> {
        self.post_event(InboundGameControlEvent::SetCharacterNoMove { enabled })
    }

    pub fn set_gamespeed_multiplier(&self, value: f32) -> Result<(), RemoteError> {
        self.post_event(InboundGameControlEvent::SetGameSpeedMultiplier { value })
    }

    pub fn set_gamespeed_multiplier_enabled(&self, enabled: bool) -> Result<(), RemoteError> {
        self.post_event(InboundGameControlEvent::SetGameSpeedMultiplierEnabled { enabled })
    }

    pub fn set_keyframes(&self, keyframes: Vec<Keyframe>) -> Result<(), RemoteError> {
        self.post_event(InboundGameControlEvent::SetKeyframes { keyframes })
    }

    pub fn set_playback_state(&self, playing: bool, time: f32) -> Result<(), RemoteError> {
        self.post_event(InboundGameControlEvent::SetPlaybackState { playing, time })
    }

    pub fn set_freecam_fov(&self, fov: f32) -> Result<(), RemoteError> {
        self.post_event(InboundGameControlEvent::SetFreecamFov { fov })
    }

    fn post_event(&self, event: InboundGameControlEvent) -> Result<(), RemoteError> {
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

    pub fn poll_events(syringe: &Syringe) -> Result<Vec<OutboundGameControlEvent>, RemoteError> {
        let agent = syringe
            .find_or_inject(DLL_PATH)
            .map_err(|_| RemoteError::AcquireAgent)?;

        let procedure = unsafe {
            syringe
                .get_payload_procedure::<fn() -> Vec<OutboundGameControlEvent>>(
                    agent,
                    "poll_events",
                )
                .ok()
        }
        .flatten()
        .ok_or(RemoteError::AcquireProcedure("poll_events".to_string()))?;

        Ok(procedure.call().unwrap())
    }
}

impl Drop for RemoteGame {
    fn drop(&mut self) {
        if let Some(cancel_tx) = self.poller_cancel_tx.take() {
            cancel_tx.send(()).unwrap();
        }

        if let Some(poller_thread) = self.poller_thread.take() {
            poller_thread.join().unwrap();
        }
    }
}
