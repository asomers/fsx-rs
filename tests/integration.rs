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
[DEBUG fsx]  1 skipping zero size read
[INFO  fsx]  2 mapwrite 0x2d2c7 .. 0x3d2c6 (0x10000 bytes)
[INFO  fsx]  3 write    0x3e479 .. 0x3ffff ( 0x1b87 bytes)
[INFO  fsx]  4 mapwrite  0x8fc9 .. 0x18fc8 (0x10000 bytes)
[INFO  fsx]  5 write    0x27059 .. 0x37058 (0x10000 bytes)
[INFO  fsx]  6 truncate 0x40000 => 0x14ae8
[INFO  fsx]  7 write     0xed7f .. 0x1ed7e (0x10000 bytes)
[INFO  fsx]  8 mapwrite 0x24c9f .. 0x34c9e (0x10000 bytes)
[INFO  fsx]  9 mapread   0xf8c9 .. 0x1f8c8 (0x10000 bytes)
[INFO  fsx] 10 write    0x2e562 .. 0x3e561 (0x10000 bytes)
"
)]
// Equivalent to C's fsx -N 10 -S 4 -o 65536 -O -RW.  Disables mmapped read and
// write.
#[case::no_mmap(
    "-O -N10 -RW -S 4",
    "[INFO  fsx] Using seed 4
[DEBUG fsx]  1 skipping zero size read
[INFO  fsx]  2 write    0x2d2c7 .. 0x3d2c6 (0x10000 bytes)
[INFO  fsx]  3 write    0x3e479 .. 0x3ffff ( 0x1b87 bytes)
[INFO  fsx]  4 write     0x8fc9 .. 0x18fc8 (0x10000 bytes)
[INFO  fsx]  5 write    0x27059 .. 0x37058 (0x10000 bytes)
[INFO  fsx]  6 truncate 0x40000 => 0x14ae8
[INFO  fsx]  7 write     0xed7f .. 0x1ed7e (0x10000 bytes)
[INFO  fsx]  8 write    0x24c9f .. 0x34c9e (0x10000 bytes)
[INFO  fsx]  9 read      0xf8c9 .. 0x1f8c8 (0x10000 bytes)
[INFO  fsx] 10 write    0x2e562 .. 0x3e561 (0x10000 bytes)
"
)]
// Equivalent to C's fsx -N 10 -d -S 8 -o 65536 -O.  Includes both truncate
// down and truncate up.
#[case::truncate(
    "-O -N10 -S 8",
    "[INFO  fsx] Using seed 8
[DEBUG fsx]  1 skipping zero size read
[INFO  fsx]  2 truncate     0x0 =>  0x98e5
[INFO  fsx]  3 mapwrite 0x1458d .. 0x2458c (0x10000 bytes)
[INFO  fsx]  4 write    0x359cf .. 0x3ffff ( 0xa631 bytes)
[INFO  fsx]  5 mapread  0x2cffd .. 0x3cffc (0x10000 bytes)
[INFO  fsx]  6 truncate 0x40000 => 0x180a9
[INFO  fsx]  7 truncate 0x180a9 => 0x2c830
[INFO  fsx]  8 write    0x286a1 .. 0x386a0 (0x10000 bytes)
[INFO  fsx]  9 write     0x8db2 .. 0x18db1 (0x10000 bytes)
[INFO  fsx] 10 truncate 0x386a1 =>  0x4082
"
)]
// Equivalent to C's fsx -b 100 -N 110 -S 4 -o 65536 -O. Uses "-b"
#[case::opnum(
    "-O -N 110 -b 100 -S 4",
    "[INFO  fsx] Using seed 4
[DEBUG fsx]   1 skipping zero size read
[INFO  fsx] 100 read     0x13b7e .. 0x23b7d (0x10000 bytes)
[INFO  fsx] 101 mapwrite 0x1c1b0 .. 0x2c1af (0x10000 bytes)
[INFO  fsx] 102 mapwrite 0x2526a .. 0x35269 (0x10000 bytes)
[INFO  fsx] 103 write    0x22490 .. 0x3248f (0x10000 bytes)
[INFO  fsx] 104 mapread   0x7d30 .. 0x17d2f (0x10000 bytes)
[INFO  fsx] 105 write     0x4364 .. 0x14363 (0x10000 bytes)
[INFO  fsx] 106 mapwrite 0x10b74 .. 0x20b73 (0x10000 bytes)
[INFO  fsx] 107 mapwrite  0xa7af .. 0x1a7ae (0x10000 bytes)
[INFO  fsx] 108 write    0x3ec61 .. 0x3ffff ( 0x139f bytes)
[INFO  fsx] 109 truncate 0x40000 => 0x10227
[INFO  fsx] 110 mapwrite 0x37d30 .. 0x3ffff ( 0x82d0 bytes)
"
)]
// Equivalent to C's fsx -N 2 -S 13 -o 65536 -O -c 2
// Exercises closeopen
#[case::closeopen(
    "-O -N 2 -S 13 -c 2",
    "[INFO  fsx] Using seed 13
