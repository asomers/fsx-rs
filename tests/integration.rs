// vim: tw=80
use std::{ffi::CString, fs, process::Command};

use assert_cmd::prelude::*;
use pretty_assertions::assert_eq;
use rstest::rstest;
use tempfile::{NamedTempFile, TempDir};

/// Test that fsx-rs's testing sequence is stable, and identical to the C-based
/// FSX's as of FreeBSD 14.0.
#[rstest]
// Equivalent to C's fsx -N 10 -S 4 -o 65536 -O.  Includes both MapRead
// and MapWrite.
#[case::sixtyfourk_ops(
    "-O -N10 -S 4",
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
"
)]
// Equivalent to C's fsx -N 10 -S 4 -o 65536 -O -RW.  Disables mmapped read and
// write.
#[case::no_mmap(
    "-O -N10 -RW -S 4",
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
"
)]
// Equivalent to C's fsx -N 10 -d -S 6 -o 65536 -O.  Includes both truncate
// down and truncate up.
#[case::truncate(
    "-O -N10 -S 6",
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
"
)]
// Equivalent to C's fsx -b 100 -N 110 -S 4 -o 65536 -O. Uses "-b"
#[case::opnum(
    "-O -N 110 -b 100 -S 4",
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
"
)]
// Equivalent to C's fsx -N 2 -S 13 -o 65536 -O -c 2
// Exercises closeopen
#[case::closeopen(
    "-O -N 2 -S 13 -c 2",
    "[INFO  fsx] Using seed 13
[INFO  fsx] 1 mapwrite  0x1781 .. 0x11780 (0x10000 bytes)
[INFO  fsx] 1 close/open
[INFO  fsx] 2 read      0xf512 .. 0x11780 ( 0x226f bytes)
[INFO  fsx] 2 close/open
"
)]
// Equivalent to C's fsx -N 2 -S 20
// Uses random oplen
#[case::baseline(
    "-N10 -S 20",
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
"
)]
// Equivalent to C's fsx -N 10 -S 30 -o 4096
// Exercises -o
#[case::oplen(
    "-N 10 -S 30 -o 4096",
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
"
)]
// Equivalent to C's fsx -N 10 -S 40 -l 1048576
// Exercises -l
#[case::flen(
    "-N 10 -S 40 -l 1048576",
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
"
)]
// Equivalent to C's fsx -N 10 -S 42 -N 10 -i 2
// Exercises -i
#[case::inval(
    "-N 10 -S 42 -i 2",
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
"
)]
// Equivalent to C's fsx -N 1 -i 1 -S 1
// https://github.com/asomers/fsx-rs/issues/13
#[case::mmap_underflow(
    "-N 1 -S 1 -i 1",
    "[INFO  fsx] Using seed 1
[DEBUG fsx] 1 skipping zero size read
[DEBUG fsx] 1 skipping invalidate of zero-length file
"
)]
// Equivalent to C's fsx -N 10 -S 45 -r 4096
// Exercises -r
#[case::readbdy(
    "-N 10 -S 45 -r 4096",
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
"
)]
// Equivalent to C's fsx -N 10 -S 46 -w 4096
// Exercises -w
#[case::writebdy(
    "-N 10 -S 46 -w 4096",
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
"
)]
// Equivalent to C's fsx -N 4 -t 4096 -S 51
// Exercises -t
#[case::truncbdy(
    "-N 4 -S 51 -t 4096",
    "[INFO  fsx] Using seed 51
[INFO  fsx] 1 truncate     0x0 => 0x16000
[INFO  fsx] 2 truncate 0x16000 =>  0xe000
[INFO  fsx] 3 read      0x94f5 ..  0xd455 ( 0x3f61 bytes)
[INFO  fsx] 4 mapread   0x5b3b ..  0xdfff ( 0x84c5 bytes)
"
)]
// Equivalent to C's fsx -N 10 -S 60 -m 32768:65536
// Exercises -m
#[case::monitor(
    "-N 10 -S 60 -m 32768:65536",
    "[INFO  fsx] Using seed 60
[WARN  fsx]  1 truncate     0x0 =>  0x6f44
[INFO  fsx]  2 read      0x19d0 ..  0x6f43 ( 0x5574 bytes)
[INFO  fsx]  3 truncate  0x6f44 => 0x1f131
[WARN  fsx]  4 mapread   0x7d00 .. 0x146f2 ( 0xc9f3 bytes)
[WARN  fsx]  5 mapread   0x6a24 ..  0xa9ba ( 0x3f97 bytes)
[WARN  fsx]  6 read      0x41c0 .. 0x13ec4 ( 0xfd05 bytes)
[WARN  fsx]  7 truncate 0x1f131 =>  0xccfe
[WARN  fsx]  8 write     0x9b8a ..  0xb6b8 ( 0x1b2f bytes)
[INFO  fsx]  9 mapwrite 0x2a9a3 .. 0x30421 ( 0x5a7f bytes)
[WARN  fsx] 10 mapread   0x7891 ..  0xc8c8 ( 0x5038 bytes)
"
)]
// Equivalent to C's fsx -S 72 -L -N 10
// Exercises -B
#[case::blockmode(
    "-B -S 72 -N 10 -P /tmp",
    "[INFO  fsx] Using seed 72
