use std::{
    fs::{create_dir_all, File},
    path::PathBuf,
    time::Duration,
};

use anyhow::{Context, Result};
use log::{debug, error};
use ron::ser::PrettyConfig;
use serde::{Deserialize, Serialize};
use tokio::time::Instant;

use crate::{SONUSMIX_APP_ID, APP_VERSION};

use super::{SonusmixReducer, SonusmixState};

fn data_dir() -> Option<PathBuf> {
    std::env::var("SONUSMIX_DATA_DIR")
        .map(PathBuf::from)
        .ok()
        .or_else(dirs::data_local_dir)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct PersistentState {
    version: String,
    state: SonusmixState,
}

impl PersistentState {
    pub fn from_state(mut state: SonusmixState) -> Self {
        // Remove links that aren't locked
        state.links.retain(|link| link.state.is_locked());
        // Remove applications that aren't active
        state.applications.retain(|_, application| application.is_active);

        Self {
            version: APP_VERSION.to_string(),
            state,
        }
    }

    pub fn into_state(self) -> SonusmixState {
        self.state
    }

    pub fn save(&self) -> Result<()> {
        let state_dir = data_dir().context("Could not resolve data dir")?;
        create_dir_all(state_dir.join(SONUSMIX_APP_ID))
            .context("Failed to create Sonusmix data dir")?;
        let state_file = File::create(state_dir.join(SONUSMIX_APP_ID).join("state.ron"))
            .context("Failed to create state file")?;
        ron::ser::to_writer_pretty(state_file, self, PrettyConfig::new())
            .context("Failed to serialize state")
    }

    pub fn load() -> Result<Self> {
        let state_dir = data_dir().context("Could not resolve data dir")?;
        let state_file = File::open(state_dir.join(SONUSMIX_APP_ID).join("state.ron"))
            .context("Failed to open state file")?;
        ron::de::from_reader(state_file).context("Failed to deserialize state")
    }
}

pub(super) async fn autosave_task() {
    let (tx, rx) = relm4::channel();

    let mut state = SonusmixReducer::subscribe_msg(&tx, |state, msg| (state, msg));
    let mut updated = false;

    const SAVE_FREQUENCY: Duration = Duration::from_secs(30);

    let sleep = tokio::time::sleep(SAVE_FREQUENCY);
    tokio::pin!(sleep);

    loop {
        tokio::select! {
            () = &mut sleep => {
                // Timer elapsed, save if there was an update
                if updated {
                    debug!("Saving state");
                    updated = false;
                    let persistent_state = PersistentState::from_state(state.as_ref().clone());
                    if let Err(err) = persistent_state.save() {
                        error!("Error saving state: {err:#}");
                    }
                }
                sleep.as_mut().reset(Instant::now() + SAVE_FREQUENCY);
            }
            Some((new_state, message)) = rx.recv() => {
                state = new_state;
                if message.is_some() {
                    updated = true;
                }
            }
            else => break,
        }
    }
}