[DEBUG fsx] 1 skipping zero size read
[INFO  fsx] 1 close/open
[INFO  fsx] 2 truncate     0x0 => 0x2d851
[INFO  fsx] 2 close/open
"
)]
// Equivalent to C's fsx -N 2 -S 20
// Uses random oplen
#[case::baseline(
    "-N10 -S 20",
    "[INFO  fsx] Using seed 20
[DEBUG fsx]  1 skipping zero size read
[DEBUG fsx]  2 skipping zero size read
[INFO  fsx]  3 truncate     0x0 => 0x17e11
[INFO  fsx]  4 read      0xee64 .. 0x17e10 ( 0x8fad bytes)
[INFO  fsx]  5 write    0x2807b .. 0x2950b ( 0x1491 bytes)
[INFO  fsx]  6 read      0x6d83 .. 0x14c4d ( 0xdecb bytes)
[INFO  fsx]  7 read     0x1fc24 .. 0x280e9 ( 0x84c6 bytes)
[INFO  fsx]  8 read     0x232fb .. 0x2950b ( 0x6211 bytes)
[INFO  fsx]  9 mapwrite  0xfee2 .. 0x17999 ( 0x7ab8 bytes)
[INFO  fsx] 10 mapread   0xdaa5 .. 0x1b222 ( 0xd77e bytes)
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
[INFO  fsx]  1 mapwrite 0x11ccb .. 0x12aae ( 0xde4 bytes)
[INFO  fsx]  2 mapread   0x9e0c ..  0xaacd ( 0xcc2 bytes)
[INFO  fsx]  3 mapread   0xdfa5 ..  0xee76 ( 0xed2 bytes)
[INFO  fsx]  4 truncate 0x12aaf => 0x371b7
[INFO  fsx]  5 mapread  0x11f1d .. 0x12016 (  0xfa bytes)
[INFO  fsx]  6 mapwrite 0x29dff .. 0x2ac32 ( 0xe34 bytes)
[INFO  fsx]  7 truncate 0x371b7 => 0x2467f
[INFO  fsx]  8 truncate 0x2467f =>  0xfbef
[INFO  fsx]  9 mapwrite 0x144e6 .. 0x1471a ( 0x235 bytes)
[INFO  fsx] 10 write     0xddf0 ..  0xecca ( 0xedb bytes)
"
)]
// Equivalent to C's fsx -N 10 -S 40 -l 1048576
// Exercises -l
#[case::flen(
    "-N 10 -S 56 -l 1048576",
    "[INFO  fsx] Using seed 56
[DEBUG fsx]  1 skipping zero size read
[INFO  fsx]  2 mapwrite  0x3bbed ..  0x4a6be ( 0xead2 bytes)
[INFO  fsx]  3 mapread   0x22790 ..  0x2c447 ( 0x9cb8 bytes)
[INFO  fsx]  4 mapwrite  0xff116 ..  0xfffff (  0xeea bytes)
[INFO  fsx]  5 write     0x9a519 ..  0xa2c56 ( 0x873e bytes)
[INFO  fsx]  6 write     0x4f14b ..  0x5b085 ( 0xbf3b bytes)
[INFO  fsx]  7 read      0xdba6d ..  0xe309d ( 0x7631 bytes)
[INFO  fsx]  8 truncate 0x100000 =>  0xb312d
[INFO  fsx]  9 write     0xb0d6d ..  0xbacf7 ( 0x9f8b bytes)
[INFO  fsx] 10 write     0x40c70 ..  0x44527 ( 0x38b8 bytes)
"
)]
// Equivalent to C's fsx -N 10 -S 42 -N 10 -i 2
// Exercises -i
#[case::inval(
    "-N 10 -S 42 -i 2",
    "[INFO  fsx] Using seed 42
