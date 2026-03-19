pub mod vad;
pub mod decay;
pub mod personality;
pub mod rumination;
pub mod config;
pub mod engine;
pub mod persistence;
pub mod prompt;
pub mod multi_agent;
pub mod plutchik;
pub mod memory;

pub use engine::Engine;
pub use vad::VadState;
pub use personality::OceanProfile;
pub use rumination::RuminationEntry;
pub use config::{EventConfig, BehaviorConfig};
pub use plutchik::{PlutchikState, PlutchikResult, classify_plutchik};
pub use memory::EmotionalMemory;
