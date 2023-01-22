# Change Log

All notable changes to this project will be documented in this file.
This project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased] - ReleaseDate
### Added
### Changed
### Fixed

- Fixed crash when using `-B` on a file of 0 bytes.
  ([#21](https://github.com/asomers/fsx-rs/pull/21))

- Fixed crashes when using `-B` on files larger than 256 kB.
  ([#17](https://github.com/asomers/fsx-rs/pull/17))

- Fixed a `TryFromIntError` crash when using `-i` with high probabilities.
  ([#14](https://github.com/asomers/fsx-rs/pull/14))