[INFO  fsx]  1 write    0x218ba .. 0x29715 ( 0x7e5c bytes)
[INFO  fsx]  1 msync(MS_INVALIDATE)
[INFO  fsx]  2 mapwrite  0xd816 .. 0x1793a ( 0xa125 bytes)
[INFO  fsx]  3 truncate 0x29716 => 0x1946c
[INFO  fsx]  3 msync(MS_INVALIDATE)
[INFO  fsx]  4 mapread  0x14eb4 .. 0x1946b ( 0x45b8 bytes)
[INFO  fsx]  4 msync(MS_INVALIDATE)
[INFO  fsx]  5 mapwrite 0x2e4c0 .. 0x2f4e4 ( 0x1025 bytes)
[INFO  fsx]  5 msync(MS_INVALIDATE)
[INFO  fsx]  6 read     0x21d3a .. 0x2d148 ( 0xb40f bytes)
[INFO  fsx]  7 mapwrite 0x38cf5 .. 0x3f69d ( 0x69a9 bytes)
[INFO  fsx]  8 write    0x131dc .. 0x1710d ( 0x3f32 bytes)
[INFO  fsx]  9 read     0x300bd .. 0x3b947 ( 0xb88b bytes)
[INFO  fsx]  9 msync(MS_INVALIDATE)
[INFO  fsx] 10 mapwrite 0x19182 .. 0x2868b ( 0xf50a bytes)
"
)]
// Equivalent to C's fsx -N 1 -i 1 -S 10
// https://github.com/asomers/fsx-rs/issues/13
#[case::mmap_underflow(
    "-N 1 -S 10 -i 1",
    "[INFO  fsx] Using seed 10
[DEBUG fsx] 1 skipping zero size read
[DEBUG fsx] 1 skipping invalidate of zero-length file
"
)]
// Equivalent to C's fsx -N 10 -S 46 -r 4096
// Exercises -r
#[case::readbdy(
    "-N 10 -S 46 -r 4096",
    "[INFO  fsx] Using seed 46
[INFO  fsx]  1 truncate     0x0 =>  0xb8fa
[INFO  fsx]  2 mapread   0xb000 ..  0xb16c (  0x16d bytes)
[INFO  fsx]  3 write    0x1edb2 .. 0x1f7d5 (  0xa24 bytes)
[INFO  fsx]  4 mapread  0x1a000 .. 0x1eeda ( 0x4edb bytes)
[INFO  fsx]  5 mapwrite 0x2ba50 .. 0x361fe ( 0xa7af bytes)
[INFO  fsx]  6 read     0x12000 .. 0x1bcb2 ( 0x9cb3 bytes)
[INFO  fsx]  7 mapread  0x34000 .. 0x358a5 ( 0x18a6 bytes)
[INFO  fsx]  8 mapwrite 0x3128a .. 0x38fd9 ( 0x7d50 bytes)
[INFO  fsx]  9 truncate 0x38fda =>  0x5ce7
[INFO  fsx] 10 mapread   0x2000 ..  0x366f ( 0x1670 bytes)
"
)]
// Equivalent to C's fsx -N 10 -S 46 -w 4096
// Exercises -w
#[case::writebdy(
    "-N 10 -S 46 -w 4096",
    "[INFO  fsx] Using seed 46
[INFO  fsx]  1 truncate     0x0 =>  0xb8fa
[INFO  fsx]  2 mapread   0xb78d ..  0xb8f9 (  0x16d bytes)
[INFO  fsx]  3 write    0x1e000 .. 0x1ea23 (  0xa24 bytes)
[INFO  fsx]  4 mapread  0x1c25b .. 0x1ea23 ( 0x27c9 bytes)
[INFO  fsx]  5 mapwrite 0x2b000 .. 0x357ae ( 0xa7af bytes)
[INFO  fsx]  6 read      0xb938 .. 0x155ea ( 0x9cb3 bytes)
[INFO  fsx]  7 mapread  0x1eefb .. 0x241c3 ( 0x52c9 bytes)
[INFO  fsx]  8 mapwrite 0x31000 .. 0x38d4f ( 0x7d50 bytes)
[INFO  fsx]  9 truncate 0x38d50 =>  0x5ce7
[INFO  fsx] 10 mapread   0x2a16 ..  0x4085 ( 0x1670 bytes)
"
)]
// Equivalent to C's fsx -N 4 -t 4096 -S 53
// Exercises -t
#[case::truncbdy(
    "-N 4 -S 53 -t 4096",
    "[INFO  fsx] Using seed 53
[INFO  fsx] 1 truncate     0x0 => 0x3e000
[INFO  fsx] 2 truncate 0x3e000 =>  0xa000
[INFO  fsx] 3 mapread   0x9290 ..  0x9fff (  0xd70 bytes)
[INFO  fsx] 4 write     0x9bb0 .. 0x12ed5 ( 0x9326 bytes)
"
)]
// Equivalent to C's fsx -N 10 -S 68 -m 32768:65536
// Exercises -m
#[case::monitor(
    "-N 10 -S 68 -m 32768:65536",
    "[INFO  fsx] Using seed 68
