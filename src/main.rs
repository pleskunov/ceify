// ceify 
//
// A small utility for converting spectrophotometer data files into a format compatible with CompleteEASE.
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

// Prevent console window in addition to Slint window in Windows release builds when, e.g., starting the app via file manager.
// Ignored on other platforms.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

pub mod correction_factors;
pub mod config;

use std::io::{self, BufRead, Write};

slint::include_modules!();

// Data representation

struct SpectralData {
    wavelengths: Vec<f64>,
    values: Vec<f64>,
    kind: String, // "uT" or "uR"
}

trait Converter {
    fn process(&self, path: &std::path::Path) -> std::io::Result<Vec<SpectralData>>;
}

// Internal helpers

#[inline(always)]
fn to_fraction(value: f64) -> f64 {
    value / 100.0
}

#[inline]
fn parse_f64(s: &str, line: usize) -> std::io::Result<f64> {
    s.parse::<f64>().map_err(|e| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("Invalid float value '{}' at line {}: {}", s, line, e)
        )
    })
}

#[inline]
fn output_filename(base: &std::path::Path, kind: &str) -> String {
    let stem = base.file_stem()
                                 .unwrap()
                                 .to_string_lossy();
    format!("{}_{}.txt", stem, kind)
}

#[allow(unused)]
#[inline(always)]
fn fast_approx_eq(a: f64, b: f64) -> bool {
    ((a * config::FP_TOL_FACTOR).round() as i64) == ((b * config::FP_TOL_FACTOR).round() as i64)
}

#[inline(always)]
fn precise_approx_eq(a: f64, b: f64) -> bool {
    (a - b).abs() < config::FP_TOLERANCE
}

#[cfg(feature = "fast_fp_compare")]
use fast_approx_eq as approx_eq;

#[cfg(not(feature = "fast_fp_compare"))]
use precise_approx_eq as approx_eq;

// Implementation for Perkin Elmer Lambda 1050 instrument

struct Lambda1050;

impl Converter for Lambda1050 {
    fn process(&self, path: &std::path::Path) -> std::io::Result<Vec<SpectralData>> {
        let file = std::fs::File::open(path)?;
        let reader = std::io::BufReader::new(file);
        let mut is_reflectance: bool = false;

        let mut kind: Option<String> = None;
        let mut wavelengths = Vec::new();
        let mut values = Vec::new();

        for (i, line) in reader.lines().enumerate() {
            let line = line?;

            // Detect the spectrum type from line 85
            if i == 84 {
                if line.contains("%T") {
                    kind = Some("uT".to_string());
                } else if line.contains("%R") {
                    kind = Some("uR".to_string());
                    is_reflectance = true;
                } else {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData, "Missing %T/%R marker."
                    ));
                }
            }

            // Start reading the spectral data from line 95
            if i >= 94 {
                let mut columns = line.split_whitespace();

                let w = columns.next();
                let v = columns.next();

                if columns.next().is_some() || w.is_none() || v.is_none() {
                    return Err(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            format!("Malformed line {}: expected 2 columns", i + 1)
                    ));
                }

                let wvl = parse_f64(w.unwrap().trim(), i + 1)?;
                let val = parse_f64(v.unwrap().trim(), i + 1)?;

                wavelengths.push(wvl);
                values.push(to_fraction(val.abs()));
            }
        }

        if wavelengths.is_empty() {
            return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "No valid spectral data found"
            ));
        }
        
        let kind = kind.ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, "Missing %T/%R marker")
        })?;

        let step_ok = wavelengths
                            .windows(2)
                            .all(|w| approx_eq(w[0] - w[1], 1.0f64));

        if cfg!(debug_assertions) {
            if step_ok {
                println!("[DEBUG] Lambda 1050 step is OK");
            } else {
                println!("[DEBUG] Lambda 1050 step is not OK: {:.5}", wavelengths[0] - wavelengths[1]);
            }
        }

        // Apply Spectralon correction to reflectance spectra aquired with 1.0 nm resolution
        if is_reflectance && step_ok {
            let ref_wvls = &correction_factors::SRS99010_WVLS;
            let ref_cf = &correction_factors::SRS99010_CORR_FACTORS;

            let ref_start = ref_wvls[0]; // 250.0
            let ref_end = ref_wvls[ref_wvls.len() - 1]; // 2500.0

            let mut corrected = values.clone();

            // Iterate over the measured data by wavelength
            for i in 0..wavelengths.len() {
                let wl = wavelengths[i];

                // Fast range rejection (cropped data handling)
                if wl < ref_start || wl > ref_end {
                    continue;
                }

                // Direct index mapping (requires step of 1.0 nm)
                let idx = (wl - ref_start).round() as usize;

                // Safety check to handle cropping or extension
                if idx < ref_cf.len() {
                    corrected[i] *= ref_cf[idx];
                }
            }

            // The data must be specified in ascending order
            wavelengths.reverse();
            corrected.reverse();

            return Ok(vec![SpectralData {
                wavelengths: wavelengths,
                values: corrected,
                kind,
            }]);
        }

        // The data must be specified in ascending order
        wavelengths.reverse();
        values.reverse();

        Ok(vec![SpectralData {
            wavelengths: wavelengths,
            values: values,
            kind,
        }])
    }
}

// Implementation for Agilent Cary instrument

struct Cary;

#[derive(Debug, Clone, Copy)]
enum CaryMode {
    Dual,
    ReflectanceOnly,
    TransmittanceOnly
}

