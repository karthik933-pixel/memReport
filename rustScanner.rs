use std::fs;
use std::path::PathBuf;
use std::time::Instant;

const TARGET_DRIVE: &str = "D:\\";
const OUTPUT_FILE: &str = "rust_result.json";

fn escape_json(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn main() {
    let start = Instant::now();

    let mut total_files: u64 = 0;
    let mut total_dirs: u64 = 0;
    let mut errors: u64 = 0;
    let mut smallest_five: Vec<String> = Vec::with_capacity(5);

    let mut stack: Vec<PathBuf> = vec![PathBuf::from(TARGET_DRIVE)];

    while let Some(current_dir) = stack.pop() {
        let entries = match fs::read_dir(&current_dir) {
            Ok(rd) => rd,
            Err(_) => {
                errors += 1;
                continue;
            }
        };

        for entry_res in entries {
            let entry = match entry_res {
                Ok(e) => e,
                Err(_) => {
                    errors += 1;
                    continue;
                }
            };

            let file_type = match entry.file_type() {
                Ok(ft) => ft,
                Err(_) => {
                    errors += 1;
                    continue;
                }
            };

            if file_type.is_dir() {
                total_dirs += 1;
                stack.push(entry.path());
            } else if file_type.is_file() {
                total_files += 1;
                let name = entry.file_name().to_string_lossy().into_owned();

                if smallest_five.len() < 5 {
                    smallest_five.push(name);
                    smallest_five.sort();
                } else if name < smallest_five[4] {
                    smallest_five[4] = name;
                    smallest_five.sort();
                }
            }
        }
    }

    let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
    let files_per_sec = if elapsed_ms > 0.0 {
        total_files as f64 / (elapsed_ms / 1000.0)
    } else {
        0.0
    };

    let top_5_json: Vec<String> = smallest_five
        .iter()
        .map(|f| format!("\"{}\"", escape_json(f)))
        .collect();

    let json = format!(
        r#"{{
  "language": "Rust",
  "drive": "{}",
  "elapsed_ms": {:.3},
  "total_files": {},
  "total_dirs": {},
  "errors_skipped": {},
  "files_per_sec": {:.1},
  "top_5_files": [{}]
}}"#,
        escape_json(TARGET_DRIVE),
        elapsed_ms,
        total_files,
        total_dirs,
        errors,
        files_per_sec,
        top_5_json.join(", ")
    );

    fs::write(OUTPUT_FILE, json).expect("Failed to write rust_result.json");
    println!(
        "[Rust]   Scanned {} files in {:.2} ms -> saved to {}",
        total_files, elapsed_ms, OUTPUT_FILE
    );
}