//! Picks the chat model onboarding suggests for a machine.
//! Must not detect hardware or download anything. Cloud is always available in onboarding,
//! so it only appears here when it is the recommendation.

use crate::catalog::{Model, QWEN3_VL_2B, QWEN3_VL_4B, QWEN3_VL_8B};
use crate::hardware::{Chip, Hardware};

const BYTES_PER_GIB: u64 = 1024 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelChoice {
    Local(&'static Model),
    Cloud,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recommendation {
    pub recommended: ModelChoice,
    pub also_offered: Vec<ModelChoice>,
    pub warn_small_model: bool,
}

pub fn recommend(hardware: &Hardware) -> Recommendation {
    let ram_gb = hardware.ram_bytes / BYTES_PER_GIB;
    match hardware.chip {
        Chip::Intel => Recommendation {
            recommended: ModelChoice::Cloud,
            also_offered: vec![ModelChoice::Local(&QWEN3_VL_2B)],
            warn_small_model: false,
        },
        Chip::AppleSilicon if ram_gb >= QWEN3_VL_8B.min_ram_gb => Recommendation {
            recommended: ModelChoice::Local(&QWEN3_VL_4B),
            also_offered: vec![ModelChoice::Local(&QWEN3_VL_8B)],
            warn_small_model: false,
        },
        Chip::AppleSilicon if ram_gb >= QWEN3_VL_4B.min_ram_gb => Recommendation {
            recommended: ModelChoice::Local(&QWEN3_VL_4B),
            also_offered: vec![],
            warn_small_model: false,
        },
        Chip::AppleSilicon => Recommendation {
            recommended: ModelChoice::Local(&QWEN3_VL_2B),
            also_offered: vec![],
            warn_small_model: true,
        },
    }
}
