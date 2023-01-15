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
[INFO  fsx]  1 truncate     0x0 => 0x3cbf8
[INFO  fsx]  2 mapwrite 0x30c3e .. 0x3ffff ( 0xf3c2 bytes)
[INFO  fsx]  3 read      0xa364 .. 0x1a363 (0x10000 bytes)
[INFO  fsx]  4 read     0x2d8b5 .. 0x3d8b4 (0x10000 bytes)
[INFO  fsx]  5 mapwrite 0x2d2c7 .. 0x3d2c6 (0x10000 bytes)
[INFO  fsx]  6 read     0x1fad5 .. 0x2fad4 (0x10000 bytes)
[INFO  fsx]  7 mapwrite 0x3e479 .. 0x3ffff ( 0x1b87 bytes)
[INFO  fsx]  8 mapwrite 0x3c227 .. 0x3ffff ( 0x3dd9 bytes)
[INFO  fsx]  9 mapread   0x8fc9 .. 0x18fc8 (0x10000 bytes)
[INFO  fsx] 10 truncate 0x40000 =>  0x938c
"
)]
// Equivalent to C's fsx -N 10 -S 4 -o 65536 -O -RW.  Disables mmapped read and
// write.
#[case::no_mmap(
    "-O -N10 -RW -S 4",
    "[INFO  fsx] Using seed 4
[DEBUG fsx]  1 skipping zero size read
[DEBUG fsx]  2 skipping zero size read
[INFO  fsx]  3 truncate     0x0 =>  0xa364
[INFO  fsx]  4 write    0x2d8b5 .. 0x3d8b4 (0x10000 bytes)
[INFO  fsx]  5 truncate 0x3d8b5 => 0x2d2c7
[INFO  fsx]  6 read      0x274e .. 0x1274d (0x10000 bytes)
[INFO  fsx]  7 read     0x181d8 .. 0x281d7 (0x10000 bytes)
[INFO  fsx]  8 truncate 0x2d2c7 => 0x3c227
[INFO  fsx]  9 read     0x27c93 .. 0x37c92 (0x10000 bytes)
[INFO  fsx] 10 read     0x194d5 .. 0x294d4 (0x10000 bytes)
"
)]
// Equivalent to C's fsx -N 10 -d -S 6 -o 65536 -O.  Includes both truncate
// down and truncate up.
#[case::truncate(
    "-O -N10 -S 6",
    "[INFO  fsx] Using seed 6
[INFO  fsx]  1 truncate     0x0 =>  0xb574
[INFO  fsx]  2 mapwrite 0x2e2ff .. 0x3e2fe (0x10000 bytes)
[INFO  fsx]  3 truncate 0x3e2ff => 0x1ede4
[INFO  fsx]  4 mapread   0x2054 .. 0x12053 (0x10000 bytes)
[INFO  fsx]  5 write    0x1baea .. 0x2bae9 (0x10000 bytes)
[INFO  fsx]  6 mapwrite 0x2d78e .. 0x3d78d (0x10000 bytes)
[INFO  fsx]  7 write     0x67c2 .. 0x167c1 (0x10000 bytes)
[INFO  fsx]  8 write    0x175c2 .. 0x275c1 (0x10000 bytes)
[INFO  fsx]  9 mapread   0x783e .. 0x1783d (0x10000 bytes)
[INFO  fsx] 10 write    0x2a14c .. 0x3a14b (0x10000 bytes)
"
)]
// Equivalent to C's fsx -b 100 -N 110 -S 4 -o 65536 -O. Uses "-b"
#[case::opnum(
    "-O -N 110 -b 100 -S 4",
    "[INFO  fsx] Using seed 4
[INFO  fsx] 100 truncate 0x352f6 => 0x3397b
[INFO  fsx] 101 mapread  0x32365 .. 0x3397a ( 0x1616 bytes)
[INFO  fsx] 102 mapread  0x25174 .. 0x3397a ( 0xe807 bytes)
[INFO  fsx] 103 write    0x22d05 .. 0x32d04 (0x10000 bytes)
[INFO  fsx] 104 mapwrite 0x1c5a7 .. 0x2c5a6 (0x10000 bytes)
[INFO  fsx] 105 read     0x255da .. 0x3397a ( 0xe3a1 bytes)
[INFO  fsx] 106 mapread   0x9c1f .. 0x19c1e (0x10000 bytes)
[INFO  fsx] 107 truncate 0x3397b =>  0xe9a3
[INFO  fsx] 108 read      0xb9f3 ..  0xe9a2 ( 0x2fb0 bytes)
[INFO  fsx] 109 mapwrite 0x130ff .. 0x230fe (0x10000 bytes)
[INFO  fsx] 110 read     0x1929a .. 0x230fe ( 0x9e65 bytes)
"
)]
// Equivalent to C's fsx -N 2 -S 13 -o 65536 -O -c 2
// Exercises closeopen
#[case::closeopen(
    "-O -N 2 -S 13 -c 2",
    "[INFO  fsx] Using seed 13
