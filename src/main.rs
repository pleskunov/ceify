use std::env;
use std::fs::File;
use std::io::{self, BufRead, BufReader, Write};
use std::path::Path;

struct SpectralData {
    wavelengths: Vec<f64>,
    values: Vec<f64>,
    kind: String, // "uT" or "uR"
}

trait Converter {
    fn process(&self, path: &Path) -> io::Result<Vec<SpectralData>>;
}

// Lambda1050 Implementation

struct Lambda1050;

impl Converter for Lambda1050 {
    fn process(&self, path: &Path) -> io::Result<Vec<SpectralData>> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);

        let mut lines = reader.lines();

        // Step 1: detect type from line 85
        let mut kind = None;
        for (i, line) in lines.by_ref().enumerate() {
            let line = line?;
            if i == 84 {
                if line.contains("%T") {
                    kind = Some("uT".to_string());
                } else if line.contains("%R") {
                    kind = Some("uR".to_string());
                }
                break;
            }
        }

        let kind = kind.ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "Cannot detect %T or %R")
        })?;

        // Step 2: parse data from line 95
        let file = File::open(path)?;
        let reader = BufReader::new(file);

        let mut wavelengths = Vec::new();
        let mut values = Vec::new();

        for (i, line) in reader.lines().enumerate() {
            if i < 94 {
                continue;
            }

            let line = line?;
            let parts: Vec<_> = line.split_whitespace().collect();

            if parts.len() != 2 {
                continue;
            }

            if let (Ok(w), Ok(v)) = (parts[0].parse::<f64>(), parts[1].parse::<f64>()) {
                wavelengths.push(w);
                values.push(v / 100.0);
            }
        }

        Ok(vec![SpectralData {
            wavelengths,
            values,
            kind,
        }])
    }
}

// Cary Implementation

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

            if i < 2 {
                continue; // skip headers
            }

            let cols: Vec<_> = line.split(',').collect();

            if cols.len() < 4 {
                continue;
            }

            let parse = |s: &str| s.trim().parse::<f64>().ok();

            if let (Some(wr), Some(rv), Some(wt), Some(tv)) = (
                parse(cols[0]),
                parse(cols[1]),
                parse(cols[2]),
                parse(cols[3]),
            ) {
                wavelengths_r.push(wr);
                r.push(rv.abs() / 100.0);

                wavelengths_t.push(wt);
                t.push(tv.abs() / 100.0);
            }
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
        writeln!(file, "{:.2}\t{:.6}", w, v)?;
    }

    println!("Saved: {}", output_path.display());

    Ok(())
}

// Format Detection
fn detect_converter(path: &Path) -> Box<dyn Converter> {
    match path.extension().and_then(|s| s.to_str()) {
        Some("csv") => Box::new(Cary),
        Some("asc") => Box::new(Lambda1050),
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
