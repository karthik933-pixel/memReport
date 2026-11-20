use rayon::prelude::*;
use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::fs;
use std::path::Path;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use sysinfo::{CpuRefreshKind, Disks, RefreshKind, System};
use walkdir::WalkDir;

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd)]
struct FileEntry {
    size: u64,
    path: String,
}

struct CpuReport {
    global_usage: f32,
    cores: Vec<(String, f32)>,
}

struct DriveReport {
    letter: String,
    name: String,
    file_system: String,
    total_bytes: u64,
    used_bytes: u64,
    free_bytes: u64,
    top_files: Vec<FileEntry>,
}

fn format_bytes(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    const TB: f64 = GB * 1024.0;

    let b = bytes as f64;
    if b >= TB {
        format!("{:.2} TB", b / TB)
    } else if b >= GB {
        format!("{:.2} GB", b / GB)
    } else if b >= MB {
        format!("{:.2} MB", b / MB)
    } else if b >= KB {
        format!("{:.2} KB", b / KB)
    } else {
        format!("{} B", bytes)
    }
}

fn escape_html(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn collect_cpu_report() -> CpuReport {
    let mut sys = System::new_with_specifics(
        RefreshKind::nothing().with_cpu(CpuRefreshKind::everything()),
    )
    .expect("Failed to initialize System info");

    thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
    sys.refresh_cpu_usage();

    let global_usage = sys.global_cpu_usage();
    let cores = sys
        .cpus()
        .iter()
        .enumerate()
        .map(|(i, cpu)| (format!("Core {}", i), cpu.usage()))
        .collect();
    CpuReport {
        global_usage,
        cores,
    }
}

fn find_top_files(root: &str, limit: usize) -> Vec<FileEntry> {
    let mut heap: BinaryHeap<Reverse<FileEntry>> = BinaryHeap::with_capacity(limit + 1);

    for entry in WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.file_type().is_file() {
            if let Ok(meta) = entry.metadata() {
                let size = meta.len();
                if heap.len() < limit {
                    heap.push(Reverse(FileEntry {
                        size,
                        path: entry.path().display().to_string(),
                    }));
                } else if let Some(Reverse(smallest)) = heap.peek() {
                    if size > smallest.size {
                        heap.pop();
                        heap.push(Reverse(FileEntry {
                            size,
                            path: entry.path().display().to_string(),
                        }));
                    }
                }
            }
        }
    }

    let mut top: Vec<FileEntry> = heap.into_iter().map(|Reverse(f)| f).collect();
    top.sort_by(|a, b| b.size.cmp(&a.size));
    top
}

