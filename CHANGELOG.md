# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.2](https://github.com/dacort/s3grep/compare/v0.1.1...v0.1.2) - 2025-07-08

### Fixed

- re-create client with bucket region when necessary

### Other

- fmt

## [0.1.1](https://github.com/dacort/s3grep/compare/v0.1.0...v0.1.1) - 2025-07-07

### Fixed

- print out error chain

## [0.1.0](https://github.com/dacort/s3grep/releases/tag/v0.1.0) - 2025-07-07

### Added

- publish 🚀

### Other

- Use the official setup-localstack action
- Detect binary files, fail early if binary and a match :)
- Add byte progress
- Clean up err output
- Add gzip support and lint
- Initial revision
