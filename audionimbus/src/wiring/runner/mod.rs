use super::SimulationStep;
use arc_swap::ArcSwap;
use object_pool::{Pool, ReusableOwned};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
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
}

impl<I, O> SimulationRunner<I, O>
where
    I: Send + Sync + 'static,
    O: Send + Sync + 'static + Default + Clear + Shrink,
{
    /// Creates a new simulation runner.
    pub fn new(
        input: Arc<ArcSwap<I>>,
        output: Arc<ArcSwap<ReusableOwned<O>>>,
        commit_needed: Arc<AtomicBool>,
        on_commit: impl FnMut() + Send + 'static,
    ) -> Self {
        Self {
            input,
            output,
            commit_needed,
            on_commit: Box::new(on_commit),
        }
    }

    /// Spawns a new simulation thread and returns its handle.
    pub fn spawn<S>(self, mut step: S) -> std::thread::JoinHandle<()>
    where
        S: SimulationStep<I, Output = O>,
        O: Allocate<I>,
    {
        let Self {
            input,
            output,
            commit_needed,
            mut on_commit,
        } = self;

        std::thread::spawn(move || {
            let pool = Arc::new(Pool::new(4, O::default));

            loop {
                // The first thread to catch the flag commits.
                if commit_needed
                    .compare_exchange(true, false, Ordering::Acquire, Ordering::Relaxed)
                    .is_ok()
                {
                    on_commit();
                }

                let input = input.load();
                let mut out = pool.pull_owned(|| O::allocate(&**input));
                out.clear();
                step.run(&**input, &mut out);
                out.shrink();
                output.store(Arc::new(out));
            }
        })
    }
}

pub trait Clear {
    fn clear(&mut self);
}

impl<T> Clear for Vec<T> {
    fn clear(&mut self) {
        Vec::clear(self);
    }
}

pub trait Shrink {
    fn shrink(&mut self);
}

impl<T> Shrink for Vec<T> {
    fn shrink(&mut self) {
        if self.capacity() > self.len() * 3 {
            self.shrink_to_fit();
        }
    }
}

pub trait Allocate<Input> {
    /// Allocates a fresh output buffer, using the input as a capacity hint.
    fn allocate(input: &Input) -> Self;
}
