use serde::{Deserialize, Serialize};
use std::path::Path;

pub const CAPS: [u32; 8] = [0, 30, 60, 90, 120, 144, 165, 240];
pub const RESOLUTIONS: [(u32, u32); 5] =
    [(0, 0), (1280, 720), (1600, 900), (1920, 1080), (2560, 1440)];
pub const SCALES: [u32; 5] = [50, 67, 75, 85, 100];
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum UpscaleQuality {
    #[default]
    #[serde(alias = "Custom")]
    Quality,
    Balanced,
    Performance,
    Native,
}
impl UpscaleQuality {
    fn next(self) -> Self {
        match self {
            Self::Quality => Self::Balanced,
            Self::Balanced => Self::Performance,
            Self::Performance => Self::Native,
            Self::Native => Self::Quality,
        }
    }
    fn label(self) -> &'static str {
        match self {
            Self::Quality => "Quality",
            Self::Balanced => "Balanced",
            Self::Performance => "Performance",
            Self::Native => "Native AA",
        }
    }
    fn ratio(self) -> f64 {
        match self {
            Self::Quality => 1. / 1.5,
            Self::Balanced => 1. / 1.7,
            Self::Performance => 0.5,
            Self::Native => 1.,
        }
    }
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub upscaler: u8,
    pub upscale_quality: UpscaleQuality,
    pub taa: bool,
    pub vsync: bool,
    pub cap: usize,
    pub resolution: usize,
    pub scale: usize,
    pub sound: bool,
    pub fullscreen: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            upscaler: 0,
            upscale_quality: UpscaleQuality::Quality,
            taa: true,
            vsync: true,
            cap: 0,
            resolution: 0,
            scale: 4,
            sound: true,
            fullscreen: false,
        }
    }
}
impl Settings {
    pub fn read(path: &Path) -> Result<Self, String> {
        let bytes = match std::fs::read(path) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(e) => return Err(e.to_string()),
        };
        let s: Self = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        if s.upscaler > 2
            || s.cap >= CAPS.len()
            || s.resolution >= RESOLUTIONS.len()
            || s.scale >= SCALES.len()
        {
            return Err("Invalid graphics settings; using defaults".into());
        }
        Ok(s)
    }
    pub fn write(&self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let temp = path.with_extension("json.tmp");
        std::fs::write(
            &temp,
            serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        std::fs::rename(temp, path).map_err(|e| e.to_string())
    }
    pub fn change(&mut self, n: u8) {
        match n {
            0 => self.taa = !self.taa,
            1 => self.vsync = !self.vsync,
            2 => self.cap = (self.cap + 1) % CAPS.len(),
            3 if self.upscaler == 0 => {
                self.resolution = (self.resolution + 1) % RESOLUTIONS.len();
            }
            4 if self.upscaler == 0 => {
                self.scale = (self.scale + 1) % SCALES.len();
            }
            5 => self.fullscreen = !self.fullscreen,
            6 => self.sound = !self.sound,
            7 => {
                if self.upscaler == 0 {
                    self.upscale_quality = UpscaleQuality::Quality;
                }
                self.upscaler = (self.upscaler + 1) % 3;
            }
            8 if self.upscaler != 0 => self.upscale_quality = self.upscale_quality.next(),
            _ => (),
        }
    }
    pub fn render_size(&self, width: u32, height: u32) -> (u32, u32) {
        if self.upscaler != 0 {
            let ratio = self.upscale_quality.ratio();
            return (
                ((width as f64 * ratio).round() as u32).max(1),
                ((height as f64 * ratio).round() as u32).max(1),
            );
        }
        let (w, h) = RESOLUTIONS[self.resolution];
        let (w, h) = if w == 0 { (width, height) } else { (w, h) };
        // Fit the resolution preset to the output aspect ratio without stretching.
        let fit = (w as f64 / width as f64).min(h as f64 / height as f64);
        let scale = fit * SCALES[self.scale] as f64 / 100.;
        (
            ((width as f64 * scale).round() as u32).max(1),
            ((height as f64 * scale).round() as u32).max(1),
        )
    }
    pub fn labels(&self) -> Vec<String> {
        let on = |b| if b { "On" } else { "Off" };
        let (w, h) = RESOLUTIONS[self.resolution];
        vec![
            if self.upscaler == 0 {
                format!("Temporal AA: {}", on(self.taa))
            } else {
                format!(
                    "Temporal AA: {}",
                    ["", "DLSS", "FSR 2"][self.upscaler as usize]
                )
            },
            format!("VSync: {}", on(self.vsync)),
            format!(
                "Frame cap: {}",
                if CAPS[self.cap] == 0 {
                    "Uncapped".into()
                } else {
                    format!("{} FPS", CAPS[self.cap])
                }
            ),
            if self.upscaler != 0 {
                "Render resolution: Upscaler controlled".into()
            } else {
                format!(
                    "Render resolution: {}",
                    if w == 0 {
                        "Native".into()
                    } else {
                        format!("{w} x {h}")
                    }
                )
            },
            if self.upscaler != 0 {
                "Render scale: Upscaler controlled".into()
            } else {
                format!("Render scale: {}%", SCALES[self.scale])
            },
            format!("Fullscreen: {}", on(self.fullscreen)),
            format!("Sound: {}", on(self.sound)),
            format!(
                "Upscaler: {}",
                ["Off", "DLSS", "FSR 2"][self.upscaler as usize]
            ),
            format!("Upscaler quality: {}", self.upscale_quality.label()),
        ]
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn named_presets_control_size_and_lock_manual_controls() {
        let mut s = Settings {
            upscaler: 1,
            resolution: 1,
            scale: 0,
            upscale_quality: UpscaleQuality::Quality,
            ..Settings::default()
        };
        for (quality, expected) in [
            (UpscaleQuality::Quality, (1280, 720)),
            (UpscaleQuality::Balanced, (1129, 635)),
            (UpscaleQuality::Performance, (960, 540)),
            (UpscaleQuality::Native, (1920, 1080)),
        ] {
            s.upscale_quality = quality;
            assert_eq!(s.render_size(1920, 1080), expected);
            assert_eq!(s.render_size(1, 1), (1, 1));
        }
        s.change(8);
        assert_eq!(s.upscale_quality, UpscaleQuality::Quality);
        for upscaler in [1, 2] {
            s.upscaler = upscaler;
            s.change(3);
            s.change(4);
            assert_eq!((s.resolution, s.scale), (1, 0));
            assert_eq!(s.render_size(1920, 1080), (1280, 720));
        }
        s.upscaler = 0;
        assert_eq!(s.render_size(1920, 1080), (640, 360));
        s.change(3);
        s.change(4);
        assert_eq!((s.resolution, s.scale), (2, 1));
        for json in [
            r#"{"upscaler":1,"scale":0}"#,
            r#"{"upscaler":1,"upscale_quality":"Custom"}"#,
        ] {
            let legacy: Settings = serde_json::from_str(json).unwrap();
            assert_eq!(legacy.upscale_quality, UpscaleQuality::Quality);
            assert_eq!(legacy.render_size(1920, 1080), (1280, 720));
        }
    }
    #[test]
    fn scaled_resolution_preserves_output_aspect_and_never_zero() {
        let mut s = Settings::default();
        assert_eq!(s.render_size(1280, 800), (1280, 800));
        s.scale = 0;
        assert_eq!(s.render_size(1280, 800), (640, 400));
        s.resolution = 1;
        assert_eq!(s.render_size(1920, 1080), (640, 360));
        assert_eq!(s.render_size(1, 1), (360, 360));
    }
}

#[cfg(test)]
mod persistence_tests {
    use super::*;
    #[test]
    fn persisted_graphics_settings_roundtrip_and_reject_invalid_indices() {
        let path = std::env::temp_dir().join(format!("voxel-settings-{}.json", std::process::id()));
        let settings = Settings {
            fullscreen: true,
            vsync: false,
            cap: 5,
            scale: 1,
            upscaler: 2,
            upscale_quality: UpscaleQuality::Balanced,
            ..Settings::default()
        };
        settings.write(&path).unwrap();
        let read = Settings::read(&path).unwrap();
        assert!(read.fullscreen && !read.vsync);
        assert_eq!(read.upscale_quality, UpscaleQuality::Balanced);
        assert_eq!((read.cap, read.scale, read.upscaler), (5, 1, 2));
        std::fs::write(&path, r#"{"scale":99}"#).unwrap();
        assert!(Settings::read(&path).is_err());
        std::fs::remove_file(path).unwrap();
    }
}