impl Converter for Cary {
    fn process(&self, path: &std::path::Path) -> std::io::Result<Vec<SpectralData>> {
        let file = std::fs::File::open(path)?;
        let mut reader = std::io::BufReader::new(file);
        let mut header = String::new();

        // Skip the first line
        reader.read_line(&mut header)?;
        header.clear();

        // Read the diagnostic header
        reader.read_line(&mut header)?;

        if cfg!(debug_assertions) {
            println!("[DEBUG] Found diagnostic Cary header: {}", header);
        }

        let mode = detect_mode(&header)?;
        match mode {
            CaryMode::Dual => parse_dual(reader),
            CaryMode::ReflectanceOnly => {
                parse_single(reader, "uR")
            }
            CaryMode::TransmittanceOnly => {
                parse_single(reader, "uT")
            }
         }
    }
}

fn detect_mode(header: &str) -> std::io::Result<CaryMode> {
    let has_r = header.contains("%R");
    let has_t = header.contains("%T");

    match (has_r, has_t) {
        (true, true) => Ok(CaryMode::Dual),
        (true, false) => Ok(CaryMode::ReflectanceOnly),
        (false, true) => Ok(CaryMode::TransmittanceOnly),
        (false, false) => Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Unknown measurements mode.",
        )),
    }
}

fn parse_dual<R: BufRead>(reader: R) -> std::io::Result<Vec<SpectralData>> {
    let mut wavelengths_r = Vec::new();
    let mut r = Vec::new();

    let mut wavelengths_t = Vec::new();
    let mut t = Vec::new();

    for (i, line) in reader.lines().enumerate() {
        let line = line?;

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
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Malformed line {}: expected 4 columns", i + 3)
            ));
        }

        let wr = parse_f64(wr.unwrap().trim(), i + 3)?;
        let rv = parse_f64(rv.unwrap().trim(), i + 3)?;
        let wt = parse_f64(wt.unwrap().trim(), i + 3)?;
        let tv = parse_f64(tv.unwrap().trim(), i + 3)?;

        wavelengths_r.push(wr);
        r.push(to_fraction(rv.abs()));

        wavelengths_t.push(wt);
        t.push(to_fraction(tv.abs()));
    }

    if wavelengths_r.is_empty() || wavelengths_t.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "No spectral data found."
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

fn parse_single<R: BufRead>(reader: R, kind: &'static str) -> std::io::Result<Vec<SpectralData>> {
    let mut wavelengths = Vec::new();
    let mut intensities = Vec::new();
    
    for (i, line) in reader.lines().enumerate() {
        let line = line?;

        if line.trim().is_empty() {
            continue;
        }

        let mut columns = line.split(',')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty());

        let w = columns.next();
        let v = columns.next();

        if columns.next().is_some() || w.is_none() || v.is_none() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Malformed line {}: expected 2 columns", i + 3)
            ));
        }

        let wavelength = parse_f64(w.unwrap().trim(), i + 3)?;
        let intensity = parse_f64(v.unwrap().trim(), i + 3)?;

        wavelengths.push(wavelength);
        intensities.push(to_fraction(intensity.abs()));
    }

    if wavelengths.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "No spectral data found."
        ));
    }

    Ok(vec![
        SpectralData {
            wavelengths,
            values: intensities,
            kind: kind.to_string(),
        }
    ])
}

// Writer
fn write_output(base: &std::path::Path, data: &SpectralData) -> io::Result<()> {
    let stem = base.file_stem().unwrap().to_string_lossy();
    let parent = base.parent().unwrap_or(std::path::Path::new(""));

    let filename = format!("{}_{}.txt", stem, data.kind);
    let output_path = parent.join(filename);

    let mut file = std::fs::File::create(&output_path)?;

    writeln!(file, "Spectroscopic Intensity Data")?;
    writeln!(file, "{}", data.kind)?;
    writeln!(file, "nm")?;

    for (w, v) in data.wavelengths.iter().zip(&data.values) {
        writeln!(file, "{:.2}\t{:.8}", w, v)?;
    }

    if cfg!(debug_assertions) {
        println!("[DEBUG] Saved: {}", output_path.display());
    }

    Ok(())
}

// Format Detection
fn detect_converter(path: &std::path::Path) -> Box<dyn Converter> {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some(ext) if ext.eq_ignore_ascii_case("csv") => Box::new(Cary),
        Some(ext) if ext.eq_ignore_ascii_case("asc") => Box::new(Lambda1050),
        _ => {
            // fallback: assume txt-like
            Box::new(Lambda1050)
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let ui = AppWindow::new()?;
    let weak_ui = ui.as_weak();

    // Connect to UI
    ui.on_select_file(move || {

        let Some(path_buf) = rfd::FileDialog::new()
            .pick_file()
        else {
            return;
        };

        let path = path_buf.as_path();

        let ui = weak_ui.unwrap();

        ui.set_selected_file(path.display().to_string().into());

        let converter = detect_converter(path);

        match converter.process(path) {

            Ok(datasets) => {

                let mut outputs: Vec<String> = Vec::new();

                for data in datasets {

                    if let Err(e) = write_output(path, &data) {

                        rfd::MessageDialog::new()
                            .set_title("Write Error")
                            .set_description(format!("{}", e))
                            .set_level(rfd::MessageLevel::Error)
                            .show();

                        return;
                    }
                    outputs.push(output_filename(path, &data.kind));
                }

                ui.set_saved_to(outputs.join("\n").into());

                ui.set_status_text("Success".into());
            }

            Err(e) => {
                ui.set_status_text("Conversion failed".into());

                rfd::MessageDialog::new()
                    .set_title("Conversion Error")
                    .set_description(format!("{}", e))
                    .set_level(rfd::MessageLevel::Error)
                    .show();
            }
        }
    });

    ui.run()?;

    Ok(())
}
