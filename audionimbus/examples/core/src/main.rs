mod audio;
mod consts;
mod dsp;
mod game_loop;
mod output;
mod room;
mod simulation;

fn main() {
    let simulation::SpawnedSimulations {
        audio_setup,
        simulation,
        source,
        listener_source,
        direct_simulation,
        reflections_reverb_simulation,
    } = simulation::spawn_simulations();

    let direct_output = direct_simulation.output().clone();
    let reflections_reverb_output = reflections_reverb_simulation.output().clone();

    let source_angle = game_loop::spawn_game_loop(
        simulation,
        source,
        listener_source,
        direct_simulation,
        reflections_reverb_simulation,
    );

    let _stream = audio::spawn_audio_thread(
        audio_setup,
        source_angle,
        direct_output,
        reflections_reverb_output,
    );

    std::thread::park();
}
