# ceify

A small command-line utility for converting spectrophotometer output files into a format compatible with CompleteEASE.

## Motivation

This tool was created out of frustration with repeatedly converting spectral data files using Excel or ad hoc Python scripts. 
The process was slow, error-prone, and inconvenient for routine work.

The goal of `ceify` is to provide a simple, reliable, and drop-in solution that works directly from the command line without requiring external dependencies or manual intervention.

## What it does

`ceify` reads raw spectrophotometer output files and converts them into text files formatted for CompleteEASE.

Currently supported input formats:

- PerkinElmer Lambda 1050 (ASC)
- Agilent Cary (CSV)

The program automatically detects the input format and produces one or more output files in the same directory.

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

Output files will be created in the same directory as the input file.

## Features

- Single static binary, no dependencies
- Works on Linux, macOS, and Windows
- Automatic format detection
- Handles both transmission and reflection data
- Fast enough for typical lab datasets
- Simple and predictable output
- No need for Python, Excel, or manual editing

## Implementation

The program is written in Rust and uses only the standard library.

### Structure

- A common data structure represents spectral datasets
- A trait-based design is used to support multiple input formats
- Each spectrophotometer format has its own parser
- Output generation is shared across formats

### Parsing

- Lambda 1050:
  - Detects `%T` or `%R` from header
  - Reads data starting from a fixed line offset
  - Normalizes values from percent to `[0, 1]`

- Cary:
  - Parses CSV rows
  - Extracts both reflection and transmission data
  - Normalizes values and splits into two datasets

### Output

- Files are written using buffered I/O
- Data is formatted with fixed precision
- Output filenames are derived from the input filename

## Building

Requires Rust (1.70+ should work, tested with newer versions).

```
cargo build --release
```

Binary will be located at:

`target/release/ceify`

## Limitations

- Assumes consistent file structure from supported instruments
- Limited validation of malformed input
- No batch processing (yet)

## Future improvements

- Better format detection
- Support for additional instruments
- Batch directory processing
- Configurable output formatting

## License

GPL-3.0
