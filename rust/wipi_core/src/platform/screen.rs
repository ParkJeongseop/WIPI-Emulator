use std::sync::{
    Mutex,
    atomic::{AtomicBool, AtomicU64, Ordering},
};

use wie_backend::{Screen, canvas::Image};
use wie_util::Result;

#[derive(Clone)]
pub struct CapturedFrame {
    pub width: u32,
    pub height: u32,
    /// RGBA, row-major
    pub pixels: Vec<u8>,
}

/// Screen that keeps the latest painted frame in memory. The Android UI (or
/// the headless test harness) pulls frames out with `take_frame`.
pub struct CaptureScreen {
    width: u32,
    height: u32,
    frame: Mutex<Option<CapturedFrame>>,
    dirty: AtomicBool,
    redraw_requested: AtomicBool,
    /// Frames painted so far, for the host's frame rate display.
    frame_count: AtomicU64,
}

impl CaptureScreen {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            frame: Mutex::new(None),
            dirty: AtomicBool::new(false),
            redraw_requested: AtomicBool::new(false),
            frame_count: AtomicU64::new(0),
        }
    }

    /// Returns the latest frame if a new one was painted since the last call.
    pub fn take_frame(&self) -> Option<CapturedFrame> {
        if !self.dirty.swap(false, Ordering::SeqCst) {
            return None;
        }
        self.frame.lock().unwrap().clone()
    }

    pub fn frame_count(&self) -> u64 {
        self.frame_count.load(Ordering::Relaxed)
    }

    pub fn take_redraw_request(&self) -> bool {
        self.redraw_requested.swap(false, Ordering::SeqCst)
    }
}

impl Screen for CaptureScreen {
    fn resize(&self, width: u32, height: u32) -> Result<()> {
        // 모바일 화면은 고정 크기 프레임 캡처라 게임의 리사이즈 요청은 적용하지 않는다.
        tracing::debug!("screen resize request ignored: {width}x{height}");
        Ok(())
    }

    fn request_redraw(&self) -> Result<()> {
        self.redraw_requested.store(true, Ordering::SeqCst);
        Ok(())
    }

    fn paint(&self, image: &dyn Image) {
        let pixels = image.colors().into_iter().flat_map(|c| [c.r, c.g, c.b, c.a]).collect();

        *self.frame.lock().unwrap() = Some(CapturedFrame {
            width: image.width(),
            height: image.height(),
            pixels,
        });
        self.dirty.store(true, Ordering::SeqCst);
        self.frame_count.fetch_add(1, Ordering::Relaxed);
    }

    fn width(&self) -> u32 {
        self.width
    }

    fn height(&self) -> u32 {
        self.height
    }
}
