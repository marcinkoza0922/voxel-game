//! Optional, bounded-memory frame diagnostics. Times are wall time, not GPU timestamps.
use std::{
    fs::File,
    io::{BufWriter, Write},
    path::Path,
};

pub struct Profile {
    writer: BufWriter<File>,
}
impl Profile {
    pub fn new(path: &Path) -> Result<Self, String> {
        let mut writer = BufWriter::new(File::create(path).map_err(|e| e.to_string())?);
        writeln!(writer, "frame,interval_ms,simulation_ms,stream_upload_ms,draw_present_ms,resident,queued,active,focused,menu").map_err(|e| e.to_string())?;
        Ok(Self { writer })
    }
    pub fn record(
        &mut self,
        frame: u32,
        times: [f64; 4],
        counts: [usize; 3],
        focused: bool,
        menu: bool,
    ) -> Result<(), String> {
        let interval = if frame == 0 {
            String::new()
        } else {
            format!("{:.4}", times[0])
        };
        writeln!(
            self.writer,
            "{frame},{interval},{:.4},{:.4},{:.4},{},{},{},{focused},{menu}",
            times[1], times[2], times[3], counts[0], counts[1], counts[2]
        )
        .map_err(|e| e.to_string())
    }
    pub fn finish(&mut self) -> Result<(), String> {
        self.writer.flush().map_err(|e| e.to_string())
    }
}
