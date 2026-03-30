// ceify 
//
// A small command-line utility for converting spectrophotometer output files into a format compatible with CompleteEASE.
//
// Copyright (c) 2026 Pavel Pleskunov.
// 
// ceify is free software; you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation; either version 3 of the License, or (at
// your option) any later version.
//
// ceify is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program; if not, write to the Free Software
// Foundation, Inc., 59 Temple Place, Suite 330, Boston, MA 02111-1307
// USA

use std::env;
use std::fs::File;
use std::io::{self, BufRead, BufReader, Write};
use std::path::Path;

// Data representation

struct SpectralData {
    wavelengths: Vec<f64>,
    values: Vec<f64>,
    kind: String, // "uT" or "uR"
}

trait Converter {
    fn process(&self, path: &Path) -> io::Result<Vec<SpectralData>>;
}

// Internal helpers

#[inline]
fn normalize(value: f64) -> f64 {
    value / 100.0
}

#[inline]
fn parse_f64(s: &str, line: usize) -> io::Result<f64> {
    s.parse::<f64>().map_err(|e| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Invalid float value '{}' at line {}: {}", s, line + 1, e)
        )
    })
}

// Implementation for Perkin Elmer Lambda 1050 instrument

struct Lambda1050;

impl Converter for Lambda1050 {
    fn process(&self, path: &Path) -> io::Result<Vec<SpectralData>> {
        let file: File = File::open(path)?;
        let reader: BufReader<File> = BufReader::new(file);

        let mut kind: Option<String> = None;
        let mut wavelengths: Vec<f64> = Vec::new();
        let mut values: Vec<f64> = Vec::new();

        for (i, line) in reader.lines().enumerate() {
            let line: String = line?;

            // Detect the spectrum type from line 85
            if i == 84 {
                if line.contains("%T") {
                    kind = Some("uT".to_string());
                } else if line.contains("%R") {
                    kind = Some("uR".to_string());
                } else {
                    return Err(io::Error::new(io::ErrorKind::InvalidData, "Missing %T/%R marker"));
                }
            }

            // Start reading the spectral data from line 95
            if i >= 94 {
                let mut columns = line.split_whitespace();

                let w = columns.next();
                let v = columns.next();

                if columns.next().is_some() || w.is_none() || v.is_none() {
                    return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            format!("Malformed line {}: expected 2 columns", i + 1)
                    ));
                }

                let wvl = parse_f64(w.unwrap().trim(), i)?;
                let val = parse_f64(v.unwrap().trim(), i)?;

                wavelengths.push(wvl);
                values.push(normalize(val));
            }
        }

        if wavelengths.is_empty() {
            return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "No valid spectral data found"
            ));
        }
        
        let kind = kind.ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "Missing %T/%R marker")
        })?;

        Ok(vec![SpectralData {
            wavelengths,
            values,
            kind,
        }])
    }
}

// Implementation for Agilent Cary instrument

struct Cary;

impl Converter for Cary {
    fn process(&self, path: &Path) -> io::Result<Vec<SpectralData>> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);

        let mut wavelengths_r = Vec::new();
        let mut r = Vec::new();

        let mut wavelengths_t = Vec::new();
        let mut t = Vec::new();

        for (i, line) in reader.lines().enumerate() {
            let line = line?;
            
            // Skip headers
            if i < 2 {
                continue;
            }

            // Skip empty lines
            if line.trim().is_empty() {
                continue;
            }

            let mut columns = line.split(',')
                .map(|s| s.trim())
                .filter(|s| !s.is_empty());

            let wr = columns.next();
            let rv = columns.next();
            let wt = columns.next();
            let tv = columns.next();

            if columns.next().is_some() || wr.is_none() || rv.is_none() || wt.is_none() || tv.is_none() {
                return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("Malformed line {}: expected 4 columns", i + 1)
                ));
            }

            let wr = parse_f64(wr.unwrap().trim(), i)?;
            let rv = parse_f64(rv.unwrap().trim(), i)?;
            let wt = parse_f64(wt.unwrap().trim(), i)?;
            let tv = parse_f64(tv.unwrap().trim(), i)?;

            wavelengths_r.push(wr);
            r.push(normalize(rv.abs()));

            wavelengths_t.push(wt);
            t.push(normalize(tv.abs()));
        }

        if wavelengths_r.is_empty() || wavelengths_t.is_empty() {
            return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "No valid spectral data found"
            ));
        }

        Ok(vec![
            SpectralData {
                wavelengths: wavelengths_r,
                values: r,
                kind: "uR".to_string(),
            },
            SpectralData {
                wavelengths: wavelengths_t,
                values: t,
                kind: "uT".to_string(),
            },
        ])
    }
}


// Writer
fn write_output(base: &Path, data: &SpectralData) -> io::Result<()> {
    let stem = base.file_stem().unwrap().to_string_lossy();
    let parent = base.parent().unwrap_or(Path::new(""));

    let filename = format!("{}_{}.txt", stem, data.kind);
    let output_path = parent.join(filename);

    let mut file = File::create(&output_path)?;

    writeln!(file, "Spectroscopic Intensity Data")?;
    writeln!(file, "{}", data.kind)?;
    writeln!(file, "nm")?;

    for (w, v) in data.wavelengths.iter().zip(&data.values) {
        writeln!(file, "{:.2}\t{:.8}", w, v)?;
    }

    println!("Saved: {}", output_path.display());

    Ok(())
}

// Format Detection
fn detect_converter(path: &Path) -> Box<dyn Converter> {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some(ext) if ext.eq_ignore_ascii_case("csv") => Box::new(Cary),
        Some(ext) if ext.eq_ignore_ascii_case("asc") => Box::new(Lambda1050),
        _ => {
            // fallback: assume txt-like
            Box::new(Lambda1050)
        }
    }
}

fn main() -> io::Result<()> {
    let args: Vec<String> = env::args().collect();

    if args.len() != 2 {
        eprintln!("Usage: ceify <input_file>");
        std::process::exit(1);
    }

    let input = Path::new(&args[1]);

    if !input.exists() {
        eprintln!("Error: file not found");
        std::process::exit(1);
    }

    let converter = detect_converter(input);

    let datasets = converter.process(input)?;

    for data in datasets {
        write_output(input, &data)?;
    }

    Ok(())
}
