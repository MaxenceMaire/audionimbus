mod direct;
pub use direct::*;
mod reflections_reverb;
pub use reflections_reverb::*;

/// Defines the simulation logic for a single step.
pub trait SimulationStep<I>: Send + 'static
where
    I: Send + Sync + 'static,
{
    type Output: Send + Sync + 'static;

    fn run(&mut self, input: &I, output: &mut Self::Output);
}