[INFO  fsx] 1 write    0x271a0 .. 0x3719f (0x10000 bytes)
[INFO  fsx] 1 close/open
[INFO  fsx] 2 truncate 0x371a0 =>  0x7541
"
)]
// Equivalent to C's fsx -N 2 -S 20
// Uses random oplen
#[case::baseline(
    "-N10 -S 20",
    "[INFO  fsx] Using seed 20
[DEBUG fsx]  1 skipping zero size read
[INFO  fsx]  2 truncate     0x0 =>  0x4d6f
[INFO  fsx]  3 mapwrite 0x2bcac .. 0x33220 ( 0x7575 bytes)
[INFO  fsx]  4 truncate 0x33221 => 0x2d073
[INFO  fsx]  5 truncate 0x2d073 => 0x152f2
[INFO  fsx]  6 mapwrite 0x156b0 .. 0x17b33 ( 0x2484 bytes)
[INFO  fsx]  7 write    0x1814f .. 0x243a6 ( 0xc258 bytes)
[INFO  fsx]  8 read     0x21e66 .. 0x243a6 ( 0x2541 bytes)
[INFO  fsx]  9 mapwrite  0xe0b7 ..  0xe2a2 (  0x1ec bytes)
[INFO  fsx] 10 read     0x1d288 .. 0x23b30 ( 0x68a9 bytes)
"
)]
// Equivalent to C's fsx -N 10 -S 20 -U
// Exercises -U, though that doesn't change the output
#[case::nomsyncafterwrite(
    "-N10 -S20 -U",
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
[INFO  fsx]  1 mapwrite  0x1473 ..  0x2119 ( 0xca7 bytes)
[INFO  fsx]  2 mapread   0x1a31 ..  0x1b6f ( 0x13f bytes)
[INFO  fsx]  3 truncate  0x211a => 0x2e8c2
[INFO  fsx]  4 mapwrite 0x1cb4a .. 0x1d774 ( 0xc2b bytes)
[INFO  fsx]  5 mapread  0x2c35d .. 0x2d351 ( 0xff5 bytes)
[INFO  fsx]  6 read     0x12925 .. 0x137f6 ( 0xed2 bytes)
[INFO  fsx]  7 mapwrite  0x968c ..  0xa469 ( 0xdde bytes)
[INFO  fsx]  8 truncate 0x2e8c2 =>  0xadb5
[INFO  fsx]  9 mapread   0x4afb ..  0x5835 ( 0xd3b bytes)
[INFO  fsx] 10 truncate  0xadb5 => 0x30b73
"
)]
// Equivalent to C's fsx -N 10 -S 40 -l 1048576
// Exercises -l
#[case::flen(
    "-N 10 -S 56 -l 1048576",
    "[INFO  fsx] Using seed 56
[DEBUG fsx]  1 skipping zero size read
[INFO  fsx]  2 write     0xdf810 ..  0xe191b ( 0x210c bytes)
[INFO  fsx]  3 truncate  0xe191c =>  0x93db7
[INFO  fsx]  4 truncate  0x93db7 =>  0x28813
[INFO  fsx]  5 write     0x2f984 ..  0x30463 (  0xae0 bytes)
[INFO  fsx]  6 read      0x2b74e ..  0x30463 ( 0x4d16 bytes)
[INFO  fsx]  7 truncate  0x30464 =>  0x3ad48
[INFO  fsx]  8 mapwrite  0xff116 ..  0xfffff (  0xeea bytes)
[INFO  fsx]  9 truncate 0x100000 =>  0x72f45
[INFO  fsx] 10 mapwrite  0x9a519 ..  0xa2c56 ( 0x873e bytes)
"
)]
// Equivalent to C's fsx -N 10 -S 42 -N 10 -i 2
// Exercises -i
#[case::inval(
    "-N 10 -S 42 -i 2",
    "[INFO  fsx] Using seed 42
