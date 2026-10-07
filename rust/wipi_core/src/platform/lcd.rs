use std::sync::Mutex;

use wie_backend::{
    AudioSink, DatabaseRepository, Filesystem, Instant, Platform, Screen,
    canvas::{ArgbPixel, Color, Font, Image, PixelType, VecImageBuffer},
};
use wie_util::{Result, WieError};

/// 게임이 요청한 크기의 LCD를 호스트의 고정 프레임 안에 보여주는 플랫폼 래퍼.
///
/// J2ME(ez-java) 게임은 디스크립터가 정한 LCD 크기(176x200, 120x143 등)에 맞춰
/// 고정 좌표로 그리므로 그 크기의 화면을 줘야 한다. 반면 모바일 UI는 고정 크기
/// 프레임만 받으므로, 게임이 그린 프레임을 비율을 유지해 확대하고 가운데 정렬한
/// 고정 크기 프레임으로 바꿔 넘긴다. WIPI 로더는 이 래퍼를 쓰지 않아 기존 동작 그대로다.
pub struct LcdPlatform {
    inner: Box<dyn Platform>,
    /// 게임이 보는 화면 크기. resize 요청 전에는 호스트 프레임 크기와 같다.
    size: Mutex<(u32, u32)>,
}

impl LcdPlatform {
    /// 게임이 요청할 수 있는 한 변의 최대 길이 — 그 이상은 피처폰 LCD일 수 없다.
    const MAX_SIDE: u32 = 1024;

    pub fn new(inner: Box<dyn Platform>) -> Self {
        let size = (inner.screen().width(), inner.screen().height());

        Self {
            inner,
            size: Mutex::new(size),
        }
    }

    /// `image`를 호스트 프레임 크기에 맞춰 확대(최근접)하고 남는 곳은 검게 채운다.
    fn fit(image: &dyn Image, frame_width: u32, frame_height: u32) -> VecImageBuffer<ArgbPixel> {
        let (width, height) = (image.width() as u64, image.height() as u64);
        let (frame_width, frame_height) = (frame_width as u64, frame_height as u64);

        // 가로를 꽉 채웠을 때 세로가 넘치지 않으면 가로 기준, 아니면 세로 기준
        let (fit_width, fit_height) = if height * frame_width <= frame_height * width {
            (frame_width, height * frame_width / width)
        } else {
            (width * frame_height / height, frame_height)
        };
        let (left, top) = ((frame_width - fit_width) / 2, (frame_height - fit_height) / 2);

        let colors = image.colors();
        let black = ArgbPixel::from_color(Color { a: 255, r: 0, g: 0, b: 0 });
        let mut frame = vec![black; (frame_width * frame_height) as usize];
        for y in 0..fit_height {
            let source_row = (y * height / fit_height * width) as usize;
            let frame_row = ((top + y) * frame_width + left) as usize;
            for x in 0..fit_width {
                frame[frame_row + x as usize] = ArgbPixel::from_color(colors[source_row + (x * width / fit_width) as usize]);
            }
        }

        VecImageBuffer::from_raw(frame_width as u32, frame_height as u32, frame)
    }
}

impl Screen for LcdPlatform {
    fn resize(&self, width: u32, height: u32) -> Result<()> {
        if width == 0 || height == 0 || width > Self::MAX_SIDE || height > Self::MAX_SIDE {
            return Err(WieError::FatalError(format!("Unsupported LCD size {width}x{height}")));
        }
        *self.size.lock().unwrap() = (width, height);

        Ok(())
    }

    fn request_redraw(&self) -> Result<()> {
        self.inner.screen().request_redraw()
    }

    fn paint(&self, image: &dyn Image) {
        let screen = self.inner.screen();
        if image.width() == 0 || image.height() == 0 || (image.width(), image.height()) == (screen.width(), screen.height()) {
            screen.paint(image);
        } else {
            screen.paint(&Self::fit(image, screen.width(), screen.height()));
        }
    }

    fn width(&self) -> u32 {
        self.size.lock().unwrap().0
    }

    fn height(&self) -> u32 {
        self.size.lock().unwrap().1
    }
}

impl Platform for LcdPlatform {
    fn font(&self) -> &Font {
        self.inner.font()
    }

    fn screen(&self) -> &dyn Screen {
        self
    }

    fn now(&self) -> Instant {
        self.inner.now()
    }

    fn database_repository(&self) -> &dyn DatabaseRepository {
        self.inner.database_repository()
    }

    fn filesystem(&self) -> &dyn Filesystem {
        self.inner.filesystem()
    }

    fn audio_sink(&self) -> Box<dyn AudioSink> {
        self.inner.audio_sink()
    }

    fn write_stdout(&self, buf: &[u8]) {
        self.inner.write_stdout(buf)
    }

    fn write_stderr(&self, buf: &[u8]) {
        self.inner.write_stderr(buf)
    }

    fn exit(&self) {
        self.inner.exit()
    }

    fn vibrate(&self, duration_ms: u64, intensity: u8) {
        self.inner.vibrate(duration_ms, intensity)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_scales_by_width_and_centres_vertically() {
        // 176x200 → 240x320: 가로 기준 240x272, 위아래 24줄씩 검은 띠
        let white = ArgbPixel::from_color(Color {
            a: 255,
            r: 255,
            g: 255,
            b: 255,
        });
        let source = VecImageBuffer::<ArgbPixel>::from_raw(176, 200, vec![white; 176 * 200]);

        let frame = LcdPlatform::fit(&source, 240, 320);

        assert_eq!((frame.width(), frame.height()), (240, 320));
        assert_eq!(frame.get_pixel(0, 23).r, 0);
        assert_eq!(frame.get_pixel(0, 24).r, 255);
        assert_eq!(frame.get_pixel(239, 295).r, 255);
        assert_eq!(frame.get_pixel(239, 296).r, 0);
    }

    #[test]
    fn fit_doubles_the_first_generation_lcd() {
        // 120x143 → 240x286 (정수 2배), 위아래 17줄
        let white = ArgbPixel::from_color(Color {
            a: 255,
            r: 255,
            g: 255,
            b: 255,
        });
        let source = VecImageBuffer::<ArgbPixel>::from_raw(120, 143, vec![white; 120 * 143]);

        let frame = LcdPlatform::fit(&source, 240, 320);

        assert_eq!(frame.get_pixel(0, 16).r, 0);
        assert_eq!(frame.get_pixel(0, 17).r, 255);
        assert_eq!(frame.get_pixel(239, 302).r, 255);
        assert_eq!(frame.get_pixel(239, 303).r, 0);
    }
}
