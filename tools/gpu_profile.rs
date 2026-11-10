//! Bounded asynchronous GPU timestamps, correlated with CPU CSV frame IDs.
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::Sender,
};

pub type Sample = (u32, Option<[f64; 6]>);
struct Slot {
    queries: wgpu::QuerySet,
    resolve: wgpu::Buffer,
    readback: wgpu::Buffer,
    busy: Arc<AtomicBool>,
}
pub struct GpuProfile {
    slots: Vec<Slot>,
    sender: Sender<Sample>,
    period: f64,
}
impl GpuProfile {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, sender: Sender<Sample>) -> Option<Self> {
        let features =
            wgpu::Features::TIMESTAMP_QUERY | wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS;
        if !device.features().contains(features) {
            eprintln!("GPU profiling unavailable: encoder timestamps unsupported");
            return None;
        }
        let slots = (0..4)
            .map(|_| Slot {
                queries: device.create_query_set(&wgpu::QuerySetDescriptor {
                    label: Some("Frame timestamps"),
                    ty: wgpu::QueryType::Timestamp,
                    count: 6,
                }),
                resolve: device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("Timestamp resolve"),
                    size: 48,
                    usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
                    mapped_at_creation: false,
                }),
                readback: device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("Timestamp readback"),
                    size: 48,
                    usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }),
                busy: Arc::new(AtomicBool::new(false)),
            })
            .collect();
        Some(Self {
            slots,
            sender,
            period: queue.get_timestamp_period() as f64,
        })
    }
    pub fn begin(&self, device: &wgpu::Device, frame: u32) -> Result<Option<usize>, String> {
        device
            .poll(wgpu::PollType::Poll)
            .map_err(|e| e.to_string())?;
        let slot = self
            .slots
            .iter()
            .position(|s| !s.busy.load(Ordering::Acquire));
        if slot.is_none() {
            let _ = self.sender.send((frame, None));
        }
        Ok(slot)
    }
    pub fn stamp(&self, encoder: &mut wgpu::CommandEncoder, slot: usize, index: u32) {
        encoder.write_timestamp(&self.slots[slot].queries, index);
    }
    pub fn resolve(&self, encoder: &mut wgpu::CommandEncoder, slot: usize) {
        let s = &self.slots[slot];
        encoder.resolve_query_set(&s.queries, 0..6, &s.resolve, 0);
        encoder.copy_buffer_to_buffer(&s.resolve, 0, &s.readback, 0, 48);
    }
    pub fn submitted(&self, slot: usize, frame: u32) {
        let s = &self.slots[slot];
        s.busy.store(true, Ordering::Release);
        let buffer = s.readback.clone();
        let busy = s.busy.clone();
        let sender = self.sender.clone();
        let period = self.period;
        s.readback
            .map_async(wgpu::MapMode::Read, .., move |result| {
                let sample = result.ok().map(|()| {
                    let data = buffer.slice(..).get_mapped_range();
                    let ticks: Vec<u64> = data
                        .as_chunks::<8>()
                        .0
                        .iter()
                        .map(|b| u64::from_ne_bytes(*b))
                        .collect();
                    let mut ms = [0.; 6];
                    for i in 0..5 {
                        ms[i] = ticks[i + 1].wrapping_sub(ticks[i]) as f64 * period / 1_000_000.;
                    }
                    ms[5] = ticks[5].wrapping_sub(ticks[0]) as f64 * period / 1_000_000.;
                    drop(data);
                    buffer.unmap();
                    ms
                });
                let _ = sender.send((frame, sample));
                busy.store(false, Ordering::Release);
            });
    }
}
