//! Bounded CPU workers. GPU resources and authoritative world state stay on the main thread.
use crate::{
    render::{Vertex, mesh_halo},
    world::{Block, SIZE, World},
};
use glam::IVec3;
use std::sync::{Arc, Mutex, mpsc};

pub const WORKERS: usize = 2;
pub struct Job {
    pub key: IVec3,
    pub revision: u64,
    pub snapshot: World,
}
pub struct Completed {
    pub key: IVec3,
    pub revision: u64,
    pub blocks: Box<[Block]>,
    pub mesh: Vec<Vertex>,
}
pub struct Workers {
    sender: Option<mpsc::SyncSender<Job>>,
    receiver: mpsc::Receiver<Completed>,
    threads: Vec<std::thread::JoinHandle<()>>,
}
impl Workers {
    pub fn new() -> Self {
        let (sender, jobs) = mpsc::sync_channel::<Job>(WORKERS);
        let (results, receiver) = mpsc::channel();
        let jobs = Arc::new(Mutex::new(jobs));
        let threads = (0..WORKERS)
            .map(|i| {
                let jobs = Arc::clone(&jobs);
                let results = results.clone();
                std::thread::Builder::new()
                    .name(format!("terrain-{i}"))
                    .spawn(move || {
                        loop {
                            let job = jobs.lock().expect("terrain job lock poisoned").recv();
                            let Ok(job) = job else { break };
                            let halo = job.snapshot.halo(job.key);
                            let mut blocks = vec![Block::Air; 4096].into_boxed_slice();
                            for y in 0..SIZE {
                                for z in 0..SIZE {
                                    for x in 0..SIZE {
                                        blocks[(x + SIZE * (z + SIZE * y)) as usize] = halo
                                            [((x + 1) + 18 * ((z + 1) + 18 * (y + 1))) as usize];
                                    }
                                }
                            }
                            let mesh = mesh_halo(
                                job.snapshot.seed,
                                job.key,
                                &halo,
                                &job.snapshot.torch_mounts,
                            );
                            if results
                                .send(Completed {
                                    key: job.key,
                                    revision: job.revision,
                                    blocks,
                                    mesh,
                                })
                                .is_err()
                            {
                                break;
                            }
                        }
                    })
                    .expect("could not start terrain worker")
            })
            .collect();
        Self {
            sender: Some(sender),
            receiver,
            threads,
        }
    }
    pub fn submit(&self, job: Job) {
        // The caller counts all outstanding jobs, so this bounded send cannot fill.
        self.sender
            .as_ref()
            .unwrap()
            .try_send(job)
            .unwrap_or_else(|_| panic!("terrain worker queue unavailable"));
    }
    pub fn receive(&self) -> Option<Completed> {
        match self.receiver.try_recv() {
            Ok(result) => Some(result),
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => panic!("terrain workers stopped unexpectedly"),
        }
    }
}
impl Drop for Workers {
    fn drop(&mut self) {
        self.sender.take();
        for thread in self.threads.drain(..) {
            let _ = thread.join();
        }
    }
}
