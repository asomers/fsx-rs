// vim: tw=80
use std::{
    ffi::CString,
    fs,
    io::Write,
    process::Command
};

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
    "[opsize]
     min = 65536
     max = 65536",
    "-N10 -S 2",
"[INFO  fsx] Using seed 2
[INFO  fsx]  1 mapwrite 0x30c8c .. 0x3ffff ( 0xf374 bytes)
[INFO  fsx]  2 mapwrite  0x91fa .. 0x191f9 (0x10000 bytes)
[INFO  fsx]  3 write     0xfd9b .. 0x1fd9a (0x10000 bytes)
[INFO  fsx]  4 write    0x196ff .. 0x296fe (0x10000 bytes)
[INFO  fsx]  5 mapread  0x3c7f1 .. 0x3ffff ( 0x380f bytes)
[INFO  fsx]  6 mapwrite 0x11f63 .. 0x21f62 (0x10000 bytes)
[INFO  fsx]  7 mapread   0xc30c .. 0x1c30b (0x10000 bytes)
[INFO  fsx]  8 read     0x1c799 .. 0x2c798 (0x10000 bytes)
[INFO  fsx]  9 mapwrite 0x36fda .. 0x3ffff ( 0x9026 bytes)
[INFO  fsx] 10 read     0x2f131 .. 0x3f130 (0x10000 bytes)
"
)]
// Equivalent to C's fsx -N 10 -S 2 -o 65536 -O -RW.  Disables mmapped read and
// write.
#[case::no_mmap(
    "[opsize]
     min = 65536
     max = 65536
     [weights]
     mapread = 0
     mapwrite = 0
     read = 1
     write = 1
     truncate = 1",
    "-N10 -S 2",
    "[INFO  fsx] Using seed 2
[INFO  fsx]  1 truncate     0x0 =>  0xf651
[INFO  fsx]  2 truncate  0xf651 =>  0x91fa
[INFO  fsx]  3 write     0xfd9b .. 0x1fd9a (0x10000 bytes)
[INFO  fsx]  4 write    0x196ff .. 0x296fe (0x10000 bytes)
[INFO  fsx]  5 write    0x3c7f1 .. 0x3ffff ( 0x380f bytes)
[INFO  fsx]  6 truncate 0x40000 => 0x2eb10
[INFO  fsx]  7 truncate 0x2eb10 => 0x1c8ad
[INFO  fsx]  8 write    0x1c799 .. 0x2c798 (0x10000 bytes)
[INFO  fsx]  9 truncate 0x2c799 =>  0xf709
[INFO  fsx] 10 read      0x2524 ..  0xf708 ( 0xd1e5 bytes)
"
)]
// Equivalent to C's fsx -N 10 -d -S 9 -o 65536 -O.  Includes both truncate
// down and truncate up.
#[case::truncate(
    "[opsize]
     min = 65536
     max = 65536",
    "-N10 -S 9",
    "[INFO  fsx] Using seed 9
[DEBUG fsx]  1 skipping zero size read
[INFO  fsx]  2 truncate     0x0 => 0x2ecef
[INFO  fsx]  3 read     0x13844 .. 0x23843 (0x10000 bytes)
[INFO  fsx]  4 write    0x13683 .. 0x23682 (0x10000 bytes)
[INFO  fsx]  5 read      0xf61f .. 0x1f61e (0x10000 bytes)
[INFO  fsx]  6 mapread   0xa22d .. 0x1a22c (0x10000 bytes)
[INFO  fsx]  7 truncate 0x2ecef => 0x2309c
[INFO  fsx]  8 mapread  0x1c462 .. 0x2309b ( 0x6c3a bytes)
[INFO  fsx]  9 truncate 0x2309c => 0x2bcbb
[INFO  fsx] 10 mapwrite 0x1eb19 .. 0x2eb18 (0x10000 bytes)
"
)]
// Equivalent to C's fsx -b 100 -N 110 -S 4 -o 65536 -O. Uses "-b"
#[case::opnum(
    "[opsize]
     min = 65536
     max = 65536",
    "-N 110 -b 100 -S 4",
    "[INFO  fsx] Using seed 4
