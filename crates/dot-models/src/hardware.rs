//! Reads the facts about this Mac that decide which model it can run.
//! Must not decide which model to use; that is `recommend`.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Chip {
    AppleSilicon,
    Intel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hardware {
    pub chip: Chip,
    pub ram_bytes: u64,
}

pub fn detect_hardware() -> Hardware {
    let mut system = sysinfo::System::new();
    system.refresh_memory();
    // The architecture this binary was compiled for. The universal build runs its arm64
    // slice on Apple Silicon, so this is only wrong when the user forces Rosetta.
    let chip = if std::env::consts::ARCH == "aarch64" {
        Chip::AppleSilicon
    } else {
        Chip::Intel
    };
    Hardware {
        chip,
        ram_bytes: system.total_memory(),
    }
}
