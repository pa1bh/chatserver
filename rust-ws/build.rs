use std::process::Command;

fn main() {
    // Get rustc version
    let output = Command::new("rustc")
        .arg("--version")
        .output()
        .expect("Failed to execute rustc");

    let version = String::from_utf8_lossy(&output.stdout);
    // Extract version like "1.75.0" from "rustc 1.75.0 (..."
    let version = version
        .split_whitespace()
        .nth(1)
        .unwrap_or("unknown")
        .to_string();

    println!("cargo:rustc-env=RUSTC_VERSION={}", version);
    println!("cargo:rerun-if-changed=build.rs");
}
