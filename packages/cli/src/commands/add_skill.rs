//! `agentmark add-skill` — install the embedded agent skill into local agent systems.

use crate::config;
use crate::skill;

/// Entry point for `agentmark add-skill`.
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
