use std::env;
use std::fs;
use std::path::PathBuf;

use statrs::distribution::{Beta, ContinuousCDF};

const MAX_PROBES_PER_LENGTH: usize = 64;
const SUCCESS_CONFIDENCE: f64 = 0.95;
const TARGET_MAX_EXPECTED_ATTEMPTS: f64 = 4.0;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

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
}
