# ceify

A small command-line utility for converting spectrophotometer output files into a format compatible with CompleteEASE.

## Motivation

This tool was created out of frustration with repeatedly converting spectral data files using Excel or small Python scripts. 
The process was slow, error-prone, and inconvenient for routine work.

`ceify` is intended to be a simple, reliable, drop-in replacement that performs the conversion directly from the command line with no external dependencies.

## What it does

`ceify` reads raw spectrophotometer output files and converts them into text files formatted for CompleteEASE.

Currently supported input formats:

- PerkinElmer Lambda 1050 (`.asc`, `.txt`-like)
- Agilent Cary (`.csv`)

The program automatically detects the input format based on file extension and produces one or more output files in the same directory.

## Output format

Generated files follow the CompleteEASE-compatible structure:

```
Spectroscopic Intensity Data
uT or uR
nm
<wavelength> <value>
```

- Wavelengths are written in nanometers
- Values are normalized to the range `[0, 1]`
- Output files are suffixed with:
  - `_uT.txt` for transmission
  - `_uR.txt` for reflection

## Usage

```shell
ceify <input_file>
```

Example:

```
ceify transmission_spectrum.asc

ceify measurement.csv
```

Output files are written to the same directory as the input file.

## Features

- Single static binary, no dependencies
- Works on Linux, macOS, and Windows
- Automatic format detection (case-insensitive extensions)
- Strict validation of input data
- Clear error messages with line numbers
- Handles both transmission and reflection data
- Simple and predictable output format

## Behavior

- Files are parsed in a single pass
- Input data is validated strictly:
  - Missing `%T` / `%R` markers (Lambda 1050) result in an error
  - Malformed lines (wrong number of columns) result in an error
  - Invalid numeric values result in an error with line number
- Leading/trailing whitespace in values is ignored
- Empty or invalid datasets are rejected

## Implementation

The program is written in Rust and uses only the standard library.

The graphical user interface is implemented using [Slint](https://slint.dev), a declarative GUI toolkit.

### Structure

- `SpectralData` represents a single dataset (wavelength + values + type)
- `Converter` trait defines a common interface for parsers
- Separate implementations exist for each supported instrument:
  - `Lambda1050`
  - `Cary`
- Output writing is shared across all formats

### Parsing

- Lambda 1050:
  - Detects `%T` or `%R` from a fixed header line
  - Reads spectral data starting from a fixed offset
  - Validates column count and numeric values

- Cary:
  - Parses CSV rows without external libraries
  - Extracts reflection and transmission data simultaneously
  - Validates structure and numeric values for each row

### Output

- Files are written using buffered I/O
- Data is formatted with fixed precision
- Output filenames are derived from the input filename

Output files will be created in the same directory as the input file.

## Building

Requires Rust (1.90+ should work, tested with newer versions).

```
cargo build --release
```

Binary will be located at:

`target/release/ceify`

## Limitations

- Assumes consistent file structure from supported instruments
- CSV parsing is simplified (no quoted field handling)
- No batch processing

## Acknowledgements

The graphical interface of `ceify` is built with [Slint](https://slint.dev).

Slint is licensed under the [GPLv3 License](https://slint.dev/terms-and-conditions#gplv3) for open-source projects.

The "Made with Slint" logo is used in accordance with the Slint branding guidelines.

## License

GPL-3.0
