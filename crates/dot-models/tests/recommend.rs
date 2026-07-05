use dot_models::{
    Chip, Hardware, ModelChoice, QWEN3_VL_2B, QWEN3_VL_4B, QWEN3_VL_8B, Recommendation,
    detect_hardware, recommend,
};

const GIB: u64 = 1024 * 1024 * 1024;

fn apple_silicon_with_ram_bytes(ram_bytes: u64) -> Hardware {
    Hardware {
        chip: Chip::AppleSilicon,
        ram_bytes,
    }
}

#[test]
fn apple_silicon_8gb_gets_2b_with_a_quality_warning() {
    assert_eq!(
        recommend(&apple_silicon_with_ram_bytes(8 * GIB)),
        Recommendation {
            recommended: ModelChoice::Local(&QWEN3_VL_2B),
            also_offered: vec![],
            warn_small_model: true,
        }
    );
}

#[test]
fn apple_silicon_just_under_16gb_gets_2b_with_a_quality_warning() {
    assert_eq!(
        recommend(&apple_silicon_with_ram_bytes(16 * GIB - 1)).recommended,
        ModelChoice::Local(&QWEN3_VL_2B)
    );
}

#[test]
fn apple_silicon_16gb_gets_4b() {
    assert_eq!(
        recommend(&apple_silicon_with_ram_bytes(16 * GIB)),
        Recommendation {
            recommended: ModelChoice::Local(&QWEN3_VL_4B),
            also_offered: vec![],
            warn_small_model: false,
        }
    );
}

#[test]
fn apple_silicon_32gb_gets_4b_and_is_offered_8b() {
    assert_eq!(
        recommend(&apple_silicon_with_ram_bytes(32 * GIB)),
        Recommendation {
            recommended: ModelChoice::Local(&QWEN3_VL_4B),
            also_offered: vec![ModelChoice::Local(&QWEN3_VL_8B)],
            warn_small_model: false,
        }
    );
}

#[test]
fn intel_is_recommended_cloud_with_local_2b_allowed() {
    let intel = Hardware {
        chip: Chip::Intel,
        ram_bytes: 16 * GIB,
    };

    assert_eq!(
        recommend(&intel),
        Recommendation {
            recommended: ModelChoice::Cloud,
            also_offered: vec![ModelChoice::Local(&QWEN3_VL_2B)],
            warn_small_model: false,
        }
    );
}

#[test]
fn detected_hardware_reports_this_machine() {
    let hardware = detect_hardware();

    assert!(hardware.ram_bytes > 0);
    if cfg!(target_arch = "aarch64") {
        assert_eq!(hardware.chip, Chip::AppleSilicon);
    }
}
