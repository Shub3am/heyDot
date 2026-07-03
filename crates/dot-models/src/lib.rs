//! Which models Hey Dot can use, which one suits this Mac, and getting their files on disk.
//! Must not run models, read settings, know about the UI, or decide where models are stored.

mod catalog;

pub use catalog::{
    CATALOG, Model, ModelFile, PARAKEET_TDT_V3, QWEN3_VL_2B, QWEN3_VL_4B, QWEN3_VL_8B,
    SILERO_VAD_V6,
};
