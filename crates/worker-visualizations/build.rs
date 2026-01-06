use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=setup_build.sh");

    // The cuml-wrapper-rs dependency requires CONDA_PREFIX to be set
    // to find cuML headers and libraries. This must be done BEFORE
    // running cargo, not in this build.rs (since dependency build.rs
    // scripts run in separate processes).
    //
    // To build this crate:
    //   source setup_build.sh && cargo build --release
    //
    // Or use the VS Code task "Run Worker Visualizations" which
    // sources the environment automatically.

    // Check if cuml environment is active
    if let Ok(conda_prefix) = env::var("CONDA_PREFIX") {
        if conda_prefix.contains("cuml") {
            println!(
                "cargo:warning=Building with cuml environment: {}",
                conda_prefix
            );

            // Verify cuml headers exist
            let cuml_include = PathBuf::from(&conda_prefix).join("include/cuml");
            if cuml_include.exists() {
                println!(
                    "cargo:warning=cuML headers found at: {}",
                    cuml_include.display()
                );
            } else {
                println!(
                    "cargo:warning=cuML headers NOT found at: {}",
                    cuml_include.display()
                );
            }
            return;
        }
    }

    // Check if we might have the environment but not activated
    let home = env::var("HOME").expect("HOME not set");
    let cuml_env_path = PathBuf::from(format!("{}/micromamba/envs/cuml", home));

    if cuml_env_path.exists() {
        panic!(
            r#"
================================================================================
ERROR: cuml mamba environment exists but is not activated!

The cuml-wrapper-rs dependency requires the mamba environment to be active
BEFORE running cargo. Environment variables set in build.rs cannot propagate
to dependency build scripts.

Please run:

    cd crates/worker-visualizations
    source setup_build.sh
    cargo build --release

The VS Code task "Run Worker Visualizations" does this automatically.
================================================================================
"#
        );
    } else {
        panic!(
            r#"
================================================================================
ERROR: cuml mamba environment not found!

The worker-visualizations crate requires a cuML environment for UMAP/HDBSCAN.

Please run the setup script first:

    cd crates/worker-visualizations
    source setup_build.sh

This will:
  1. Install micromamba if needed
  2. Create a 'cuml' environment with RAPIDS cuML
  3. Activate the environment

Then build with:

    cargo build --release
================================================================================
"#
        );
    }
}
