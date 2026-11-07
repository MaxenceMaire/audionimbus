//! Shared worker loop and frame allocation contracts.

#[cfg(feature = "dynamic")]
use crate::binding::Dynamic;
#[cfg(static_api)]
use crate::binding::Static;
#[cfg(feature = "dynamic")]
use crate::geometry::scene::RayTracerKind;
use crate::geometry::scene::{GeometryBinding, Scene as BoundScene};
use crate::wiring::binding::InputBinding;
use crate::wiring::step::SimulationStep;
use arc_swap::ArcSwap;
use object_pool::{Pool, ReusableOwned};
use std::collections::{HashMap, HashSet};
use std::hash::Hash;
use std::sync::{
    Arc, Condvar, Mutex,
    atomic::{AtomicBool, Ordering},
};

/// Drives a [`SimulationStep`] on a dedicated thread.
#[cfg(static_api)]
pub type StaticSimulationRunner<I, O, T> = SimulationRunner<I, O, Static<T>>;

/// A simulation runner accepting runtime scene commits.
#[cfg(feature = "dynamic")]
pub type DynamicSimulationRunner<I, O> = SimulationRunner<I, O, Dynamic<RayTracerKind>>;

/// A worker loop with scene storage selected by its geometry binding.
pub struct SimulationRunner<I, O, B: GeometryBinding> {
    /// The latest input frame.
    input: Arc<ArcSwap<I>>,
    /// The published simulation output.
    output: Arc<ArcSwap<ReusableOwned<O>>>,
    /// The pending simulator commit flag.
    simulator_commit_needed: Arc<AtomicBool>,
    /// The simulator commit callback.
    on_simulator_commit: Box<dyn FnMut() + Send + 'static>,
    /// The scenes awaiting a commit.
    pending_scene_commits: Arc<ArcSwap<HashSet<BoundScene<B>>>>,
    /// The shutdown request.
    shutdown: Arc<AtomicBool>,
    /// The pause flag and wake condition.
    paused: Arc<(Mutex<bool>, Condvar)>,
}

impl<I, O, B> SimulationRunner<I, O, B>
where
    I: Send + Sync + 'static,
    O: Send + Sync + 'static + Default + Clear + Shrink,
    B: GeometryBinding + 'static,
    BoundScene<B>: Send + Sync,
{
    /// Creates a new simulation runner.
    ///
    /// # Arguments
    ///
    /// - `input`: shared input frame updated each frame.
    /// - `output`: shared output written after each simulation run.
    /// - `simulator_commit_needed`: set to `true` to trigger a simulator commit on the next run.
    /// - `on_simulator_commit`: called when the commit flag is set to `true`.
    /// - `pending_scene_commits`: scenes pending a commit.
    /// - `shutdown`: set to `true` to stop the thread after its current iteration.
    /// - `paused`: set to `true` to pause the thread after its current iteration.
    pub fn new(
        input: Arc<ArcSwap<I>>,
        output: Arc<ArcSwap<ReusableOwned<O>>>,
        simulator_commit_needed: Arc<AtomicBool>,
        on_simulator_commit: impl FnMut() + Send + 'static,
        pending_scene_commits: Arc<ArcSwap<HashSet<BoundScene<B>>>>,
        shutdown: Arc<AtomicBool>,
        paused: Arc<(Mutex<bool>, Condvar)>,
    ) -> Self {
        Self {
            input,
            output,
            simulator_commit_needed,
            on_simulator_commit: Box::new(on_simulator_commit),
            pending_scene_commits,
            shutdown,
            paused,
        }
    }

    /// Spawns a new simulation thread and returns its handle.
    pub fn spawn<S, E>(
        self,
        mut step: S,
        on_error: impl Fn(E) + Send + 'static,
    ) -> std::thread::JoinHandle<()>
    where
        I: Resolve,
        for<'a> S: SimulationStep<<I as Resolve>::Resolved<'a>, Output = O, Error = E>,
        S: Send + 'static,
        for<'a> O: Allocate<<I as Resolve>::Resolved<'a>>,
        E: Send + 'static,
    {
        let Self {
            input,
            output,
            simulator_commit_needed,
            mut on_simulator_commit,
            pending_scene_commits,
            shutdown,
            paused,
        } = self;

        std::thread::spawn(move || {
            let pool = Arc::new(Pool::new(4, O::default));
            let empty_scene_commits = Arc::new(HashSet::new());

            loop {
                if shutdown.load(Ordering::Relaxed) {
                    break;
                }

                {
                    let (lock, condvar) = &*paused;
                    let mut is_paused = lock.lock().unwrap();
                    while *is_paused {
                        is_paused = condvar.wait(is_paused).unwrap();
                    }
                }

                let scenes_to_commit = pending_scene_commits.swap(Arc::clone(&empty_scene_commits));
                for scene in scenes_to_commit.iter() {
                    scene.commit();
                }

                // The first thread to catch the flag commits.
                if simulator_commit_needed
                    .compare_exchange(true, false, Ordering::Acquire, Ordering::Relaxed)
                    .is_ok()
                {
                    on_simulator_commit();
                }

                let frame = input.load();
                let resolved = frame.resolve();
                let mut out = pool.pull_owned(|| O::allocate(&resolved));
                out.clear();

                match step.run(&resolved, &mut out) {
                    Ok(()) => {
                        out.shrink();
                        output.store(Arc::new(out));
                    }
                    Err(error) => on_error(error),
                }
            }
        })
    }
}

/// Types that can be cleared.
pub trait Clear {
    /// Clears the type.
    fn clear(&mut self);
}

impl<T> Clear for Vec<T> {
    fn clear(&mut self) {
        Vec::clear(self);
    }
}

impl<K, V> Clear for HashMap<K, V> {
    fn clear(&mut self) {
        HashMap::clear(self);
    }
}

/// Types whose capacity can be shrunk.
pub trait Shrink {
    /// Shrinks the capacity of the type.
    fn shrink(&mut self);
}

impl<T> Shrink for Vec<T> {
    fn shrink(&mut self) {
        if self.capacity() > self.len() * 3 {
            self.shrink_to_fit();
        }
    }
}

impl<K, V> Shrink for HashMap<K, V>
where
    K: Eq + Hash,
{
    fn shrink(&mut self) {
        if self.capacity() > self.len() * 2 {
            self.shrink_to_fit();
        }
    }
}

/// Types that can be preallocated.
pub trait Allocate<Input> {
    /// Allocates a fresh output buffer, using the input as a capacity hint.
    fn allocate(input: &Input) -> Self;
}

/// Types that can be resolved into a borrowed frame.
///
/// Used to keep sources alive for the duration of the simulation.
pub trait Resolve {
    /// The resolved type, which borrows from `self` for lifetime `'a`.
    type Resolved<'a>
    where
        Self: 'a;

    /// Returns a borrowed frame.
    fn resolve(&self) -> Self::Resolved<'_>;
}

/// A guard holding the source list alive for the duration of a simulation step.
pub type SourcesGuard<SourceId, B> =
    arc_swap::Guard<Arc<ReusableOwned<<B as InputBinding>::Sources<SourceId>>>>;
