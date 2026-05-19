// Effects configuration: gates the Effects feature tab and selects the
// active audio post-processing effect applied to TTS playback.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum EffectId {
    None,
    WalkieTalkie,
    GameBoy,
}

impl Default for EffectId {
    fn default() -> Self {
        EffectId::None
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EffectsConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub active_effect: EffectId,
}