[DEBUG fsx]   1 skipping zero size read
[DEBUG fsx]   2 skipping zero size read
[DEBUG fsx]   3 skipping zero size read
[DEBUG fsx]   4 skipping zero size read
[DEBUG fsx]   5 skipping zero size read
[INFO  fsx] 100 mapread    0x6eb ..  0x15f3 (  0xf09 bytes)
[INFO  fsx] 101 truncate  0x15f4 => 0x2c8b1
[INFO  fsx] 102 read     0x14b48 .. 0x24b47 (0x10000 bytes)
[INFO  fsx] 103 read     0x17991 .. 0x27990 (0x10000 bytes)
[INFO  fsx] 104 mapread  0x1aedd .. 0x2aedc (0x10000 bytes)
[INFO  fsx] 105 mapread   0x63c6 .. 0x163c5 (0x10000 bytes)
[INFO  fsx] 106 mapwrite 0x25dcd .. 0x35dcc (0x10000 bytes)
[INFO  fsx] 107 read     0x1ebd7 .. 0x2ebd6 (0x10000 bytes)
[INFO  fsx] 108 truncate 0x35dcd => 0x3b1c0
[INFO  fsx] 109 mapwrite 0x3debb .. 0x3ffff ( 0x2145 bytes)
[INFO  fsx] 110 mapread  0x2041a .. 0x30419 (0x10000 bytes)
"
)]
// Equivalent to C's fsx -N 2 -S 13 -o 65536 -O -c 2
// Exercises closeopen
#[case::closeopen(
    "[opsize]
     min = 65536
     max = 65536
     [weights]
     close_open = 1
     mapread = 0
     mapwrite = 0
     read = 1
     write = 1
     truncate = 1",
    "-N 1 -S 13 -c 2",
    "[INFO  fsx] Using seed 13
[DEBUG fsx] 1 skipping zero size read
[INFO  fsx] 1 close/open
"
)]
// Equivalent to C's fsx -N 2 -S 20
// Uses random oplen
#[case::baseline(
    "",
    "-N10 -S 20",
    "[INFO  fsx] Using seed 20
[DEBUG fsx]  1 skipping zero size read
[DEBUG fsx]  2 skipping zero size read
[DEBUG fsx]  3 skipping zero size read
[DEBUG fsx]  4 skipping zero size read
[INFO  fsx]  5 write    0x2464d .. 0x24dbc (  0x770 bytes)
[INFO  fsx]  6 mapwrite 0x18775 .. 0x2219c ( 0x9a28 bytes)
[INFO  fsx]  7 truncate 0x24dbd => 0x3738b
[INFO  fsx]  8 mapread  0x1e1e8 .. 0x2c0b6 ( 0xdecf bytes)
[INFO  fsx]  9 mapwrite 0x34dc0 .. 0x372c2 ( 0x2503 bytes)
[INFO  fsx] 10 truncate 0x3738b => 0x3694a
"
)]
// Equivalent to C's fsx -N 10 -S 20 -U
// Exercises -U, though that doesn't change the output
#[case::nomsyncafterwrite(
    "nomsyncafterwrite = true",
    "-N10 -S20",
    "[INFO  fsx] Using seed 20
[DEBUG fsx]  1 skipping zero size read
[DEBUG fsx]  2 skipping zero size read
[DEBUG fsx]  3 skipping zero size read
[DEBUG fsx]  4 skipping zero size read
[INFO  fsx]  5 write    0x2464d .. 0x24dbc (  0x770 bytes)
[INFO  fsx]  6 mapwrite 0x18775 .. 0x2219c ( 0x9a28 bytes)
[INFO  fsx]  7 truncate 0x24dbd => 0x3738b
[INFO  fsx]  8 mapread  0x1e1e8 .. 0x2c0b6 ( 0xdecf bytes)
[INFO  fsx]  9 mapwrite 0x34dc0 .. 0x372c2 ( 0x2503 bytes)
[INFO  fsx] 10 truncate 0x3738b => 0x3694a
"
)]
// Equivalent to C's fsx -N 10 -S 30 -o 4096
// Exercises -o
#[case::oplen(
    "[opsize]
     min = 0
     max = 4096",
    "-N 10 -S 30",
    "[INFO  fsx] Using seed 30
[INFO  fsx]  1 mapwrite 0x1bcd6 .. 0x1c41f ( 0x74a bytes)
[INFO  fsx]  2 mapread  0x165d3 .. 0x16d91 ( 0x7bf bytes)
[INFO  fsx]  3 truncate 0x1c420 => 0x16494
[INFO  fsx]  4 write    0x2ca6f .. 0x2cd25 ( 0x2b7 bytes)
[INFO  fsx]  5 truncate 0x2cd26 => 0x144e6
[INFO  fsx]  6 write     0x49f3 ..  0x4b8a ( 0x198 bytes)
[INFO  fsx]  7 read      0x371f ..  0x4508 ( 0xdea bytes)
[INFO  fsx]  8 mapread   0xddab ..  0xddf2 (  0x48 bytes)
[INFO  fsx]  9 read      0x5e29 ..  0x6229 ( 0x401 bytes)
[INFO  fsx] 10 truncate 0x144e6 => 0x101a7
"
)]
// Equivalent to C's fsx -N 10 -S 50 -l 1048576
// Exercises -l
#[case::flen(
    "flen = 1048576",
    "-N 10 -S 50",
    "[INFO  fsx] Using seed 50
