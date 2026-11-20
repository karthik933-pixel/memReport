use std::fs;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use sysinfo::{CpuRefreshKind, RefreshKind, System};
use tiny_http::{Header, Response, Server};

#[derive(Clone, serde::Serialize)]
struct CpuSnapshot {
    global: f32,
    cores: Vec<f32>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut sys = System::new_with_specifics(
        RefreshKind::nothing().with_cpu(CpuRefreshKind::everything()),
    )?;

    thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
    sys.refresh_cpu_usage();

    let latest = Arc::new(Mutex::new(CpuSnapshot {
        global: sys.global_cpu_usage(),
        cores: sys.cpus().iter().map(|c| c.usage()).collect(),
    }));

    // Background thread to update CPU stats every second
    let updater_state = Arc::clone(&latest);
    thread::spawn(move || loop {
        thread::sleep(Duration::from_secs(1));
        sys.refresh_cpu_usage();

        let snapshot = CpuSnapshot {
            global: sys.global_cpu_usage(),
            cores: sys.cpus().iter().map(|c| c.usage()).collect(),
        };

        if let Ok(mut data) = updater_state.lock() {
            *data = snapshot;
        }
    });

    // Start local web server on port 8080
    let server = Server::http("127.0.0.1:8080").unwrap();
    println!("Dashboard running at http://127.0.0.1:8080");

    for request in server.incoming_requests() {
        match request.url() {
            "/api/cpu" => {
                let json = {
                    let data = latest.lock().unwrap();
                    serde_json::to_string(&*data).unwrap()
                };
                let header = Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap();
                let _ = request.respond(Response::from_string(json).with_header(header));
            }
            _ => {
                let html = fs::read_to_string("index.html")
                    .unwrap_or_else(|_| "<h1>Error: index.html not found in project root</h1>".to_string());
                let header = Header::from_bytes(&b"Content-Type"[..], &b"text/html; charset=utf-8"[..]).unwrap();
                let _ = request.respond(Response::from_string(html).with_header(header));
            }
        }
    }

    Ok(())
}