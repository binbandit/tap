# tap 🖱️

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-1.77%2B-blue.svg)](https://www.rust-lang.org)

`tap` is a next-generation replacement for the Unix `touch` command, offering enhanced capabilities and intuitive options for file and directory manipulation.

## 🌟 Features

- Create or update files and directories
- Set file permissions (with recursive option for directories)
- Write or append content to files
- Use template files for content
- Remove trailing whitespace from lines
- Check for file/directory existence without modification
- Support for glob patterns
- Set custom timestamps for files
- Convert line endings (CRLF ↔ LF)
- Force creation of parent directories
- Transform file encodings (with automatic detection)
- Structured output formats (JSON, YAML)
- Cross-platform compatibility

## 🚀 Installation

To install `tap`, you need to have Rust and Cargo installed on your system. If you don't have them installed, you can get them from [rust-lang.org](https://www.rust-lang.org/tools/install).

Once you have Rust and Cargo, you can install `tap` using the following command:

```bash
cargo install tap
```

Or, to build from source:

```bash
git clone https://github.com/crazywolf132/tap.git
cd tap
cargo build --release
```

The built binary will be located at `target/release/tap`.

## 💡 Usage

Here are some examples of how to use `tap`:

```bash
# Create a new file or update its timestamp
tap file.txt

# Create multiple files
tap file1.txt file2.txt file3.txt

# Create a directory
tap -d new_directory

# Set file permissions
tap --chmod 644 file.txt

# Set file permissions recursively
tap -R --chmod 755 src/

# Write content to a file
tap -w "Hello, World!" greeting.txt

# Append content to a file
tap -a -w "New line" existing_file.txt

# Use a template file
tap --template README.md new_file.md

# Remove trailing whitespace
tap --trim *.txt

# Check if files exist (dry run)
tap --check config/*.yml

# Set a specific timestamp
tap -t "2023-05-01 12:00:00" file.txt

# Use glob patterns
tap src/**/*.rs

# Force creation of parent directories
tap --force deeply/nested/new/file.txt

# Convert line endings
tap --line_endings crlf2lf windows_file.txt
tap --line_endings lf2crlf unix_file.txt

# Get JSON output
tap --output-format json *.txt

# Get YAML output
tap --output-format yaml config/

# Convert file encodings with automatic detection
tap --encoding utf8 mixed_encoding_files/*.txt
```

## 🔧 Options

- `-d, --dir`: Create a directory instead of a file
- `--chmod <MODE>`: Set specific permissions (octal format, e.g., 644)
- `-w, --write <CONTENT>`: Add content to the file
- `-t, --timestamp <TIME>`: Set access and modification times (YYYY-MM-DD HH:MM:SS)
- `-a, --append`: Append content instead of overwriting
- `-v, --verbose`: Enable verbose output
- `-R, --recursive`: Apply chmod recursively (only works with directories)
- `--template <FILE>`: Use a template file for content
- `--trim`: Remove trailing whitespace from each line
- `--check`: Check if the file or directory exists (dry run)
- `-f, --force`: Force creation of parent directories without confirmation
- `--line_endings <CONVERSION>`: Convert line endings (values: crlf2lf, lf2crlf)
- `--encoding <ENCODING>`: Convert file encoding (values: utf8, latin1, windows-1252)
- `--timestamp_format <FORMAT>`: Custom timestamp format (e.g., "%Y/%m/%d %H:%M")
- `--output-format <FORMAT>`: Output format (values: text, json, yaml)

## 🏗️ Architecture

The `tap` application is organized into modules for better maintainability:

- `cli`: Command-line interface definition
- `file_ops`: File and directory operations
- `permissions`: Permission handling
- `timestamp`: Timestamp parsing and modification
- `output`: Output formatting and result handling
- `glob_utils`: Path expansion utilities
- `main`: Main program logic and orchestration

## 🤝 Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## 🙏 Acknowledgments

- Inspired by the original Unix `touch` command
- Built with [Rust](https://www.rust-lang.org/) and [clap](https://github.com/clap-rs/clap)