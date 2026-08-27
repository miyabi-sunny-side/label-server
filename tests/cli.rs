//! Command-line contract of the release binary.
//!
//! Copyright (C) 2026 label-server contributors.
//! Licensed under the GNU General Public License version 3 (GPL-3.0-only).

use std::process::Command;

#[test]
fn version_flag_names_the_crate_version_and_exits() {
    let output = Command::new(env!("CARGO_BIN_EXE_label-server"))
        .arg("--version")
        .output()
        .expect("binary runs");

    assert!(output.status.success(), "{:?}", output.status);
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        format!("label-server {}\n", env!("CARGO_PKG_VERSION"))
    );
    assert!(output.stderr.is_empty());
}
