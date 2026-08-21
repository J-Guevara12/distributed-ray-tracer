//! Hardware generation label.
//!
//! Wall time only compares within one machine and one configuration of it, so
//! an unlabelled change to either is indistinguishable from a code change. It
//! already happened: the Windows host had power saving on, the guest cannot see
//! that, and `cpu_mhz` was null so nothing caught it. Records either side of it
//! differ by 1.40x with no code change.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, bail};
use serde::{Deserialize, Serialize};

pub const DEFAULT_PATH: &str = "./bench/hardware.toml";

#[derive(Deserialize)]
struct HardwareFile {
    current: String,
    #[serde(flatten)]
    generations: BTreeMap<String, Generation>,
}

/// The description travels with the record so the JSONL stays readable without
/// the file next to it.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Generation {
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Serialize, Clone, Debug)]
pub struct Hardware {
    /// `gen0`, `gen1`, `server1`.
    pub id: String,
    #[serde(flatten)]
    pub generation: Generation,
}

/// Reads the active generation. `override_id` comes from `--hardware` and wins.
pub fn load(path: &Path, override_id: Option<&str>) -> anyhow::Result<Hardware> {
    if !path.exists() {
        bail!(
            "{} is missing. It labels the hardware generation; without it a change of \
             machine is indistinguishable from a change of code.\nCreate one with:\n\n  \
             current = \"gen1\"\n\n  [gen1]\n  description = \"what this machine is\"",
            path.display()
        );
    }

    let content =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let file: HardwareFile =
        toml::from_str(&content).with_context(|| format!("parsing {}", path.display()))?;

    let id = override_id.unwrap_or(&file.current);

    // A typo in `current` would record a label that describes nothing.
    let Some(generation) = file.generations.get(id) else {
        let known: Vec<&str> = file.generations.keys().map(String::as_str).collect();
        bail!(
            "{} does not define generation \"{id}\". Defined: {}",
            path.display(),
            if known.is_empty() {
                "none".to_string()
            } else {
                known.join(", ")
            }
        );
    };

    Ok(Hardware {
        id: id.to_string(),
        generation: generation.clone(),
    })
}