fn build_html(cpu: &CpuReport, drives: &[DriveReport], auto_refresh: bool) -> String {
    let refresh_meta = if auto_refresh {
        r#"<meta http-equiv="refresh" content="5">"#
    } else {
        ""
    };

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let cpu_color = if cpu.global_usage > 85.0 {
        "#ef4444"
    } else if cpu.global_usage > 60.0 {
        "#f59e0b"
    } else {
        "#10b981"
    };

    let mut cores_html = String::new();
    for (name, usage) in &cpu.cores {
        let c_color = if *usage > 85.0 {
            "#ef4444"
        } else if *usage > 60.0 {
            "#f59e0b"
        } else {
            "#38bdf8"
        };
        cores_html.push_str(&format!(
            r#"<div class="core-box">
                <div class="core-top">
                    <span>{}</span>
                    <strong>{:.1}%</strong>
                </div>
                <div class="mini-track">
                    <div class="mini-fill" style="width: {:.1}%; background: {};"></div>
                </div>
            </div>"#,
            escape_html(name),
            usage,
            usage.min(100.0),
            c_color
        ));
    }

    let mut drives_html = String::new();
    for d in drives {
        let used_pct = if d.total_bytes > 0 {
            (d.used_bytes as f64 / d.total_bytes as f64) * 100.0
        } else {
            0.0
        };

        let bar_color = if used_pct > 85.0 {
            "#ef4444"
        } else if used_pct > 65.0 {
            "#f59e0b"
        } else {
            "#3b82f6"
        };

        let mut rows_html = String::new();
        for (idx, file) in d.top_files.iter().enumerate() {
            rows_html.push_str(&format!(
                r#"<tr>
                    <td class="rank">#{}</td>
                    <td class="path" title="{}">{}</td>
                    <td class="size">{}</td>
                </tr>"#,
                idx + 1,
                escape_html(&file.path),
                escape_html(&file.path),
                format_bytes(file.size)
            ));
        }

        if rows_html.is_empty() {
            rows_html.push_str(
                r#"<tr><td colspan="3" class="empty">No accessible files found or drive not mounted.</td></tr>"#,
            );
        }

        drives_html.push_str(&format!(
            r#"<section class="card">
                <div class="card-header">
                    <div>
                        <h2>Drive {} <span class="badge">{}</span></h2>
                        <p class="subtitle">File System: {}</p>
                    </div>
                    <div class="stats">
                        <span><strong>Used:</strong> {} ({:.1}%)</span>
                        <span><strong>Free:</strong> {}</span>
                        <span><strong>Total:</strong> {}</span>
                    </div>
                </div>
                <div class="progress-track">
                    <div class="progress-fill" style="width: {:.1}%; background: {};"></div>
                </div>
                <h3>Top 5 Heaviest Files</h3>
                <table>
                    <thead>
                        <tr>
                            <th style="width: 50px;">Rank</th>
                            <th>File Path</th>
                            <th style="width: 120px; text-align: right;">Size</th>
                        </tr>
                    </thead>
                    <tbody>
                        {}
                    </tbody>
                </table>
            </section>"#,
            escape_html(&d.letter),
            escape_html(&d.name),
            escape_html(&d.file_system),
            format_bytes(d.used_bytes),
            used_pct,
            format_bytes(d.free_bytes),
            format_bytes(d.total_bytes),
            used_pct,
            bar_color,
            rows_html
        ));
    }

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
{}
<title>memReport - CPU &amp; Storage Dashboard</title>
<style>
    body {{
        font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
        background: #0f172a;
        color: #e2e8f0;
        margin: 0;
        padding: 2rem;
    }}
    .container {{ max-width: 1100px; margin: 0 auto; }}
    .top-bar {{
        display: flex;
        justify-content: space-between;
        align-items: baseline;
        margin-bottom: 1.5rem;
        flex-wrap: wrap;
        gap: 0.5rem;
    }}
    h1 {{ font-size: 1.75rem; margin: 0; color: #f8fafc; }}
    .meta-time {{ font-size: 0.85rem; color: #94a3b8; }}
    .card {{
        background: #1e293b;
        border: 1px solid #334155;
        border-radius: 12px;
        padding: 1.5rem;
        margin-bottom: 1.5rem;
        box-shadow: 0 4px 12px rgba(0,0,0,0.25);
    }}
    .card-header {{
        display: flex;
        justify-content: space-between;
        align-items: flex-start;
        flex-wrap: wrap;
        gap: 1rem;
    }}
    h2 {{ margin: 0; font-size: 1.3rem; color: #38bdf8; }}
    .badge {{
        font-size: 0.8rem;
        background: #334155;
        color: #cbd5e1;
        padding: 2px 8px;
        border-radius: 6px;
        vertical-align: middle;
    }}
    .subtitle {{ margin: 4px 0 0; font-size: 0.85rem; color: #94a3b8; }}
    .stats {{ display: flex; gap: 1.25rem; font-size: 0.9rem; color: #cbd5e1; }}
    .progress-track {{
        width: 100%;
        height: 10px;
        background: #334155;
        border-radius: 999px;
        overflow: hidden;
        margin: 1rem 0 1.25rem;
    }}
    .progress-fill {{ height: 100%; border-radius: 999px; }}
    .cores-grid {{
        display: grid;
        grid-template-columns: repeat(auto-fill, minmax(140px, 1fr));
        gap: 0.75rem;
    }}
    .core-box {{
        background: #0f172a;
        border: 1px solid #334155;
        border-radius: 8px;
        padding: 0.65rem 0.8rem;
    }}
    .core-top {{
        display: flex;
        justify-content: space-between;
        font-size: 0.82rem;
        margin-bottom: 0.4rem;
        color: #cbd5e1;
    }}
    .mini-track {{
        width: 100%;
        height: 6px;
        background: #334155;
        border-radius: 999px;
        overflow: hidden;
    }}
    .mini-fill {{ height: 100%; border-radius: 999px; }}
    h3 {{ font-size: 0.95rem; margin: 0 0 0.75rem; color: #94a3b8; text-transform: uppercase; letter-spacing: 0.05em; }}
    table {{ width: 100%; border-collapse: collapse; font-size: 0.9rem; }}
    th {{ text-align: left; padding: 0.6rem; border-bottom: 1px solid #334155; color: #94a3b8; }}
    td {{ padding: 0.6rem; border-bottom: 1px solid #1e293b; }}
    tr:nth-child(even) {{ background: rgba(15, 23, 42, 0.45); }}
    .rank {{ font-weight: 700; color: #38bdf8; }}
    .path {{ font-family: Consolas, monospace; word-break: break-all; color: #e2e8f0; }}
    .size {{ text-align: right; font-weight: 600; color: #34d399; white-space: nowrap; }}
    .empty {{ text-align: center; color: #64748b; padding: 1rem; }}
</style>
</head>
<body>
<div class="container">
    <div class="top-bar">
        <h1>memReport &mdash; CPU &amp; Storage Dashboard</h1>
        <span class="meta-time">Snapshot Epoch: {}</span>
    </div>

    <section class="card">
        <div class="card-header">
            <div>
                <h2>CPU Usage <span class="badge">{} Logical Cores</span></h2>
                <p class="subtitle">Real-time global and per-core utilization</p>
            </div>
            <div class="stats">
                <span><strong>Total Load:</strong> {:.1}%</span>
            </div>
        </div>
        <div class="progress-track">
            <div class="progress-fill" style="width: {:.1}%; background: {};"></div>
        </div>
        <h3>Per-Core Breakdown</h3>
        <div class="cores-grid">
            {}
        </div>
    </section>

    {}
</div>
</body>
</html>"#,
        refresh_meta,
        timestamp,
        cpu.cores.len(),
        cpu.global_usage,
        cpu.global_usage.min(100.0),
        cpu_color,
        cores_html,
        drives_html
    )
}

fn scan_drives() -> Vec<DriveReport> {
    let target_drives = ["C:\\", "D:\\", "E:\\"];
    let disks = Disks::new_with_refreshed_list().expect("Failed to list system disks");

    target_drives
        .par_iter()
        .map(|&mount| {
            let disk_info = disks.iter().find(|d| {
                d.mount_point()
                    .to_string_lossy()
                    .eq_ignore_ascii_case(mount)
            });

            let (name, file_system, total_bytes, free_bytes) = match disk_info {
                Some(d) => (
                    d.name().to_string_lossy().into_owned(),
                    d.file_system().to_string_lossy().into_owned(),
                    d.total_space(),
                    d.available_space(),
                ),
                None => ("Not Mounted".to_string(), "N/A".to_string(), 0, 0),
            };

            let used_bytes = total_bytes.saturating_sub(free_bytes);
            let top_files = if Path::new(mount).exists() {
                find_top_files(mount, 5)
            } else {
                Vec::new()
            };

            DriveReport {
                letter: mount.to_string(),
                name: if name.is_empty() {
                    "Local Disk".to_string()
                } else {
                    name
                },
                file_system,
                total_bytes,
                used_bytes,
                free_bytes,
                top_files,
            }
        })
        .collect()
}

fn main() {
    let watch_mode = std::env::args().any(|arg| arg == "--watch");

    println!("Scanning C:\\, D:\\, and E:\\ + measuring CPU usage...");
    let drive_reports = scan_drives();

    if watch_mode {
        println!(
            "Watch mode active: updating index.html CPU metrics every 3 seconds (Ctrl+C to stop)..."
        );
        loop {
            let cpu_report = collect_cpu_report();
            let html = build_html(&cpu_report, &drive_reports, true);
            fs::write("index.html", html).expect("Failed to write index.html");
            thread::sleep(Duration::from_secs(3));
        }
    } else {
        let cpu_report = collect_cpu_report();
        let html = build_html(&cpu_report, &drive_reports, false);
        fs::write("index.html", html).expect("Failed to write index.html");
        println!("Done! Generated index.html");
    }
}