[INFO  fsx]  1 mapread   0x7a51 ..  0xeef7 ( 0x74a7 bytes)
[INFO  fsx]  2 mapwrite 0x1bfbb .. 0x22bdf ( 0x6c25 bytes)
[INFO  fsx]  3 mapwrite 0x34117 .. 0x3d783 ( 0x966d bytes)
[INFO  fsx]  4 mapwrite 0x3b18d .. 0x3c6ff ( 0x1573 bytes)
[INFO  fsx]  5 mapread  0x1fbfc .. 0x284fa ( 0x88ff bytes)
[INFO  fsx]  6 read      0x8ec4 .. 0x15701 ( 0xc83e bytes)
[INFO  fsx]  7 mapread   0x998c ..  0x9d58 (  0x3cd bytes)
[INFO  fsx]  8 read     0x28865 .. 0x2f824 ( 0x6fc0 bytes)
[INFO  fsx]  9 write     0x5b17 .. 0x10d53 ( 0xb23d bytes)
[INFO  fsx] 10 mapwrite  0xd97b .. 0x19ae3 ( 0xc169 bytes)
"
)]
#[cfg_attr(not(target_os = "freebsd"), ignore)] // Depends on exact PRNG output
fn stability(#[case] args: &str, #[case] stderr: &str) {
    let mut tf = NamedTempFile::new().unwrap();

    if args.contains("-B") {
        // When using -B, must manually set file size before starting program
        tf.as_file_mut().set_len(262144).unwrap();
    }

    let cmd = Command::cargo_bin("fsx")
        .unwrap()
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

#[cfg_attr(not(target_os = "freebsd"), allow(unused))]
#[rstest]
fn miscompare() {
    let tf = NamedTempFile::new().unwrap();

    let cmd = Command::cargo_bin("fsx")
        .unwrap()
        .env("RUST_LOG", "debug")
        .args(["-N10", "-S6", "--inject", "5"])
        .arg(tf.path())
        .assert()
        .failure();
    #[cfg(target_os = "freebsd")] // Depends on exact PRNG output
    {
        let actual_stderr = CString::new(cmd.get_output().stderr.clone())
            .unwrap()
            .into_string()
            .unwrap();
        assert_eq!(
            actual_stderr,
            "[INFO  fsx] Using seed 6
[INFO  fsx]  1 write    0x21c37 .. 0x2d199 ( 0xb563 bytes)
[INFO  fsx]  2 mapwrite 0x35509 .. 0x373de ( 0x1ed6 bytes)
[INFO  fsx]  3 read     0x32a86 .. 0x373de ( 0x4959 bytes)
[INFO  fsx]  4 mapread   0xbf7f .. 0x14c14 ( 0x8c96 bytes)
[INFO  fsx]  6 read     0x16d69 .. 0x226d1 ( 0xb969 bytes)
[INFO  fsx]  7 mapread  0x21125 .. 0x301cd ( 0xf0a9 bytes)
[ERROR fsx] miscompare: offset= 0x21125, size = 0xf0a9
[ERROR fsx] OFFSET  GOOD  BAD  RANGE  
[ERROR fsx] 0x24dea 0x05 0x01  0x8942
[ERROR fsx] Step# (mod 256) for a misdirected write may be 1
[ERROR fsx] LOG DUMP
[ERROR fsx]  0 WRITE    0x21c37 => 0x2d19a ( 0xb563 bytes) HOLE
[ERROR fsx]  1 MAPWRITE 0x35509 => 0x373df ( 0x1ed6 bytes) HOLE
[ERROR fsx]  2 READ     0x32a86 => 0x373df ( 0x4959 bytes)
[ERROR fsx]  3 MAPREAD   0xbf7f => 0x14c15 ( 0x8c96 bytes)
[ERROR fsx]  4 WRITE    0x24dea => 0x2d72d ( 0x8943 bytes)
[ERROR fsx]  5 READ     0x16d69 => 0x226d2 ( 0xb969 bytes)
[ERROR fsx]  6 MAPREAD  0x21125 => 0x301ce ( 0xf0a9 bytes)
"
        );
    }
    // There should be a .fsxgood artifact
    let mut fsxgoodfname = tf.path().to_owned();
    let mut final_component = fsxgoodfname.file_name().unwrap().to_owned();
    final_component.push(".fsxgood");
    fsxgoodfname.set_file_name(final_component);
    assert_eq!(fs::metadata(&fsxgoodfname).unwrap().len(), 262144);

    // finally, clean it up.
    fs::remove_file(&fsxgoodfname).unwrap();
}

#[test]
fn artifacts_dir() {
    let tf = NamedTempFile::new().unwrap();
    let artifacts_dir = TempDir::new().unwrap();

    Command::cargo_bin("fsx")
        .unwrap()
        .env("RUST_LOG", "debug")
        .args(["-N2", "-S2", "--inject", "1", "-P"])
        .arg(artifacts_dir.path())
        .arg(tf.path())
        .assert()
        .failure();
    // Don't bother checking stderr; we cover that in the miscompare test.  Here
    // we're just checking the location of the artifacts.

    // Check the location of the .fsxgood artifact
    let mut fsxgoodfname = artifacts_dir.path().to_owned();
    let mut final_component = tf.path().file_name().unwrap().to_owned();
    final_component.push(".fsxgood");
    fsxgoodfname.push(final_component);
    assert_eq!(fs::metadata(&fsxgoodfname).unwrap().len(), 262144);

    // finally, clean it up.
    fs::remove_file(&fsxgoodfname).unwrap();
}
