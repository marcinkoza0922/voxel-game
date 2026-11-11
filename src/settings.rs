use serde::{Deserialize, Serialize};
use std::path::Path;

pub const CAPS: [u32; 8] = [0, 30, 60, 90, 120, 144, 165, 240];
pub const RESOLUTIONS: [(u32, u32); 5] =
    [(0, 0), (1280, 720), (1600, 900), (1920, 1080), (2560, 1440)];
pub const SCALES: [u32; 5] = [50, 67, 75, 85, 100];
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Aa {
    Off,
    Fxaa,
    Smaa,
    #[default]
    Taa,
}
impl Aa {
    fn next(self) -> Self {
        match self {
            Self::Off => Self::Fxaa,
            Self::Fxaa => Self::Smaa,
            Self::Smaa => Self::Taa,
            Self::Taa => Self::Off,
        }
    }
    fn label(self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::Fxaa => "FXAA",
            Self::Smaa => "SMAA",
            Self::Taa => "TAA",
        }
    }
}
fn read_aa<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Aa, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum SavedAa {
        Mode(Aa),
        LegacyTaa(bool),
    }
    Ok(match SavedAa::deserialize(deserializer)? {
        SavedAa::Mode(mode) => mode,
        SavedAa::LegacyTaa(true) => Aa::Taa,
        SavedAa::LegacyTaa(false) => Aa::Off,
    })
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    #[serde(alias = "taa", deserialize_with = "read_aa")]
    pub aa: Aa,
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
            aa: Aa::Taa,
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
        if s.cap >= CAPS.len() || s.resolution >= RESOLUTIONS.len() || s.scale >= SCALES.len() {
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
            0 => self.aa = self.aa.next(),
            1 => self.vsync = !self.vsync,
            2 => self.cap = (self.cap + 1) % CAPS.len(),
            3 => {
                self.resolution = (self.resolution + 1) % RESOLUTIONS.len();
            }
            4 => {
                self.scale = (self.scale + 1) % SCALES.len();
            }
            5 => self.fullscreen = !self.fullscreen,
            6 => self.sound = !self.sound,
            _ => (),
        }
    }
    pub fn render_size(&self, width: u32, height: u32) -> (u32, u32) {
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
            format!("Anti-aliasing: {}", self.aa.label()),
            format!("VSync: {}", on(self.vsync)),
            format!(
                "Frame cap: {}",
                if CAPS[self.cap] == 0 {
                    "Uncapped".into()
                } else {
                    format!("{} FPS", CAPS[self.cap])
                }
            ),
            format!(
                "Render resolution: {}",
                if w == 0 {
                    "Native".into()
                } else {
                    format!("{w} x {h}")
                }
            ),
            format!("Render scale: {}%", SCALES[self.scale]),
            format!("Fullscreen: {}", on(self.fullscreen)),
            format!("Sound: {}", on(self.sound)),
        ]
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_settings_keep_manual_scale_and_aa_preference() {
        let mut settings: Settings = serde_json::from_str(
            r#"{"upscaler":1,"upscale_quality":"Custom","taa":false,"resolution":1,"scale":0}"#,
        )
        .unwrap();
        assert_eq!(settings.aa, Aa::Off);
        assert_eq!(settings.render_size(1920, 1080), (640, 360));
        settings.change(3);
        settings.change(4);
        assert_eq!((settings.resolution, settings.scale), (2, 1));
        for mode in [Aa::Fxaa, Aa::Smaa, Aa::Taa, Aa::Off] {
            settings.change(0);
            assert_eq!(settings.aa, mode);
        }
        let legacy: Settings = serde_json::from_str(r#"{"upscaler":2,"taa":true}"#).unwrap();
        assert_eq!(legacy.aa, Aa::Taa);
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
            aa: Aa::Smaa,
            ..Settings::default()
        };
        settings.write(&path).unwrap();
        let read = Settings::read(&path).unwrap();
        assert!(read.fullscreen && !read.vsync);
        assert_eq!(read.aa, Aa::Smaa);
        assert_eq!((read.cap, read.scale), (5, 1));
        std::fs::write(&path, r#"{"scale":99}"#).unwrap();
        assert!(Settings::read(&path).is_err());
        std::fs::remove_file(path).unwrap();
    }
}
