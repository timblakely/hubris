// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

// Paths used during the XTask build process.
pub struct XTaskPaths {
    // Absolute path to the .cargo directory. Pulled from `env("CARGO_HOME")`,
    // with a fallback to the user's home directory.
    pub cargo_home: PathBuf,
    // Absolute path to the hubris root. Currently expects a local checked-out
    // version. Path is pulled form `env("HUBRIS_ROOT")`, then falls back to
    // `env("CARGO_MANIFEST_DIR")`.
    pub hubris_root: PathBuf,
    // Absolute path to the cargo output directory (read: `target`). Pulled
    // from `env("HUBRIS_TARGET")`, then falls back to
    // `env("CARGO_TARGET_DIR")`.
    pub output_dir: PathBuf,
}

fn user_dir() -> Result<String> {
    std::env::var("HOME")
        .or_else(|_| std::env::var("%HOMEPATH%"))
        .with_context(|| "Could not detect home directory")
}

pub fn config_paths() -> Result<XTaskPaths> {
    let cargo_home = dunce::canonicalize(
        std::env::var("CARGO_HOME").or_else(|_| user_dir())?,
    )?;
    let hubris_root = dunce::canonicalize(
        std::env::var("HUBRIS_ROOT").or_else::<std::env::VarError, _>(
            |_| {
                // When we run with `cargo xtask`, this actually sets the
                // `CARGO_MANIFEST_DIR` to the directory that _`xtask`_ was
                // compiled in, ***not*** the depending crate. In order to get
                // back to the root of the Hubris task, we need to pop up a few
                // directories.
                let mut hubris_root =
                    Path::new(&std::env::var("CARGO_MANIFEST_DIR")?)
                        .to_path_buf();
                hubris_root.pop(); // Get rid of build/xtask
                hubris_root.pop();
                Ok(hubris_root.to_string_lossy().to_string())
            },
        )?,
    )?;
    let output_dir =
        dunce::canonicalize(std::env::var("HUBRIS_TARGET").or_else(|_| {
            //asdf
            std::env::var("CARGO_TARGET_DIR")
        })?)?;
    Ok(XTaskPaths {
        cargo_home,
        hubris_root,
        output_dir,
    })
}