[INFO  fsx]  1 truncate      0x0 =>  0x890fd
[INFO  fsx]  2 mapwrite  0xf9c49 ..  0xfffff ( 0x63b7 bytes)
[INFO  fsx]  3 mapread   0x9331b ..  0xa1c68 ( 0xe94e bytes)
[INFO  fsx]  4 write     0x40b9e ..  0x4bf4b ( 0xb3ae bytes)
[INFO  fsx]  5 mapwrite  0x358c0 ..  0x38a48 ( 0x3189 bytes)
[INFO  fsx]  6 truncate 0x100000 =>  0x6ebc5
[INFO  fsx]  7 read      0x2d8ae ..  0x31a83 ( 0x41d6 bytes)
[INFO  fsx]  8 read      0x676b2 ..  0x6d9a2 ( 0x62f1 bytes)
[INFO  fsx]  9 mapwrite  0xfccb9 ..  0xfffff ( 0x3347 bytes)
[INFO  fsx] 10 mapread   0x60d51 ..  0x60e61 (  0x111 bytes)
"
)]
// Equivalent to C's fsx -N 10 -S 42 -N 10 -i 2
// Exercises -i
#[case::inval(
    "",
    "-N 10 -S 42 -i 2",
    "[INFO  fsx] Using seed 42
[DEBUG fsx]  1 skipping zero size read
[DEBUG fsx]  1 skipping invalidate of zero-length file
[INFO  fsx]  2 truncate     0x0 =>  0x150b
[INFO  fsx]  2 msync(MS_INVALIDATE)
[INFO  fsx]  3 mapread    0xb89 ..  0x150a (  0x982 bytes)
[INFO  fsx]  3 msync(MS_INVALIDATE)
[INFO  fsx]  4 read       0x670 ..  0x150a (  0xe9b bytes)
[INFO  fsx]  4 msync(MS_INVALIDATE)
[INFO  fsx]  5 truncate  0x150b => 0x131dc
[INFO  fsx]  6 write    0x1bb8e .. 0x286a9 ( 0xcb1c bytes)
[INFO  fsx]  7 mapread  0x102ce .. 0x1bab7 ( 0xb7ea bytes)
[INFO  fsx]  7 msync(MS_INVALIDATE)
[INFO  fsx]  8 read      0x72a3 ..  0xe7f0 ( 0x754e bytes)
[INFO  fsx]  8 msync(MS_INVALIDATE)
[INFO  fsx]  9 mapread  0x1439e .. 0x21d9e ( 0xda01 bytes)
[INFO  fsx]  9 msync(MS_INVALIDATE)
[INFO  fsx] 10 write    0x184d3 .. 0x23091 ( 0xabbf bytes)
[INFO  fsx] 10 msync(MS_INVALIDATE)
"
)]
// Equivalent to C's fsx -N 1 -i 1 -S 10
// https://github.com/asomers/fsx-rs/issues/13
#[case::mmap_underflow(
    "",
    "-N 1 -S 10 -i 1",
    "[INFO  fsx] Using seed 10
[DEBUG fsx] 1 skipping zero size read
[DEBUG fsx] 1 skipping invalidate of zero-length file
"
)]
// Equivalent to C's fsx -N 10 -S 46 -r 4096
// Exercises -r
#[case::align(
    "[opsize]
    align = 4096",
    "-N 10 -S 46",
    "[INFO  fsx] Using seed 46
[INFO  fsx]  1 mapwrite 0x2e000 .. 0x31fff ( 0x4000 bytes)
[INFO  fsx]  2 write    0x18000 .. 0x1cfff ( 0x5000 bytes)
[INFO  fsx]  3 read     0x1e000 .. 0x27fff ( 0xa000 bytes)
[INFO  fsx]  4 mapread  0x23000 .. 0x2afff ( 0x8000 bytes)
[INFO  fsx]  5 mapwrite 0x13000 .. 0x1cfff ( 0xa000 bytes)
[INFO  fsx]  6 truncate 0x32000 => 0x1b6d8
[INFO  fsx]  7 read     0x16000 .. 0x17fff ( 0x2000 bytes)
[INFO  fsx]  8 read     0x11000 .. 0x12fff ( 0x2000 bytes)
[INFO  fsx]  9 mapwrite     0x0 ..  0x9fff ( 0xa000 bytes)
[INFO  fsx] 10 mapread   0x8000 ..  0x8fff ( 0x1000 bytes)
"
)]
// Equivalent to C's fsx -N 10 -S 68 -m 32768:65536
// Exercises -m
#[case::monitor(
    "",
    "-N 10 -S 68 -m 32768:65536",
    "[INFO  fsx] Using seed 68
