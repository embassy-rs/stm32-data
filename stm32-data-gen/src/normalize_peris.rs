//! Normalizes CubeDB peripheral instance names to the canonical names used
//! throughout the generated data.
//!
//! The table lives in `data/peripherals.yaml`.

use crate::config;

/// Map a CubeDB instance name to its canonical peripheral name.
pub fn normalize_peri_name(name: &str) -> &str {
    config::PERIPHERALS.normalize.get(name).map_or(name, String::as_str)
}
