//! `agentmark add-skill` — install the embedded agent skill into local agent systems.

use tracing::instrument;

use crate::config;
use crate::skill;

/// Entry point for `agentmark add-skill`.
#[instrument]
pub fn run_add_skill() -> Result<(), Box<dyn std::error::Error>> {
    let home = config::home_dir()?;
    let result = skill::install_skill(&home)?;

    println!("Skill installed to {}", result.canonical_dir.display());

    for name in &result.linked {
        println!("  Linked: {name}");
    }
    for reason in &result.skipped {
        eprintln!("  Skipped: {reason}");
    }

    Ok(())
}