[INFO  fsx]  1 truncate     0x0 => 0x34d0d
[INFO  fsx]  2 mapwrite 0x340d8 .. 0x381ce ( 0x40f7 bytes)
[INFO  fsx]  3 write    0x218ba .. 0x29715 ( 0x7e5c bytes)
[INFO  fsx]  4 truncate 0x381cf => 0x16407
[INFO  fsx]  5 mapread   0xb94e ..  0xe951 ( 0x3004 bytes)
[INFO  fsx]  6 mapwrite  0xd816 .. 0x1793a ( 0xa125 bytes)
[INFO  fsx]  7 read      0x8321 ..  0xdb30 ( 0x5810 bytes)
[INFO  fsx]  8 read      0xd0da .. 0x11a18 ( 0x493f bytes)
[INFO  fsx]  9 mapread  0x12f21 .. 0x1793a ( 0x4a1a bytes)
[INFO  fsx]  9 msync(MS_INVALIDATE)
[INFO  fsx] 10 mapread   0x26b5 ..  0x2e29 (  0x775 bytes)
"
)]
// Equivalent to C's fsx -N 2 -i 1 -S 11
// https://github.com/asomers/fsx-rs/issues/13
#[case::mmap_underflow(
    "-N 2 -S 11 -i 1",
    "[INFO  fsx] Using seed 11
[DEBUG fsx] 1 skipping zero size read
[DEBUG fsx] 2 skipping zero size read
[DEBUG fsx] 2 skipping invalidate of zero-length file
"
)]
// Equivalent to C's fsx -N 10 -S 45 -r 4096
// Exercises -r
#[case::readbdy(
    "-N 10 -S 45 -r 4096",
    "[INFO  fsx] Using seed 45
[DEBUG fsx]  1 skipping zero size read
[INFO  fsx]  2 mapwrite 0x3e972 .. 0x3ffff ( 0x168e bytes)
[INFO  fsx]  3 truncate 0x40000 => 0x2e370
[INFO  fsx]  4 write    0x12edc .. 0x1975d ( 0x6882 bytes)
[INFO  fsx]  5 read     0x17000 .. 0x23ea2 ( 0xcea3 bytes)
[INFO  fsx]  6 read      0x2000 ..  0xc729 ( 0xa72a bytes)
[INFO  fsx]  7 read     0x11000 .. 0x1de97 ( 0xce98 bytes)
[INFO  fsx]  8 mapwrite 0x3e69a .. 0x3ffff ( 0x1966 bytes)
[INFO  fsx]  9 write    0x3ab1d .. 0x3d337 ( 0x281b bytes)
[INFO  fsx] 10 truncate 0x40000 => 0x10d5d
"
)]
// Equivalent to C's fsx -N 10 -S 46 -w 4096
// Exercises -w
#[case::writebdy(
    "-N 10 -S 46 -w 4096",
    "[INFO  fsx] Using seed 46
[INFO  fsx]  1 write    0x36000 .. 0x3f099 ( 0x909a bytes)
[INFO  fsx]  2 truncate 0x3f09a => 0x34b8b
[INFO  fsx]  3 mapread  0x2f0af .. 0x3160d ( 0x255f bytes)
[INFO  fsx]  4 truncate 0x34b8b => 0x3f064
[INFO  fsx]  5 write    0x26000 .. 0x2ca25 ( 0x6a26 bytes)
[INFO  fsx]  6 mapread  0x15644 .. 0x19e30 ( 0x47ed bytes)
[INFO  fsx]  7 read     0x1beaa .. 0x20da5 ( 0x4efc bytes)
[INFO  fsx]  8 write    0x35000 .. 0x3f1d3 ( 0xa1d4 bytes)
[INFO  fsx]  9 mapwrite 0x2b000 .. 0x357ae ( 0xa7af bytes)
[INFO  fsx] 10 truncate 0x3f1d4 =>  0xdd2f
"
)]
// Equivalent to C's fsx -N 4 -t 4096 -S 51
// Exercises -t
#[case::truncbdy(
    "-N 4 -S 52 -t 4096",
    "[INFO  fsx] Using seed 52
[INFO  fsx] 1 truncate     0x0 => 0x16000
[INFO  fsx] 2 mapwrite 0x343fd .. 0x3d93f ( 0x9543 bytes)
[INFO  fsx] 3 truncate 0x3d940 => 0x23000
[INFO  fsx] 4 read     0x19850 .. 0x22fff ( 0x97b0 bytes)
"
)]
// Equivalent to C's fsx -N 10 -S 60 -m 32768:65536
// Exercises -m
#[case::monitor(
    "-N 10 -S 61 -m 32768:65536",
    "[INFO  fsx] Using seed 61
