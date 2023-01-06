use std::{
    ffi::CString,
    process::Command
};

use assert_cmd::prelude::*;
use pretty_assertions::assert_eq;
use rstest::rstest;
use tempfile::NamedTempFile;

/// Test that fsx-rs's testing sequence is stable, and identical to the C-based
/// FSX's as of FreeBSD 14.0.
#[rstest]
// Equivalent to C's fsx -R -N 10 -d  -S 4 -o 65536 -O
#[case("-N10 -S 4",
"[INFO  fsx] Using seed 4
[INFO  fsx] 1 write 0x35b79 thru 0x3ffff (0xa487 bytes)
[INFO  fsx] 2 write 0x2c014 thru 0x3c013 (0x10000 bytes)
[INFO  fsx] 3 read 0x1abd3 thru 0x2abd2 (0x10000 bytes)
[INFO  fsx] 4 write 0x2ccb1 thru 0x3ccb0 (0x10000 bytes)
[INFO  fsx] 5 mapwrite 0x3e3b0 thru 0x3ffff (0x1c50 bytes)
[INFO  fsx] 6 mapread 0xcb52 thru 0x1cb51 (0x10000 bytes)
[INFO  fsx] 7 write 0x3b714 thru 0x3ffff (0x48ec bytes)
[INFO  fsx] 8 mapread 0x11e77 thru 0x21e76 (0x10000 bytes)
[INFO  fsx] 9 write 0xc8d9 thru 0x1c8d8 (0x10000 bytes)
[INFO  fsx] 10 write 0x6940 thru 0x1693f (0x10000 bytes)
")]
fn stability(#[case] args: &str, #[case] stderr: &str) {
    let tf = NamedTempFile::new().unwrap();

    let cmd = Command::cargo_bin("fsx").unwrap()
        .env("RUST_LOG", "debug")
        .args(args.split_ascii_whitespace())
        .arg(tf.path())
        .assert()
        .success();
    let actual_stderr = CString::new(cmd.get_output().stderr.clone())
        .unwrap()
        .into_string()
        .unwrap();
    assert_eq!(actual_stderr, stderr);
}
