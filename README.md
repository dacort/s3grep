# s3grep - Fast Parallel grep for S3

I use `grep` almost daily, but often deal with unstructured content and logs on S3...so I wrote an easy way to `grep` S3!

[![CI](https://github.com/dacort/s3grep/actions/workflows/ci.yml/badge.svg)](https://github.com/dacort/s3grep/actions/workflows/ci.yml)

`s3grep` is a parallel CLI tool for searching logs and unstructured content in Amazon S3 buckets. It supports .gz decompression, progress bars, and robust error handling—making it ideal for cloud-native log analysis.

---

## Features

- Parallel, concurrent search across S3 objects
- Supports plain text and `.gz` compressed files
- Progress bars for files and bytes processed
- Case-sensitive and insensitive search
- Line number output option
- Graceful handling of binary files and decompression errors
- Colorized match highlighting

---

## Installation

### crates.io

```bash
cargo install s3grep
```

---

## Usage

```sh
s3grep --pattern "ERROR" --bucket my-logs-bucket --prefix logs/ --concurrent-tasks 16
```

### CLI Options

| Flag                | Description                                 |
|---------------------|---------------------------------------------|
| `-p`, `--pattern`   | Search pattern (required)                   |
| `-b`, `--bucket`    | S3 bucket name (required)                   |
| `-z`, `--prefix`    | S3 prefix to search in (default: "")        |
| `-c`, `--concurrent-tasks` | Number of concurrent tasks (default: 8) |
| `-i`, `--case-sensitive`   | Case sensitive search                 |
| `-q`, `--quiet`     | Hide progress bar                           |
| `-n`, `--line-number` | Show line numbers in output               |

---

## Example

```sh
s3grep --pattern "timeout" --bucket my-bucket --prefix logs/2025/06/ --concurrent-tasks 12 --line-number
```

---

## Testing

Integration tests use [Localstack](https://github.com/localstack/localstack) to mock S3. See [CONTRIBUTING.md](CONTRIBUTING.md) for details.

---

## Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

---

## License

This project is licensed under the [MIT License](LICENSE).

---

## Acknowledgments

- Inspired by daily use of `grep` and the need for cloud-native log search.
- Built with [Rust](https://www.rust-lang.org/), [aws-sdk-rust](https://github.com/awslabs/aws-sdk-rust), and [Localstack](https://github.com/localstack/localstack) for testing.
