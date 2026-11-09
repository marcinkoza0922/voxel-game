//! Optional Linux PulseAudio/PipeWire playback via paplay; no gameplay-thread I/O.
use crate::feedback::Cue;
use std::{
    io::Write,
    process::{Command, Stdio},
    sync::mpsc::{self, SyncSender},
    time::{Duration, Instant},
};
pub struct Audio {
    sender: SyncSender<(Cue, Instant)>,
}
impl Audio {
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::sync_channel::<(Cue, Instant)>(8);
        std::thread::spawn(move || {
            let Ok(mut child) = Command::new("paplay")
                .args([
                    "--raw",
                    "--rate=22050",
                    "--channels=1",
                    "--format=s16le",
                    "--latency-msec=30",
                ])
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
            else {
                eprintln!("Audio unavailable: install paplay for sound playback");
                return;
            };
            let mut output = child.stdin.take().expect("piped audio input");
            stream_audio(receiver, &mut output);
            drop(output);
            let _ = child.wait();
        });
        Self { sender }
    }
    pub fn play(&self, cue: Cue) {
        let _ = self.sender.try_send((cue, Instant::now()));
    }
}

fn stream_audio(receiver: mpsc::Receiver<(Cue, Instant)>, output: &mut impl Write) {
    let mut voices: Vec<(Cue, usize)> = Vec::new();
    let mut noise = 1234567u32;
    let mut deadline = Instant::now();
    loop {
        // The pipe can buffer seconds of PCM. Never use its backpressure
        // as a clock, and never catch up missed deadlines with a burst.
        std::thread::sleep(deadline.saturating_duration_since(Instant::now()));
        let now = Instant::now();
        deadline = next_chunk(now);
        let mut disconnected = false;
        loop {
            match receiver.try_recv() {
                Ok((cue, emitted)) => {
                    if cue_is_fresh(emitted, now) && voices.len() < 12 {
                        voices.push((cue, 0));
                    }
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    disconnected = true;
                    break;
                }
            }
        }
        if disconnected {
            break;
        }
        let mut bytes = Vec::with_capacity(882);
        for _ in 0..441 {
            let mut sample = 0.;
            for (cue, age) in &mut voices {
                let t = *age as f32 / 22050.;
                let (frequency, decay, grit) = match cue {
                    Cue::Stone => (950., 45., 0.65),
                    Cue::Wood => (180., 35., 0.3),
                    Cue::Earth => (100., 40., 0.85),
                    Cue::Break => (85., 18., 0.8),
                    Cue::Hit => (130., 25., 0.55),
                    Cue::Hurt => (65., 14., 0.3),
                    Cue::Swing => (420., 40., 1.),
                    Cue::Pickup => (1050. + 3500. * t.min(0.08), 32., 0.02),
                };
                noise = noise.wrapping_mul(1664525).wrapping_add(1013904223);
                let random = (noise >> 8) as f32 / 8388608. - 1.;
                sample += ((t * frequency * std::f32::consts::TAU).sin() * (1. - grit)
                    + random * grit)
                    * (-t * decay).exp()
                    * 0.18;
                *age += 1;
            }
            bytes.extend_from_slice(&((sample.clamp(-1., 1.) * 32767.) as i16).to_le_bytes());
        }
        voices.retain(|(_, age)| *age < 6615);
        if output.write_all(&bytes).is_err() {
            break;
        }
    }
}

// 441 mono frames at 22,050 Hz represent exactly 20 milliseconds.
const CHUNK_DURATION: Duration = Duration::from_millis(20);
const MAX_CUE_AGE: Duration = Duration::from_millis(100);
fn next_chunk(now: Instant) -> Instant {
    now + CHUNK_DURATION
}
fn cue_is_fresh(emitted: Instant, now: Instant) -> bool {
    now.saturating_duration_since(emitted) <= MAX_CUE_AGE
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fast_sink_cannot_accumulate_future_audio() {
        struct FastSink {
            writes: usize,
        }
        impl Write for FastSink {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                self.writes += 1;
                if self.writes > 10 {
                    return Err(std::io::ErrorKind::BrokenPipe.into());
                }
                Ok(bytes.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let (_sender, receiver) = mpsc::sync_channel(8);
        let start = Instant::now();
        let mut sink = FastSink { writes: 0 };
        stream_audio(receiver, &mut sink);
        // A freely accepting pipe must still take real time to receive 200ms PCM.
        assert!(start.elapsed() >= Duration::from_millis(200));
        assert_eq!(sink.writes, 11);
    }
    #[test]
    fn delayed_cues_are_discarded() {
        let emitted = Instant::now();
        assert!(cue_is_fresh(emitted, emitted + CHUNK_DURATION));
        assert!(!cue_is_fresh(emitted, emitted + Duration::from_millis(101)));
    }
}
