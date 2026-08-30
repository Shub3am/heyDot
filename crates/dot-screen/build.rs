//! Links screencapturekit's Swift bridge on Macs that have only the Command Line Tools.
//! Must not build anything itself; screencapturekit's own build script builds the bridge.

use std::path::Path;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=DEVELOPER_DIR");

    // screencapturekit looks for the Swift compatibility libraries inside Xcode only. Without Xcode
    // they sit next to swiftc, and the link fails on `__swift_FORCE_LOAD_$_swiftCompatibility56`.
    // A link search path reaches every crate that depends on this one.
    let swiftc = Command::new("xcrun")
        .args(["--find", "swiftc"])
        .output()
        .expect("xcrun ships with the macOS developer tools");
    let swiftc = String::from_utf8(swiftc.stdout).expect("xcrun prints a UTF-8 path");
    let swift_libraries = Path::new(swiftc.trim())
        .parent()
        .expect("swiftc sits in a bin folder")
        .join("../lib/swift/macosx");
    println!(
        "cargo:rustc-link-search=native={}",
        swift_libraries.display()
    );

    // Link args reach only the package that prints them, so screencapturekit's own rpath never
    // reaches these tests. app/src-tauri/build.rs repeats this line for the app.
    println!("cargo:rustc-link-arg=-Wl,-rpath,/usr/lib/swift");
}
