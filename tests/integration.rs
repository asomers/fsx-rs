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
#[case("-O -N10 -S 4",
"[INFO  fsx] Using seed 4
[INFO  fsx]  1 write    0x35b79 .. 0x3ffff ( 0xa487 bytes)
[INFO  fsx]  2 write    0x2c014 .. 0x3c013 (0x10000 bytes)
[INFO  fsx]  3 read     0x1abd3 .. 0x2abd2 (0x10000 bytes)
[INFO  fsx]  4 write    0x2ccb1 .. 0x3ccb0 (0x10000 bytes)
[INFO  fsx]  5 mapwrite 0x3e3b0 .. 0x3ffff ( 0x1c50 bytes)
[INFO  fsx]  6 mapread   0xcb52 .. 0x1cb51 (0x10000 bytes)
[INFO  fsx]  7 write    0x3b714 .. 0x3ffff ( 0x48ec bytes)
[INFO  fsx]  8 mapread  0x11e77 .. 0x21e76 (0x10000 bytes)
[INFO  fsx]  9 write     0xc8d9 .. 0x1c8d8 (0x10000 bytes)
[INFO  fsx] 10 write     0x6940 .. 0x1693f (0x10000 bytes)
")]
// Equivalent to C's fsx -N 10 -S 4 -o 65536 -O -RW.  Disables mmapped read and
// write.
#[case("-O -N10 -RW -S 4",
"[INFO  fsx] Using seed 4
[DEBUG fsx]  1 skipping zero size read
[INFO  fsx]  2 truncate     0x0 => 0x2c014
[INFO  fsx]  3 write    0x1abd3 .. 0x2abd2 (0x10000 bytes)
[INFO  fsx]  4 read     0x19db5 .. 0x29db4 (0x10000 bytes)
[INFO  fsx]  5 truncate 0x2c014 => 0x3e3b0
[INFO  fsx]  6 read     0x33082 .. 0x3e3af ( 0xb32e bytes)
[INFO  fsx]  7 read     0x13354 .. 0x23353 (0x10000 bytes)
[INFO  fsx]  8 read     0x23bb7 .. 0x33bb6 (0x10000 bytes)
[INFO  fsx]  9 read     0x395a9 .. 0x3e3af ( 0x4e07 bytes)
[INFO  fsx] 10 read      0x7390 .. 0x1738f (0x10000 bytes)
")]
// Equivalent to C's fsx -N 10 -d -S 6 -o 65536 -O.  Includes both truncate
// down and truncate up.
#[case("-O -N10 -S 6",
"[INFO  fsx] Using seed 6
[INFO  fsx]  1 write     0xb97f .. 0x1b97e (0x10000 bytes)
[INFO  fsx]  2 mapwrite 0x1aa09 .. 0x2aa08 (0x10000 bytes)
[INFO  fsx]  3 truncate 0x2aa09 => 0x35509
[INFO  fsx]  4 read     0x11024 .. 0x21023 (0x10000 bytes)
[INFO  fsx]  5 mapread  0x296a0 .. 0x35508 ( 0xbe69 bytes)
[INFO  fsx]  6 truncate 0x35509 => 0x2d7a2
[INFO  fsx]  7 write    0x2c959 .. 0x3c958 (0x10000 bytes)
[INFO  fsx]  8 write    0x3b513 .. 0x3ffff ( 0x4aed bytes)
[INFO  fsx]  9 read     0x1c693 .. 0x2c692 (0x10000 bytes)
[INFO  fsx] 10 mapread   0xfc15 .. 0x1fc14 (0x10000 bytes)
")]
// Equivalent to C's fsx -b 100 -N 110 -S 4 -o 65536 -O. Uses "-b"
#[case("-O -N 110 -b 100 -S 4",
"[INFO  fsx] Using seed 4
[INFO  fsx] 100 mapwrite   0x6a1 .. 0x106a0 (0x10000 bytes)
[INFO  fsx] 101 read     0x2ae4a .. 0x3ae49 (0x10000 bytes)
[INFO  fsx] 102 write    0x11f35 .. 0x21f34 (0x10000 bytes)
[INFO  fsx] 103 mapread  0x2083b .. 0x3083a (0x10000 bytes)
[INFO  fsx] 104 write     0x9c86 .. 0x19c85 (0x10000 bytes)
[INFO  fsx] 105 mapread  0x1a80d .. 0x2a80c (0x10000 bytes)
[INFO  fsx] 106 truncate 0x3e589 => 0x25a3c
[INFO  fsx] 107 read      0x16c3 .. 0x116c2 (0x10000 bytes)
[INFO  fsx] 108 mapwrite 0x1ba38 .. 0x2ba37 (0x10000 bytes)
[INFO  fsx] 109 truncate 0x2ba38 => 0x2e53c
[INFO  fsx] 110 mapwrite 0x124ae .. 0x224ad (0x10000 bytes)
")]
// Equivalent to C's fsx -N 2 -S 13 -o 65536 -O -c 2
// Exercises closeopen
#[case("-O -N 2 -S 13 -c 2",
"[INFO  fsx] Using seed 13
[INFO  fsx] 1 mapwrite  0x1781 .. 0x11780 (0x10000 bytes)
[INFO  fsx] 1 close/open
[INFO  fsx] 2 read      0xf512 .. 0x11780 ( 0x226f bytes)
[INFO  fsx] 2 close/open
")]
// Equivalent to C's fsx -N 2 -S 20
// Uses random oplen
#[case("-N10 -S 20",
"[INFO  fsx] Using seed 20
[DEBUG fsx]  1 skipping zero size read
[INFO  fsx]  2 write    0x19f18 .. 0x249f6 ( 0xaadf bytes)
[INFO  fsx]  3 write    0x3a8ba .. 0x3f983 ( 0x50ca bytes)
[INFO  fsx]  4 mapwrite 0x17b18 .. 0x1be26 ( 0x430f bytes)
[INFO  fsx]  5 write    0x314db .. 0x3e9a7 ( 0xd4cd bytes)
[INFO  fsx]  6 write    0x3ac28 .. 0x3ffff ( 0x53d8 bytes)
[INFO  fsx]  7 truncate 0x40000 =>  0x54f7
[INFO  fsx]  8 mapread   0x1d79 ..  0x54f6 ( 0x377e bytes)
[INFO  fsx]  9 truncate  0x54f7 => 0x24268
[INFO  fsx] 10 read     0x1110e .. 0x12858 ( 0x174b bytes)
")]
// Equivalent to C's fsx -N 10 -S 30 -o 4096
// Exercises -o
#[case("-N 10 -S 30 -o 4096",
"[INFO  fsx] Using seed 30
[INFO  fsx]  1 write     0x7f70 ..  0x8ed0 ( 0xf61 bytes)
[INFO  fsx]  2 mapread    0xc62 ..  0x1794 ( 0xb33 bytes)
[INFO  fsx]  3 write    0x16a35 .. 0x179b4 ( 0xf80 bytes)
[INFO  fsx]  4 truncate 0x179b5 => 0x146fb
[INFO  fsx]  5 truncate 0x146fb =>  0x6d78
[INFO  fsx]  6 write    0x271bd .. 0x27bca ( 0xa0e bytes)
[INFO  fsx]  7 mapread  0x137f0 .. 0x13a45 ( 0x256 bytes)
[INFO  fsx]  8 write     0xe378 ..  0xe3d2 (  0x5b bytes)
[INFO  fsx]  9 truncate 0x27bcb => 0x2b910
[INFO  fsx] 10 mapread  0x28200 .. 0x28b28 ( 0x929 bytes)
")]
// Equivalent to C's fsx -N 10 -S 40 -l 1048576
// Exercises -l
#[case("-N 10 -S 40 -l 1048576",
"[INFO  fsx] Using seed 40
[DEBUG fsx]  1 skipping zero size read
[INFO  fsx]  2 write     0x1e6f6 ..  0x23555 ( 0x4e60 bytes)
[INFO  fsx]  3 truncate  0x23556 =>  0x3b953
[INFO  fsx]  4 mapread   0x39db2 ..  0x3b952 ( 0x1ba1 bytes)
[INFO  fsx]  5 mapread    0xaed5 ..   0xe87b ( 0x39a7 bytes)
[INFO  fsx]  6 mapwrite  0x18e47 ..  0x23a83 ( 0xac3d bytes)
[INFO  fsx]  7 write     0xf8d72 ..  0xfffff ( 0x728e bytes)
[INFO  fsx]  8 read      0x25453 ..  0x2e0be ( 0x8c6c bytes)
[INFO  fsx]  9 read      0x6d53b ..  0x6f500 ( 0x1fc6 bytes)
[INFO  fsx] 10 truncate 0x100000 =>  0xe57a5
")]
// Equivalent to C's fsx -N 10 -S 42 -N 10 -i 2
// Exercises -i
#[case("-N 10 -S 42 -i 2",
"[INFO  fsx] Using seed 42
[INFO  fsx]  1 write    0x32c3c .. 0x3d016 ( 0xa3db bytes)
[INFO  fsx]  1 msync(MS_INVALIDATE)
[INFO  fsx]  2 truncate 0x3d017 =>  0x1cbe
[INFO  fsx]  2 msync(MS_INVALIDATE)
[INFO  fsx]  3 write     0x8117 .. 0x1107e ( 0x8f68 bytes)
[INFO  fsx]  4 mapread   0x928d ..  0xb356 ( 0x20ca bytes)
[INFO  fsx]  4 msync(MS_INVALIDATE)
[INFO  fsx]  5 write    0x1f8e2 .. 0x2bf33 ( 0xc652 bytes)
[INFO  fsx]  6 truncate 0x2bf34 => 0x37187
[INFO  fsx]  7 mapread  0x26120 .. 0x2da28 ( 0x7909 bytes)
[INFO  fsx]  8 mapread  0x21dc5 .. 0x312d9 ( 0xf515 bytes)
[INFO  fsx]  9 mapread   0x4c8a .. 0x13746 ( 0xeabd bytes)
[INFO  fsx] 10 write    0x24538 .. 0x31d46 ( 0xd80f bytes)
")]
// Equivalent to C's fsx -N 10 -S 45 -r 4096
// Exercises -r
#[case("-N 10 -S 45 -r 4096",
"[INFO  fsx] Using seed 45
[DEBUG fsx]  1 skipping zero size read
[INFO  fsx]  2 truncate     0x0 => 0x34c83
[INFO  fsx]  3 read     0x1e000 .. 0x1e652 (  0x653 bytes)
[INFO  fsx]  4 write    0x344f1 .. 0x35f5c ( 0x1a6c bytes)
[INFO  fsx]  5 mapread  0x13000 .. 0x15dd6 ( 0x2dd7 bytes)
[INFO  fsx]  6 mapwrite  0xb3b9 .. 0x1b0fe ( 0xfd46 bytes)
[INFO  fsx]  7 mapwrite  0xa683 .. 0x16135 ( 0xbab3 bytes)
[INFO  fsx]  8 write     0xac2f .. 0x104e4 ( 0x58b6 bytes)
[INFO  fsx]  9 read      0x9000 ..  0xa762 ( 0x1763 bytes)
[INFO  fsx] 10 truncate 0x35f5d =>  0x4206
")]
// Equivalent to C's fsx -N 10 -S 46 -w 4096
// Exercises -w
#[case("-N 10 -S 46 -w 4096",
"[INFO  fsx] Using seed 46
[INFO  fsx]  1 write    0x36000 .. 0x3d360 ( 0x7361 bytes)
[INFO  fsx]  2 mapread  0x2ecf5 .. 0x348c6 ( 0x5bd2 bytes)
[INFO  fsx]  3 mapwrite 0x13000 .. 0x1e5f4 ( 0xb5f5 bytes)
[INFO  fsx]  4 write    0x30000 .. 0x309c9 (  0x9ca bytes)
[INFO  fsx]  5 mapread  0x1f039 .. 0x2bc32 ( 0xcbfa bytes)
[INFO  fsx]  6 write    0x2d000 .. 0x302d0 ( 0x32d1 bytes)
[INFO  fsx]  7 mapread  0x1c26d .. 0x20d83 ( 0x4b17 bytes)
[INFO  fsx]  8 truncate 0x3d361 => 0x2f688
[INFO  fsx]  9 mapread  0x1eaa5 .. 0x245cf ( 0x5b2b bytes)
[INFO  fsx] 10 mapwrite 0x3a000 .. 0x3f30c ( 0x530d bytes)
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
