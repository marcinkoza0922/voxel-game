//! Optional CPU wall-time CSV and a separate asynchronous GPU timestamp CSV.
use std::{
    fs::File,
    io::{BufWriter, Write},
    path::Path,
};

pub struct Profile {
    writer: BufWriter<File>,
    gpu_writer: BufWriter<File>,
    gpu_tx: std::sync::mpsc::Sender<crate::gpu_profile::Sample>,
    gpu_rx: std::sync::mpsc::Receiver<crate::gpu_profile::Sample>,
}
impl Profile {
    pub fn new(path: &Path) -> Result<Self, String> {
        let mut writer = BufWriter::new(File::create(path).map_err(|e| e.to_string())?);
        writeln!(writer, "frame,interval_ms,simulation_ms,stream_upload_ms,draw_present_ms,resident,queued,active,focused,menu,draw_prepare_ms,surface_acquire_ms,draw_encode_ms,queue_submit_ms,present_capture_ms").map_err(|e| e.to_string())?;
        let mut gpu_writer = BufWriter::new(
            File::create(path.with_extension("gpu.csv")).map_err(|e| e.to_string())?,
        );
        writeln!(
            gpu_writer,
            "frame,gpu_shadow_ms,gpu_world_ms,gpu_temporal_ms,gpu_held_ms,gpu_ui_ms,gpu_total_ms"
        )
        .map_err(|e| e.to_string())?;
        let (gpu_tx, gpu_rx) = std::sync::mpsc::channel();
        Ok(Self {
            writer,
            gpu_writer,
            gpu_tx,
            gpu_rx,
        })
    }
    pub fn gpu_sender(&self) -> std::sync::mpsc::Sender<crate::gpu_profile::Sample> {
        self.gpu_tx.clone()
    }
    fn drain_gpu(&mut self) -> Result<(), String> {
        while let Ok((frame, sample)) = self.gpu_rx.try_recv() {
            write!(self.gpu_writer, "{frame}").map_err(|e| e.to_string())?;
            for value in sample
                .map(|s| s.map(|v| format!("{v:.6}")))
                .unwrap_or_default()
            {
                write!(self.gpu_writer, ",{value}").map_err(|e| e.to_string())?;
            }
            writeln!(self.gpu_writer).map_err(|e| e.to_string())?;
        }
        Ok(())
    }
    pub fn record(
        &mut self,
        frame: u32,
        times: [f64; 4],
        counts: [usize; 3],
        focused: bool,
        menu: bool,
        draw_times: [f64; 5],
    ) -> Result<(), String> {
        self.drain_gpu()?;
        let interval = if frame == 0 {
            String::new()
        } else {
            format!("{:.4}", times[0])
        };
        writeln!(
            self.writer,
            "{frame},{interval},{:.4},{:.4},{:.4},{},{},{},{focused},{menu},{:.4},{:.4},{:.4},{:.4},{:.4}",
            times[1], times[2], times[3], counts[0], counts[1], counts[2],
            draw_times[0], draw_times[1], draw_times[2], draw_times[3], draw_times[4]
        )
        .map_err(|e| e.to_string())
    }
    pub fn finish(&mut self) -> Result<(), String> {
        self.drain_gpu()?;
        self.gpu_writer.flush().map_err(|e| e.to_string())?;
        self.writer.flush().map_err(|e| e.to_string())
    }
}