[DEBUG fsx]  1 skipping zero size read
[INFO  fsx]  2 write    0x3e0c9 .. 0x3ffff ( 0x1f37 bytes)
[INFO  fsx]  3 read     0x1001b .. 0x1e0b6 ( 0xe09c bytes)
[WARN  fsx]  4 read      0x5e79 ..  0xe411 ( 0x8599 bytes)
[INFO  fsx]  5 mapwrite 0x1a4a6 .. 0x29a5c ( 0xf5b7 bytes)
[WARN  fsx]  6 mapwrite  0x7f07 ..  0xa49f ( 0x2599 bytes)
[INFO  fsx]  7 mapwrite 0x3d331 .. 0x3ffff ( 0x2ccf bytes)
[WARN  fsx]  8 write     0x3e8f ..  0xec39 ( 0xadab bytes)
[INFO  fsx]  9 read     0x2124f .. 0x2efde ( 0xdd90 bytes)
[INFO  fsx] 10 write    0x1425a .. 0x211d0 ( 0xcf77 bytes)
"
)]
// Equivalent to C's fsx -S 72 -L -N 10
// Exercises -B
#[case::blockmode(
    "-B -S 72 -N 10 -P /tmp",
    "[INFO  fsx] Using seed 72
[INFO  fsx]  1 read      0xd7e2b ..  0xdf129 ( 0x72ff bytes)
[INFO  fsx]  2 read      0x68997 ..  0x697f6 (  0xe60 bytes)
[INFO  fsx]  3 read      0xc0405 ..  0xcd716 ( 0xd312 bytes)
[INFO  fsx]  4 mapread   0x19f63 ..  0x1b0cb ( 0x1169 bytes)
[INFO  fsx]  5 mapread   0x162e4 ..  0x19055 ( 0x2d72 bytes)
[INFO  fsx]  6 read      0xe8886 ..  0xee99a ( 0x6115 bytes)
[INFO  fsx]  7 mapwrite  0x808bc ..  0x82901 ( 0x2046 bytes)
[INFO  fsx]  8 write     0x5044a ..  0x5f467 ( 0xf01e bytes)
[INFO  fsx]  9 mapread   0x84f30 ..  0x8678e ( 0x185f bytes)
[INFO  fsx] 10 mapwrite  0x30237 ..  0x3df1b ( 0xdce5 bytes)
"
)]
fn stability(#[case] args: &str, #[case] stderr: &str) {
    let mut tf = NamedTempFile::new().unwrap();

    if args.contains("-B") {
        // When using -B, must manually set file size before starting program
        // Set flen higher than default
        // https://github.com/asomers/fsx-rs/issues/13
        tf.as_file_mut().set_len(1048576).unwrap();
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
    assert_eq!(stderr, actual_stderr);
}

#[cfg_attr(not(target_os = "freebsd"), allow(unused))]
#[rstest]
fn miscompare() {
    let tf = NamedTempFile::new().unwrap();

    let cmd = Command::cargo_bin("fsx")
        .unwrap()
        .env("RUST_LOG", "debug")
        .args(["-N10", "-S7", "--inject", "4"])
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
            "[INFO  fsx] Using seed 7
[INFO  fsx]  1 mapwrite 0x1b166 .. 0x27bb9 ( 0xca54 bytes)
[INFO  fsx]  2 read     0x109fa .. 0x14186 ( 0x378d bytes)
[INFO  fsx]  3 truncate 0x27bba => 0x253bb
[INFO  fsx]  5 mapread  0x1dc4f .. 0x1df55 (  0x307 bytes)
[ERROR fsx] miscompare: offset= 0x1dc4f, size = 0x307
[ERROR fsx] OFFSET  GOOD  BAD  RANGE  
[ERROR fsx] 0x1dda8 0x04 0x01   0x1ae
[ERROR fsx] Step# (mod 256) for a misdirected write may be 1
[ERROR fsx] LOG DUMP
[ERROR fsx]  0 MAPWRITE 0x1b166 => 0x27bba ( 0xca54 bytes) HOLE
[ERROR fsx]  1 READ     0x109fa => 0x14187 ( 0x378d bytes)
[ERROR fsx]  2 TRUNCATE  DOWN from 0x27bba to 0x253bb
[ERROR fsx]  3 WRITE    0x1dda8 => 0x22444 ( 0x469c bytes)
[ERROR fsx]  4 MAPREAD  0x1dc4f => 0x1df56 (  0x307 bytes)
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
        .args(["-N2", "-S8", "--inject", "1", "-P"])
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

// https://github.com/asomers/fsx-rs/issues/20
#[test]
fn blockmode_zero() {
    let tf = NamedTempFile::new().unwrap();
    let artifacts_dir = TempDir::new().unwrap();

    let cmd = Command::cargo_bin("fsx")
        .unwrap()
        .env("RUST_LOG", "warn")
        .args(["-B", "-N2", "-S72", "-P"])
        .arg(artifacts_dir.path())
        .arg(tf.path())
        .assert()
        .failure();

    let actual_stderr = CString::new(cmd.get_output().stderr.clone())
        .unwrap()
        .into_string()
        .unwrap();
    assert_eq!(
        actual_stderr,
        "[ERROR fsx] ERROR: file length must be greater than zero
"
    );
}
