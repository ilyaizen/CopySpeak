// Effect identifier used by VoiceProfile.effects.

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