[DEBUG fsx]  1 skipping zero size read
[DEBUG fsx]  2 skipping zero size read
[DEBUG fsx]  3 skipping zero size read
[INFO  fsx]  4 truncate     0x0 => 0x39b5e
[WARN  fsx]  5 read      0xe83d .. 0x18461 ( 0x9c25 bytes)
[WARN  fsx]  6 truncate 0x39b5e =>  0xb88b
[WARN  fsx]  7 read      0x8d51 ..  0xb88a ( 0x2b3a bytes)
[INFO  fsx]  8 read      0x4c42 ..  0x738a ( 0x2749 bytes)
[WARN  fsx]  9 read      0x9017 ..  0xb88a ( 0x2874 bytes)
[INFO  fsx] 10 truncate  0xb88b => 0x3e3ea
"
)]
// Equivalent to C's fsx -S 72 -L -N 10
// Exercises -B
#[case::blockmode(
    "blockmode = true
    [weights]
    truncate = 0",
    "-S 72 -N 10 -P /tmp",
    "[INFO  fsx] Using seed 72
[INFO  fsx]  1 write     0xc0405 ..  0xc2ac7 ( 0x26c3 bytes)
[INFO  fsx]  2 mapwrite  0x77eb8 ..  0x78c78 (  0xdc1 bytes)
[INFO  fsx]  3 read      0x323d0 ..  0x37cd9 ( 0x590a bytes)
[INFO  fsx]  4 read      0xb8dbb ..  0xc2342 ( 0x9588 bytes)
[INFO  fsx]  5 read      0x45efa ..  0x4d083 ( 0x718a bytes)
[INFO  fsx]  6 mapwrite  0x926be ..  0xa06d8 ( 0xe01b bytes)
[INFO  fsx]  7 mapwrite  0x753bd ..  0x7605e (  0xca2 bytes)
[INFO  fsx]  8 write     0xc3bef ..  0xc5cfe ( 0x2110 bytes)
[INFO  fsx]  9 mapread   0x7296b ..  0x7b6f8 ( 0x8d8e bytes)
[INFO  fsx] 10 read      0x8d39a ..  0x9b122 ( 0xdd89 bytes)
"
)]
fn stability(#[case] conf: &str, #[case] args: &str, #[case] stderr: &str) {
    let mut cf = NamedTempFile::new().unwrap();
    cf.write_all(conf.as_bytes()).unwrap();

    let mut tf = NamedTempFile::new().unwrap();

    if conf.contains("blockmode = true") {
        // When using -B, must manually set file size before starting program
        // Set flen higher than default
        // https://github.com/asomers/fsx-rs/issues/13
        tf.as_file_mut().set_len(1048576).unwrap();
    }

    let cmd = Command::cargo_bin("fsx")
        .unwrap()
        .env("RUST_LOG", "debug")
        .args(args.split_ascii_whitespace())
        .arg("-f")
        .arg(cf.path())
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
        .args(["-N10", "-S9", "--inject", "5"])
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
[DEBUG fsx]  1 skipping zero size read
[INFO  fsx]  2 truncate     0x0 => 0x1d670
[INFO  fsx]  3 mapread   0xae57 ..  0xe34e ( 0x34f8 bytes)
[INFO  fsx]  4 read      0x8162 ..  0xa251 ( 0x20f0 bytes)
[INFO  fsx]  6 mapread   0x3554 .. 0x12c46 ( 0xf6f3 bytes)
[ERROR fsx] miscompare: offset= 0x3554, size = 0xf6f3
[ERROR fsx] OFFSET  GOOD  BAD  RANGE  
[ERROR fsx] 0x10d4d 0x62 0x00  0x1ee4
[ERROR fsx] Step# for the bad data is unknown; check HOLE and EXTEND ops
[ERROR fsx] LOG DUMP
[ERROR fsx]  0 SKIPPED  (mapread)
[ERROR fsx]  1 TRUNCATE  UP   from     0x0 to 0x1d670
[ERROR fsx]  2 MAPREAD   0xae57 =>  0xe34f ( 0x34f8 bytes)
[ERROR fsx]  3 READ      0x8162 =>  0xa252 ( 0x20f0 bytes)
[ERROR fsx]  4 WRITE    0x10d4d => 0x14869 ( 0x3b1c bytes)
[ERROR fsx]  5 MAPREAD   0x3554 => 0x12c47 ( 0xf6f3 bytes)
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
        .args(["-N2", "-S11", "--inject", "1", "-P"])
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
    let mut cf = NamedTempFile::new().unwrap();
    cf.write_all(
    b"blockmode = true
[weights]
truncate = 0").unwrap();

    let tf = NamedTempFile::new().unwrap();
    let artifacts_dir = TempDir::new().unwrap();

    let cmd = Command::cargo_bin("fsx")
        .unwrap()
        .env("RUST_LOG", "warn")
        .args(["-N2", "-S72", "-P"])
        .arg(artifacts_dir.path())
        .arg(tf.path())
        .arg("-f")
        .arg(cf.path())
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
