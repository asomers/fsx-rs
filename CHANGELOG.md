# Change Log

All notable changes to this project will be documented in this file.
This project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased] - ReleaseDate

### Changed

- The CLI has been completely changed by the addition of a config file.  Now,
  most settings are specified in the config file.  Also:

  * close/open and invalidate are specified like other operations, not as
    modifiers that happen after an operation.

  * Seed-for-seed backwards compatibility is broken.  So you can't run fsx-rs
    and expect the exact same sequence of operations as would've been produced
    by legacy fsx.

  * Operations are now selected with variable weights instead of equal
    probabilities.

  * It is no longer possible to specify separate alignment requirements for
    read, write, and truncate.  Now the same alignment requirement applies to
    all.

  ([#23](https://github.com/asomers/fsx-rs/pull/23))

## [0.1.1] - 2023-01-22
### Fixed

- Fixed crash when using `-B` on a file of 0 bytes.
  ([#21](https://github.com/asomers/fsx-rs/pull/21))

- Fixed crashes when using `-B` on files larger than 256 kB.
  ([#17](https://github.com/asomers/fsx-rs/pull/17))

- Fixed a `TryFromIntError` crash when using `-i` with high probabilities.
  ([#14](https://github.com/asomers/fsx-rs/pull/14))