[WARN  fsx]  1 truncate     0x0 =>  0x5366
[INFO  fsx]  2 mapwrite 0x1d30c .. 0x21f07 ( 0x4bfc bytes)
[INFO  fsx]  3 truncate 0x21f08 => 0x1594d
[WARN  fsx]  4 read      0x20c5 ..  0xeff1 ( 0xcf2d bytes)
[INFO  fsx]  5 read     0x14d41 .. 0x1594c (  0xc0c bytes)
[INFO  fsx]  6 write    0x32422 .. 0x3ffff ( 0xdbde bytes)
[INFO  fsx]  7 mapread  0x1d40b .. 0x22c3c ( 0x5832 bytes)
[WARN  fsx]  8 write     0x7ddd .. 0x17366 ( 0xf58a bytes)
[WARN  fsx]  9 mapwrite  0xa8cd ..  0xbd84 ( 0x14b8 bytes)
[INFO  fsx] 10 truncate 0x40000 => 0x2a40d
"
)]
// Equivalent to C's fsx -S 72 -L -N 10
// Exercises -B
#[case::blockmode(
    "-B -S 72 -N 10 -P /tmp",
    "[INFO  fsx] Using seed 72
[INFO  fsx]  1 mapwrite  0xbca1b ..  0xbf152 ( 0x2738 bytes)
[INFO  fsx]  2 write     0xec146 ..  0xf7048 ( 0xaf03 bytes)
[INFO  fsx]  3 read      0xe1fbf ..  0xe8b46 ( 0x6b88 bytes)
[INFO  fsx]  4 mapwrite  0x5044a ..  0x5f467 ( 0xf01e bytes)
[INFO  fsx]  5 read      0x9e6be ..  0xaa93e ( 0xc281 bytes)
[INFO  fsx]  6 mapread   0x837ab ..  0x89af3 ( 0x6349 bytes)
[INFO  fsx]  7 mapwrite  0x4bdd9 ..  0x5997c ( 0xdba4 bytes)
[INFO  fsx]  8 mapwrite  0x6e962 ..  0x74089 ( 0x5728 bytes)
[INFO  fsx]  9 mapread   0x2bdec ..  0x2e593 ( 0x27a8 bytes)
[INFO  fsx] 10 mapwrite  0x9083e ..  0x9600c ( 0x57cf bytes)
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
        .args(["-N10", "-S9", "--inject", "3"])
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
            "[INFO  fsx] Using seed 9
[INFO  fsx]  1 truncate     0x0 => 0x186ea
[INFO  fsx]  2 write    0x24240 .. 0x2de98 ( 0x9c59 bytes)
[INFO  fsx]  4 read     0x17842 .. 0x24a0b ( 0xd1ca bytes)
[INFO  fsx]  5 read     0x25444 .. 0x2de98 ( 0x8a55 bytes)
[INFO  fsx]  6 write    0x13683 .. 0x1f3e0 ( 0xbd5e bytes)
[INFO  fsx]  7 read      0x8b82 ..  0xef2c ( 0x63ab bytes)
[ERROR fsx] miscompare: offset= 0x8b82, size = 0x63ab
[ERROR fsx] OFFSET  GOOD  BAD  RANGE  
[ERROR fsx]  0x8b82 0x03 0x00  0x6379
[ERROR fsx] Step# for the bad data is unknown; check HOLE and EXTEND ops
[ERROR fsx] LOG DUMP
[ERROR fsx]  0 TRUNCATE  UP   from     0x0 to 0x186ea
[ERROR fsx]  1 WRITE    0x24240 => 0x2de99 ( 0x9c59 bytes) HOLE
[ERROR fsx]  2 MAPWRITE  0x5f5f => 0x14930 ( 0xe9d1 bytes)
[ERROR fsx]  3 READ     0x17842 => 0x24a0c ( 0xd1ca bytes)
[ERROR fsx]  4 READ     0x25444 => 0x2de99 ( 0x8a55 bytes)
[ERROR fsx]  5 WRITE    0x13683 => 0x1f3e1 ( 0xbd5e bytes)
[ERROR fsx]  6 READ      0x8b82 =>  0xef2d ( 0x63ab bytes)
",
            actual_stderr
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
        .args(["-N2", "-S9", "--inject", "1", "-P"])
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
