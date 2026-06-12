//! Scrolling-capture session: a thread captures the selected region at ~3fps
//! via xcap and feeds the Stitcher; stop() finishes, saves, and re-enters the
//! normal capture pipeline. One session at a time.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

use crate::stitch::{PushResult, StitchConfig, Stitcher};

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub struct Region {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Serialize)]
struct ProgressPayload {
    frames: u32,
    height: u32,
}

#[derive(Clone, Serialize)]
struct StatusPayload {
    state: &'static str, // "locked" | "lost"
}

pub struct Session {
    stop: Arc<AtomicBool>,
    handle: JoinHandle<Stitcher>,
}

#[derive(Default)]
pub struct ScrollState(pub Mutex<Option<Session>>);

/// Settle time before the first frame so the collapsing veil never bakes in.
const FIRST_FRAME_DELAY_MS: u64 = 150;
/// Tiny breather between frames; real pacing comes from the capture call
/// itself (CGWindowListCreateImage takes tens of ms for large regions).
const LOOP_BREATHER_MS: u64 = 15;

pub fn start(app: AppHandle, region: Region) -> Result<Session, String> {
    // Verify the primary monitor is reachable before spawning the thread.
    let _primary = xcap::Monitor::all()
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|m| m.is_primary().unwrap_or(false))
        .ok_or("no primary monitor")?;

    let stop = Arc::new(AtomicBool::new(false));
    let stop_t = stop.clone();
    let handle = std::thread::spawn(move || {
        let mut stitcher = Stitcher::new(StitchConfig::default());
        let mut frames: u32 = 0;
        let mut lowconf_run: u32 = 0;
        let mut warned = false;

        // Settle time: the selector veil has just been shrunk into the control
        // panel and the window move must settle, or the veil gets baked into
        // frame 1.
        std::thread::sleep(std::time::Duration::from_millis(FIRST_FRAME_DELAY_MS));
        if stop_t.load(Ordering::Relaxed) {
            return stitcher;
        }

        loop {
            if stop_t.load(Ordering::Relaxed) {
                break;
            }
            // Re-resolve the primary monitor inside the thread on each frame so
            // we don't need to hold a Monitor across thread boundaries.
            let monitor = match xcap::Monitor::all()
                .ok()
                .and_then(|v| v.into_iter().find(|m| m.is_primary().unwrap_or(false)))
            {
                Some(m) => m,
                None => {
                    let _ = app.emit("scroll-capture-error", "no primary monitor".to_string());
                    break;
                }
            };
            let t0 = std::time::Instant::now();
            match monitor.capture_region(region.x, region.y, region.width, region.height) {
                Ok(frame) => {
                    let capture_ms = t0.elapsed().as_millis();
                    let t1 = std::time::Instant::now();
                    let result = stitcher.push_frame(&frame);
                    let push_ms = t1.elapsed().as_millis();
                    eprintln!(
                        "[scroll] frame={} capture={}ms push={}ms result={:?} canvas_h={}",
                        frames, capture_ms, push_ms, result, stitcher.height()
                    );
                    match result {
                        PushResult::SkippedLowConfidence => {
                            lowconf_run += 1;
                            if lowconf_run >= 2 && !warned {
                                warned = true;
                                let _ = app.emit("scroll-status", StatusPayload { state: "lost" });
                            }
                        }
                        PushResult::AppendedRows(_) | PushResult::HardAppended => {
                            lowconf_run = 0;
                            if warned {
                                warned = false;
                                let _ = app.emit("scroll-status", StatusPayload { state: "locked" });
                            }
                        }
                        _ => {}
                    }
                    frames += 1;
                    let _ = app.emit(
                        "scroll-progress",
                        ProgressPayload { frames, height: stitcher.height() },
                    );
                }
                Err(e) => {
                    eprintln!("scroll frame capture failed: {e}");
                    let _ = app.emit("scroll-capture-error", e.to_string());
                    break;
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(LOOP_BREATHER_MS));
        }
        stitcher
    });

    Ok(Session { stop, handle })
}

/// Stops the loop and returns the stitched image (None if no frame was captured).
/// Returns Err if the capture thread panicked.
pub fn stop(session: Session) -> Result<Option<image::RgbaImage>, String> {
    session.stop.store(true, Ordering::Relaxed);
    let stitcher = session
        .handle
        .join()
        .map_err(|_| "capture thread crashed".to_string())?;
    if stitcher.height() == 0 {
        return Ok(None);
    }
    Ok(Some(stitcher.finish()))
}

pub fn cancel(session: Session) {
    session.stop.store(true, Ordering::Relaxed);
    let _ = session.handle.join();
}
