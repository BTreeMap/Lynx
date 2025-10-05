use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

use statrs::distribution::{Beta, ContinuousCDF};

const MAX_PROBES_PER_LENGTH: usize = 64;
const SUCCESS_CONFIDENCE: f64 = 0.95;
const TARGET_MAX_EXPECTED_ATTEMPTS: f64 = 4.0;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    // Generate required successes lookup table
    let lower_tail = (1.0 - SUCCESS_CONFIDENCE).max(f64::EPSILON);
    let success_threshold = 1.0 / TARGET_MAX_EXPECTED_ATTEMPTS;

    let mut required_successes = Vec::with_capacity(MAX_PROBES_PER_LENGTH);

    for attempts in 1..=MAX_PROBES_PER_LENGTH {
        let mut min_successes = attempts + 1;

        for successes in 0..=attempts {
            let failures = attempts - successes;
            let alpha = successes as f64 + 1.0;
            let beta = failures as f64 + 1.0;

            let lower_bound = Beta::new(alpha, beta)
                .map(|dist| dist.inverse_cdf(lower_tail))
                .unwrap_or(0.0);

            if lower_bound >= success_threshold {
                min_successes = successes;
                break;
            }
        }

        if min_successes > attempts {
            required_successes.push(u8::MAX);
        } else {
            required_successes.push(min_successes as u8);
        }
    }

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR not set"));
    let dest_path = out_dir.join("required_successes.in");
    let content = format!("{:?}", required_successes);

    fs::write(dest_path, content).expect("failed to write required successes lookup table");

    // Build and bundle frontend if it exists
    let frontend_dir = PathBuf::from("frontend");
    if frontend_dir.exists() && frontend_dir.join("package.json").exists() {
        println!("cargo:rerun-if-changed=frontend/src");
        println!("cargo:rerun-if-changed=frontend/package.json");
        println!("cargo:rerun-if-changed=frontend/vite.config.ts");

        // Check if npm is available
        let npm_available = Command::new("npm")
            .arg("--version")
            .output()
            .is_ok();

        if npm_available {
            println!("cargo:warning=Building frontend with npm...");

            // Install dependencies
            let install_status = Command::new("npm")
                .arg("install")
                .current_dir(&frontend_dir)
                .status()
                .expect("Failed to run npm install");

            if !install_status.success() {
                panic!("npm install failed");
            }

            // Build frontend
            let build_status = Command::new("npm")
                .arg("run")
                .arg("build")
                .current_dir(&frontend_dir)
                .status()
                .expect("Failed to run npm build");

            if !build_status.success() {
                panic!("npm build failed");
            }

            println!("cargo:warning=Frontend built successfully");
        } else {
            println!("cargo:warning=npm not found, skipping frontend build. Static files will need to be provided separately.");
        }
    }
}
