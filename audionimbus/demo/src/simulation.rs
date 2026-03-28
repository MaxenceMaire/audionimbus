use crate::consts::{AMBISONICS_ORDER, IMPULSE_RESPONSE_DURATION};
use crate::output::{FRAME_SIZE, SAMPLE_RATE};
use audionimbus::wiring::*;
use audionimbus::*;

/// Audio context and settings forwarded to the audio thread.
pub struct AudioSetup {
    pub context: Context,
    pub audio_settings: AudioSettings,
    pub hrtf: Hrtf,
}

/// Handles produced by [`spawn_simulations`].
pub struct SpawnedSimulations {
    pub audio_setup: AudioSetup,
    pub simulation: Simulation<(), DefaultRayTracer, Direct, Reflections, (), Convolution>,
    pub source: Source<Direct, Reflections, (), Convolution>,
    pub listener_source: Source<(), Reflections, (), Convolution>,
    pub direct_simulation: DirectSimulation<(), Reflections, (), Convolution>,
    pub reflections_reverb_simulation:
        ReflectionsReverbSimulation<(), Direct, (), Convolution, (), ()>,
}

/// Builds the scene, configures the simulator, registers sources, and spawns the simulation worker
/// threads.
pub fn spawn_simulations() -> SpawnedSimulations {
    let context = Context::default();
    let audio_settings = AudioSettings {
        sampling_rate: SAMPLE_RATE,
        frame_size: FRAME_SIZE,
    };
    let hrtf = Hrtf::try_new(&context, &audio_settings, &HrtfSettings::default()).unwrap();

    let mut scene = Scene::try_new(&context).unwrap();
    let mesh = crate::room::room(
        &scene,
        30.0,
        10.0,
        30.0,
        Material {
            absorption: [0.08, 0.10, 0.12],
            scattering: 0.25,
            transmission: [0.0, 0.0, 0.0],
        },
    );
    scene.add_static_mesh(mesh);
    scene.commit();

    let mut simulator = Simulator::try_new(
        &context,
        &SimulationSettings::new(&audio_settings)
            .with_direct(DirectSimulationSettings {
                max_num_occlusion_samples: 32,
            })
            .with_reflections(ConvolutionSettings {
                max_num_rays: 16_384,
                num_diffuse_samples: 128,
                max_duration: IMPULSE_RESPONSE_DURATION,
                max_num_sources: 2,
                num_threads: 2,
                max_order: AMBISONICS_ORDER,
            }),
    )
    .unwrap();
    simulator.set_scene(&scene);

    // Orbiting point source.
    let source = Source::try_new(&simulator).unwrap();
    simulator.add_source(&source);

    // Listener source used for reverb.
    let listener_source =
        Source::<(), Reflections, (), Convolution>::try_new_subset(&simulator).unwrap();
    simulator.add_source(&listener_source);

    simulator.commit();

    let mut simulation = Simulation::new(simulator);
    simulation.request_commit();

    let direct_simulation = simulation.spawn_direct();
    let reflections_reverb_simulation = simulation.spawn_reflections_reverb::<(), ()>();

    SpawnedSimulations {
        audio_setup: AudioSetup {
            context,
            audio_settings,
            hrtf,
        },
        simulation,
        source,
        listener_source,
        direct_simulation,
        reflections_reverb_simulation,
    }
}
