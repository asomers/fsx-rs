// vim: tw=80
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
// Equivalent to C's fsx -N 10 -S 4 -o 65536 -O.  Includes both MapRead
// and MapWrite.
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
// Equivalent to C's fsx -N 10 -d -S 6 -o 65536 -O.  Includes both truncate
// down and truncate up.
#[case("-N10 -S 6",
"[INFO  fsx] Using seed 6
[INFO  fsx] 1 write 0xb97f thru 0x1b97e (0x10000 bytes)
[INFO  fsx] 2 mapwrite 0x1aa09 thru 0x2aa08 (0x10000 bytes)
[INFO  fsx] 3 truncate from 0x2aa09 to 0x35509
[INFO  fsx] 4 read 0x11024 thru 0x21023 (0x10000 bytes)
[INFO  fsx] 5 mapread 0x296a0 thru 0x35508 (0xbe69 bytes)
[INFO  fsx] 6 truncate from 0x35509 to 0x2d7a2
[INFO  fsx] 7 write 0x2c959 thru 0x3c958 (0x10000 bytes)
[INFO  fsx] 8 write 0x3b513 thru 0x3ffff (0x4aed bytes)
[INFO  fsx] 9 read 0x1c693 thru 0x2c692 (0x10000 bytes)
[INFO  fsx] 10 mapread 0xfc15 thru 0x1fc14 (0x10000 bytes)
")]
// Equivalent to C's fsx -b 100 -N 110 -S 4 -o 65536 -O. Uses "-b"
#[case("-N 110 -b 100 -S 4",
"[INFO  fsx] Using seed 4
[INFO  fsx] 100 mapwrite 0x6a1 thru 0x106a0 (0x10000 bytes)
[INFO  fsx] 101 read 0x2ae4a thru 0x3ae49 (0x10000 bytes)
[INFO  fsx] 102 write 0x11f35 thru 0x21f34 (0x10000 bytes)
[INFO  fsx] 103 mapread 0x2083b thru 0x3083a (0x10000 bytes)
[INFO  fsx] 104 write 0x9c86 thru 0x19c85 (0x10000 bytes)
[INFO  fsx] 105 mapread 0x1a80d thru 0x2a80c (0x10000 bytes)
[INFO  fsx] 106 truncate from 0x3e589 to 0x25a3c
[INFO  fsx] 107 read 0x16c3 thru 0x116c2 (0x10000 bytes)
[INFO  fsx] 108 mapwrite 0x1ba38 thru 0x2ba37 (0x10000 bytes)
[INFO  fsx] 109 truncate from 0x2ba38 to 0x2e53c
[INFO  fsx] 110 mapwrite 0x124ae thru 0x224ad (0x10000 bytes)
")]
// Equivalent to C's fsx -N 2 -S 13 -o 65536 -O -c 2
// Exercises closeopen
#[case("-N 2 -S 13 -c 2",
"[INFO  fsx] Using seed 13
[INFO  fsx] 1 mapwrite 0x1781 thru 0x11780 (0x10000 bytes)
[INFO  fsx] 1 close/open
[INFO  fsx] 2 read 0xf512 thru 0x11780 (0x226f bytes)
[INFO  fsx] 2 close/open
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
