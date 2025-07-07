# Contributing to s3grep

Thank you for your interest in contributing to s3grep! We welcome contributions of all kinds, including bug reports, feature requests, documentation improvements, and code.

## How to Contribute

1. **Fork the repository** and create your branch from `main`.
2. **Describe your changes** clearly in your pull request.
3. **Ensure your code is formatted** with `cargo fmt` and passes `cargo clippy`.
4. **Add or update tests** as appropriate.
5. **Run all tests** with `cargo test`. For integration tests involving S3, use [Localstack](https://github.com/localstack/localstack).

## Code Style

- Follow Rust best practices and conventions.
- Document public functions and types with Rustdoc comments.
- Prefer small, focused pull requests.

## Running Tests

- Unit tests: `cargo test`
- Integration tests (S3): Start Localstack, then run `cargo test -- --ignored` (if integration tests are marked as ignored by default).

## Reporting Issues

- Please include as much detail as possible.
- Provide steps to reproduce, expected behavior, and actual behavior.

## Code of Conduct

Be respectful and inclusive. See [Contributor Covenant](https://www.contributor-covenant.org/) for a standard code of conduct.

---

Thank you for helping make s3grep better!