// extendo-core entry point.
//
//   extendo-core --bind 0.0.0.0 --port 9578 --token <tok> --monitor 0 \
//                --width 1280 --height 720 --fps 30 --quality 70
//
// Spawned by the Node host (host/src/capture.js backend "wgc"). Width/height/fps
// arrive already capped by the entitlement layer - see AGENTS.md section 10.1.

use std::sync::Arc;

use extendo_core::{capture, cli::Args, server, JpegFrame, Stats};
use tokio::sync::watch;

#[tokio::main]
async fn main() -> std::process::ExitCode {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    if argv.iter().any(|a| a == "--help" || a == "-h") {
        println!("{USAGE}");
        return std::process::ExitCode::SUCCESS;
    }
    let cfg = match Args::parse(argv) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("extendo-core: {e}\n\n{USAGE}");
            return std::process::ExitCode::FAILURE;
        }
    };

    let monitor = match capture::resolve_monitor(cfg.monitor) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("extendo-core: {e}");
            return std::process::ExitCode::FAILURE;
        }
    };
    // Friendly name is best-effort: a virtual/headless display may not report one.
    let source = monitor
        .name()
        .or_else(|_| monitor.device_name())
        .unwrap_or_else(|_| format!("monitor#{}", cfg.monitor));

    let stats = Arc::new(Stats::default());
    let (tx, rx) = watch::channel::<Option<JpegFrame>>(None);

    if let Err(e) = capture::spawn(monitor, cfg.clone(), tx, Arc::clone(&stats)) {
        eprintln!("extendo-core: cannot start capture thread: {e}");
        return std::process::ExitCode::FAILURE;
    }

    match server::serve(cfg, rx, stats, source).await {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("extendo-core: server failed: {e}");
            std::process::ExitCode::FAILURE
        }
    }
}

const USAGE: &str = "\
extendo-core - Windows Graphics Capture -> JPEG -> MJPEG

  --bind <addr>     listen address (default 127.0.0.1; non-loopback needs --token)
  --port <n>        listen port (default 9578)
  --token <tok>     pairing token required by /video.mjpg and /frame.jpg
  --monitor <n>     0 = primary, else 1-based monitor index
  --width <px>      max width  (default 1280, v1 cap 1920)
  --height <px>     max height (default 720,  v1 cap 1080)
  --fps <n>         frame cap  (default 30, v1 cap 60)
  --quality <1-100> JPEG quality (default 70)

Routes: GET /video.mjpg  GET /frame.jpg  GET /health";
