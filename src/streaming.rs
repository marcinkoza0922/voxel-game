//! Bounded CPU workers. GPU resources and authoritative world state stay on the main thread.
use crate::{
    render::{Vertex, mesh_halo},
    world::{Block, SIZE, World},
};
use glam::IVec3;
use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::{Arc, Condvar, Mutex, mpsc},
};

pub const WORKERS: usize = 2;
// Job snapshots, finished CPU meshes and pending GPU uploads have separate bounds.
pub const MAX_OUTSTANDING: usize = 64;
pub const UPLOAD_BACKLOG: usize = 32;
struct Queue {
    jobs: VecDeque<Job>,
    shutdown: bool,
}
pub struct Job {
    pub key: IVec3,
    pub revision: u64,
    pub snapshot: World,
}
pub struct Completed {
    pub key: IVec3,
    pub revision: u64,
    pub blocks: Box<[Block]>,
    pub fluid_frontier: Vec<[i32; 3]>,
    pub physics_frontier: Vec<[i32; 3]>,
    pub roofs: HashMap<(i32, i32), i32>,
    pub mesh: Vec<Vertex>,
}
pub struct Workers {
    queue: Arc<(Mutex<Queue>, Condvar)>,
    receiver: Option<mpsc::Receiver<Completed>>,
    threads: Vec<std::thread::JoinHandle<()>>,
}
impl Workers {
    pub fn new() -> Self {
        let queue = Arc::new((
            Mutex::new(Queue {
                jobs: VecDeque::new(),
                shutdown: false,
            }),
            Condvar::new(),
        ));
        let (results, receiver) = mpsc::sync_channel(UPLOAD_BACKLOG);
        let threads = (0..WORKERS)
            .map(|i| {
                let queue = Arc::clone(&queue);
                let results = results.clone();
                std::thread::Builder::new()
                    .name(format!("terrain-{i}"))
                    .spawn(move || {
                        loop {
                            let job = {
                                let (lock, ready) = &*queue;
                                let mut state = lock.lock().expect("terrain job lock poisoned");
                                while state.jobs.is_empty() && !state.shutdown {
                                    state = ready.wait(state).expect("terrain job lock poisoned");
                                }
                                if state.shutdown {
                                    break;
                                }
                                state.jobs.pop_front().unwrap()
                            };
                            let halo = job.snapshot.halo(job.key);
                            let mut blocks = vec![Block::Air; 4096].into_boxed_slice();
                            let mut physics_frontier = Vec::new();
                            for y in 0..SIZE {
                                for z in 0..SIZE {
                                    for x in 0..SIZE {
                                        let block = halo
                                            [((x + 1) + 18 * ((z + 1) + 18 * (y + 1))) as usize];
                                        blocks[(x + SIZE * (z + SIZE * y)) as usize] = block;
                                        if matches!(
                                            block,
                                            Block::Sand | Block::Gravel | Block::Leaves
                                        ) {
                                            physics_frontier.push(
                                                (job.key * SIZE + IVec3::new(x, y, z)).to_array(),
                                            );
                                        }
                                    }
                                }
                            }
                            let mut mesh = mesh_halo(
                                job.snapshot.seed,
                                job.key,
                                &halo,
                                &job.snapshot.torch_mounts,
                            );
                            let roofs =
                                crate::render::light_chunk(&job.snapshot, job.key, &mut mesh);
                            // Stable partition preserves triangle order within each pass.
                            crate::render::prepare_chunk_mesh(&mut mesh);
                            if results
                                .send(Completed {
                                    key: job.key,
                                    revision: job.revision,
                                    blocks,
                                    fluid_frontier: crate::fluid::exposed_frontier(job.key, &halo),
                                    physics_frontier,
                                    roofs,
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
            queue,
            receiver: Some(receiver),
            threads,
        }
    }
    pub fn submit(&self, job: Job) {
        let (lock, ready) = &*self.queue;
        let mut state = lock.lock().expect("terrain job lock poisoned");
        assert!(state.jobs.len() < MAX_OUTSTANDING);
        state.jobs.push_back(job);
        ready.notify_one();
    }
    pub fn cancel_pending(&self) -> Vec<IVec3> {
        self.queue
            .0
            .lock()
            .expect("terrain job lock poisoned")
            .jobs
            .drain(..)
            .map(|job| job.key)
            .collect()
    }
    pub fn receive(&self) -> Option<Completed> {
        match self.receiver.as_ref().unwrap().try_recv() {
            Ok(result) => Some(result),
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => panic!("terrain workers stopped unexpectedly"),
        }
    }
}
impl Drop for Workers {
    fn drop(&mut self) {
        {
            let mut state = self.queue.0.lock().expect("terrain job lock poisoned");
            state.shutdown = true;
            state.jobs.clear();
        }
        self.queue.1.notify_all();
        // Unblock workers waiting on a full completed-results channel before joining.
        self.receiver.take();
        for thread in self.threads.drain(..) {
            let _ = thread.join();
        }
    }
}

type Bounds = HashMap<(i32, i32), (i32, i32)>;
struct PlanRequest {
    seed: u64,
    center: IVec3,
    radius: i32,
    bounds: Bounds,
    waterways: crate::hydrology::SharedCache,
}
pub struct Plan {
    pub center: IVec3,
    pub radius: i32,
    pub bounds: Bounds,
    pub desired: HashSet<IVec3>,
}
/// One outstanding request/result; the caller rejects plans for obsolete views.
pub struct Planner {
    sender: Option<mpsc::SyncSender<PlanRequest>>,
    receiver: mpsc::Receiver<Plan>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Planner {
    pub fn new() -> Self {
        let (sender, requests) = mpsc::sync_channel::<PlanRequest>(1);
        let (results, receiver) = mpsc::sync_channel(1);
        let thread = std::thread::Builder::new()
            .name("terrain-plan".into())
            .spawn(move || {
                while let Ok(request) = requests.recv() {
                    let mut world = World::new(request.seed);
                    world.surface_bounds = request.bounds;
                    world.waterways = request.waterways;
                    let desired = world.plan_surface(request.center, request.radius);
                    if results
                        .send(Plan {
                            center: request.center,
                            radius: request.radius,
                            bounds: world.surface_bounds,
                            desired,
                        })
                        .is_err()
                    {
                        break;
                    }
                }
            })
            .expect("could not start terrain planner");
        Self {
            sender: Some(sender),
            receiver,
            thread: Some(thread),
        }
    }
    pub fn request(
        &self,
        seed: u64,
        center: IVec3,
        radius: i32,
        bounds: Bounds,
        waterways: crate::hydrology::SharedCache,
    ) {
        self.sender
            .as_ref()
            .unwrap()
            .try_send(PlanRequest {
                seed,
                center,
                radius,
                bounds,
                waterways,
            })
            .unwrap_or_else(|_| panic!("terrain planner queue unavailable"));
    }
    pub fn receive(&self) -> Option<Plan> {
        match self.receiver.try_recv() {
            Ok(plan) => Some(plan),
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => panic!("terrain planner stopped unexpectedly"),
        }
    }
}
impl Drop for Planner {
    fn drop(&mut self) {
        self.sender.take();
        // Only one plan is outstanding, so sending its result cannot block.
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn workers_fill_bounded_results_without_redraw_and_drop_when_full() {
        let workers = Workers::new();
        for x in 0..MAX_OUTSTANDING {
            workers.submit(Job {
                key: IVec3::new(x as i32, 100, 0),
                revision: 0,
                snapshot: World::new(42),
            });
        }
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        // No receive/poll/redraw: workers must progress until backpressure stops them.
        while workers.queue.0.lock().unwrap().jobs.len()
            > MAX_OUTSTANDING - UPLOAD_BACKLOG - WORKERS
        {
            assert!(
                std::time::Instant::now() < deadline,
                "workers did not fill results"
            );
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        // Drop must release blocked result sends before joining threads.
        drop(workers);
    }
}
