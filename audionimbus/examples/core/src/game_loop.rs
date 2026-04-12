use crate::consts::{AMBISONICS_ORDER, LISTENER_HEIGHT};
use crate::output::{FRAME_SIZE, SAMPLE_RATE};
use audionimbus::wiring::*;
use audionimbus::*;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// Spawns the game loop thread and returns a shared source angle.
///
/// Each iteration of the loop:
/// 1. Advances the source angle by one frame's worth of rotation.
/// 2. Pushes the new source position into the simulation's source list.
/// 3. Forwards a complete [`DirectFrame`] and [`ReflectionsReverbFrame`] to their respective
///    worker threads.
///
/// The returned [`AtomicU32`] holds the current angle (read by the audio thread to derive the
/// binaural render direction).
pub fn spawn_game_loop<T, RE>(
    simulation: Simulation<(), T, Direct, Reflections, (), RE>,
    source: Source<Direct, Reflections, (), RE>,
    listener_source: Source<(), Reflections, (), RE>,
    direct_simulation: DirectSimulation<(), Reflections, (), RE>,
    reflections_reverb_simulation: ReflectionsReverbSimulation<(), Direct, (), RE, (), ()>,
) -> Arc<AtomicU32>
where
    T: RayTracer + Send + 'static,
    RE: ReflectionEffectType + Send + 'static,
    Simulation<(), T, Direct, Reflections, (), RE>: Send + 'static,
{
    let source_angle = Arc::new(AtomicU32::new(0_f32.to_bits()));
    let source_angle_for_thread = source_angle.clone();

    let frame_duration = Duration::from_secs_f32(FRAME_SIZE as f32 / SAMPLE_RATE as f32);

    std::thread::spawn(move || {
        let orbit_radius = 2.0_f32;
        let angular_speed = 1.0_f32; // rad/s

        // The listener stays fixed at the origin at ear height.
        let listener_transform = CoordinateSystem {
            origin: Point::new(0.0, LISTENER_HEIGHT, 0.0),
            ahead: Point::new(0.0, 0.0, -1.0),
            up: Point::new(0.0, 1.0, 0.0),
            right: Point::new(1.0, 0.0, 0.0),
        };

        let shared_inputs = SimulationSharedInputs::new(listener_transform)
            .with_direct()
            .with_reflections(ReflectionsSharedInputs {
                num_rays: 16_384,
                num_bounces: 32,
                duration: 2.0,
                order: AMBISONICS_ORDER,
                irradiance_min_distance: 1.0,
            });

        loop {
            let previous_angle = f32::from_bits(source_angle_for_thread.load(Ordering::Relaxed));
            let angle = (previous_angle + angular_speed * frame_duration.as_secs_f32())
                .rem_euclid(std::f32::consts::TAU);
            source_angle_for_thread.store(angle.to_bits(), Ordering::Relaxed);

            let source_x = angle.cos() * orbit_radius;
            let source_z = angle.sin() * orbit_radius;

            simulation.update_sources(|sources| {
                sources.push((
                    (),
                    SourceWithInputs {
                        source: source.clone(),
                        simulation_inputs: SimulationInputs {
                            source: CoordinateSystem {
                                origin: Point::new(source_x, LISTENER_HEIGHT, source_z),
                                ..CoordinateSystem::default()
                            },
                            parameters: SimulationParameters::new()
                                .with_direct(DirectSimulationParameters::new().with_occlusion(
                                    Occlusion::new(OcclusionAlgorithm::Raycast).with_transmission(
                                        TransmissionParameters {
                                            num_transmission_rays: 1,
                                        },
                                    ),
                                ))
                                .with_reflections(ConvolutionParameters {
                                    baked_data_identifier: None,
                                }),
                        },
                    },
                ));
            });

            direct_simulation.set_input(DirectFrame {
                sources: simulation.sources.clone(),
                shared_inputs: shared_inputs.clone(),
            });

            reflections_reverb_simulation.set_input(ReflectionsReverbFrame {
                sources: simulation.sources.clone(),
                listener: Some(SourceWithInputs {
                    source: listener_source.clone(),
                    simulation_inputs: SimulationInputs {
                        source: listener_transform,
                        parameters: SimulationParameters::new().with_reflections(
                            ConvolutionParameters {
                                baked_data_identifier: None,
                            },
                        ),
                    },
                }),
                shared_inputs: shared_inputs.clone(),
            });

            std::thread::sleep(frame_duration);
        }
    });

    source_angle
}
