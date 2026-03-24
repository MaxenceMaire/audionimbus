//! Primitives for driving simulation steps on dedicated threads.
//!
//! The entry point is [`SimulationRunner`], which owns the thread loop, commit signalling, and
//! lock-free input/output.

use super::simulation::SourceWithInputs;
use super::step::SimulationStep;
use arc_swap::ArcSwap;
use object_pool::{Pool, ReusableOwned};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Condvar, Mutex,
};

mod direct;
pub use direct::*;
mod pathing;
pub use pathing::*;
mod reflections;
pub use reflections::*;
mod reflections_reverb;
pub use reflections_reverb::*;

/// Drives a [`SimulationStep`] on a dedicated thread.
pub struct SimulationRunner<I, O> {
    input: Arc<ArcSwap<I>>,
    output: Arc<ArcSwap<ReusableOwned<O>>>,
    commit_needed: Arc<AtomicBool>,
    on_commit: Box<dyn FnMut() + Send + 'static>,
    shutdown: Arc<AtomicBool>,
    paused: Arc<(Mutex<bool>, Condvar)>,
}

impl<I, O> SimulationRunner<I, O>
where
    I: Send + Sync + 'static,
    O: Send + Sync + 'static + Default + Clear + Shrink,
{
    /// Creates a new simulation runner.
    ///
    /// # Arguments
    ///
    /// - `input`: shared input frame updated each frame.
    /// - `output`: shared output written after each simulation run.
    /// - `commit_needed`: set to `true` to trigger a simulator commit on the next run.
    /// - `on_commit`: called when the commit flag is set to `true`.
    /// - `shutdown`: set to `true` to stop the thread after its current iteration.
    /// - `paused`: set to `true` to pause the thread after its current iteration.
    pub fn new(
        input: Arc<ArcSwap<I>>,
        output: Arc<ArcSwap<ReusableOwned<O>>>,
        commit_needed: Arc<AtomicBool>,
        on_commit: impl FnMut() + Send + 'static,
        shutdown: Arc<AtomicBool>,
        paused: Arc<(Mutex<bool>, Condvar)>,
    ) -> Self {
        Self {
            input,
            output,
            commit_needed,
            on_commit: Box::new(on_commit),
            shutdown,
            paused,
        }
    }

    /// Spawns a new simulation thread and returns its handle.
    pub fn spawn<S>(self, mut step: S) -> std::thread::JoinHandle<()>
    where
        I: Resolve,
        for<'a> S: SimulationStep<<I as Resolve>::Resolved<'a>, Output = O>,
        S: Send + 'static,
        for<'a> O: Allocate<<I as Resolve>::Resolved<'a>>,
    {
        let Self {
            input,
            output,
            commit_needed,
            mut on_commit,
            shutdown,
            paused,
        } = self;

        std::thread::spawn(move || {
            let pool = Arc::new(Pool::new(4, O::default));

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

                // The first thread to catch the flag commits.
                if commit_needed
                    .compare_exchange(true, false, Ordering::Acquire, Ordering::Relaxed)
                    .is_ok()
                {
                    on_commit();
                }

                let frame = input.load();
                let resolved = frame.resolve();
                let mut out = pool.pull_owned(|| O::allocate(&resolved));
                out.clear();
                step.run(&resolved, &mut out).unwrap();
                out.shrink();
                output.store(Arc::new(out));
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
pub(crate) type SourcesGuard<SourceId, D, R, P, RE> =
    arc_swap::Guard<Arc<ReusableOwned<Vec<(SourceId, SourceWithInputs<D, R, P, RE>)>>>>;
