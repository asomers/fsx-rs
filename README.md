# File System eXerciser

Test file system I/O routines for correctness, with random input.

# Overview

`fsx` generates a pseudorandom sequence of file modifications and applies them
to the file system under test.  On every read, it verifies the expected data.
It's highly configurable, and the test sequence is 100% reproducible according
to a seed value.

# History

The first version of FSX was written in C at Apple Computer, beginning in 1998,
by Avadis Tevanian Jr.  It was imported into FreeBSD 5.0 by Jordan Hubbard, but
only as a development tool.  It was never installed as part of any release.
It's had occasional enhancements since then.

FSX was independently imported into Linux in 2001 by user robbiew, and has
occasionally merged in features from the FreeBSD version.

A tool by the same name was included in DEC Unix 4.0, but I don't think it
shared any code.

This version is a full rewrite in Rust, by Alan Somers.

# License

`fsx` is distributed until the Apple Public Source License version 2.0.  See
LICENSE for details.
