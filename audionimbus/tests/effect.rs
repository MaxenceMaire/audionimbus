use audionimbus::*;

pub mod common;
use common::sine_wave;

#[cfg(static_api)]
/// Creates convolution parameters from a reflection simulation.
///
/// # Panics
///
/// Panics if native setup or simulation fails.
fn convolution_parameters(
    context: &Context,
    audio_settings: &AudioSettings,
) -> ReflectionEffectParams<Convolution> {
    let simulation_settings =
        SimulationSettings::new(audio_settings).with_reflections(ConvolutionSettings {
            max_num_rays: 4096,
            num_diffuse_samples: 32,
            max_duration: 2.0,
            max_num_sources: 8,
            num_threads: 1,
            max_order: 1,
        });
    let mut simulator = Simulator::try_new(context, &simulation_settings).unwrap();

    let scene = Scene::try_new(context).unwrap();
    simulator.set_scene(&scene);

    let source = Source::try_new(&simulator).unwrap();
    simulator.add_source(&source);

    let simulation_shared_inputs = SimulationSharedInputs::new(CoordinateSystem::default())
        .with_reflections(ReflectionsSharedInputs {
            num_rays: 4096,
            num_bounces: 16,
            duration: 2.0,
            order: 1,
            irradiance_min_distance: 1.0,
        });
    simulator
        .set_shared_reflections_inputs(&simulation_shared_inputs)
        .unwrap();

    simulator.commit();

    assert!(simulator.run_reflections().is_ok());

    source.get_reflections_outputs().unwrap()
}

#[test]
fn test_binaural_effect() {
    let frequency = 440.0;
    let amplitude = 0.5;
    let duration_secs = 0.1;
    let sample_rate = 48000;
    let sine_wave = sine_wave(frequency, amplitude, duration_secs, sample_rate);
    let input_buffer = AudioBufferRef::try_from(&sine_wave[..]).unwrap();
    let frame_size = sine_wave.len() as u32;

    let mut output_container = vec![0.0; 2 * input_buffer.num_samples() as usize];
    let mut output_buffer = AudioBufferMut::try_new(&mut output_container, 2).unwrap();

    let context = Context::default();

    let audio_settings = AudioSettings {
        frame_size,
        ..Default::default()
    };

    let hrtf_settings = HrtfSettings::default();

    let hrtf = Hrtf::try_new(&context, &audio_settings, &hrtf_settings).unwrap();

    let binaural_effect_settings = effect::BinauralEffectSettings { hrtf: hrtf.clone() };

    let mut binaural_effect =
        effect::BinauralEffect::try_new(&context, &audio_settings, &binaural_effect_settings)
            .unwrap();

    let binaural_effect_params = effect::BinauralEffectParams {
        direction: geometry::Direction::new(1.0, 1.0, 1.0),
        interpolation: HrtfInterpolation::Nearest,
        spatial_blend: 1.0,
        hrtf,
        peak_delays: None,
    };

    let _ = binaural_effect.apply(&binaural_effect_params, &input_buffer, &mut output_buffer);

    let mut interleaved =
        vec![0.0; (output_buffer.num_channels() * output_buffer.num_samples()) as usize];
    assert!(output_buffer.interleave(&context, &mut interleaved).is_ok());
}

#[test]
fn test_ambisonics_encode_effect() {
    let frequency = 440.0;
    let amplitude = 0.5;
    let duration_secs = 0.1;
    let sample_rate = 48000;
    let sine_wave = sine_wave(frequency, amplitude, duration_secs, sample_rate);
    let input_buffer = AudioBufferRef::try_from(&sine_wave[..]).unwrap();
    let frame_size = sine_wave.len() as u32;

    let mut output_container = vec![0.0; input_buffer.num_samples()];
    let mut output_buffer = AudioBufferMut::try_from(&mut output_container[..]).unwrap();

    let context = Context::default();

    let audio_settings = AudioSettings {
        frame_size,
        ..Default::default()
    };

    let ambisonics_encode_effect_settings = effect::AmbisonicsEncodeEffectSettings { max_order: 0 };

    let mut ambisonics_encode_effect = effect::AmbisonicsEncodeEffect::try_new(
        &context,
        &audio_settings,
        &ambisonics_encode_effect_settings,
    )
    .unwrap();

    let ambisonics_encode_effect_params = effect::AmbisonicsEncodeEffectParams {
        direction: geometry::Direction::new(1.0, 1.0, 1.0),
        order: 0,
    };

    let _ = ambisonics_encode_effect.apply(
        &ambisonics_encode_effect_params,
        &input_buffer,
        &mut output_buffer,
    );

    let mut interleaved =
        vec![0.0; (output_buffer.num_channels() * output_buffer.num_samples()) as usize];
    assert!(output_buffer.interleave(&context, &mut interleaved).is_ok());
}

#[test]
fn test_ambisonics_decode_effect() {
    let frequency = 440.0;
    let amplitude = 0.5;
    let duration_secs = 0.1;
    let sample_rate = 48000;
    let sine_wave = sine_wave(frequency, amplitude, duration_secs, sample_rate);
    let input_buffer = AudioBufferRef::try_from(&sine_wave[..]).unwrap();
    let frame_size = sine_wave.len() as u32;

    let mut output_container = vec![0.0; 2 * input_buffer.num_samples()];
    let mut output_buffer = AudioBufferMut::try_new(&mut output_container, 2).unwrap();

    let context = Context::default();

    let audio_settings = AudioSettings {
        frame_size,
        ..Default::default()
    };

    let hrtf_settings = HrtfSettings::default();

    let hrtf = Hrtf::try_new(&context, &audio_settings, &hrtf_settings).unwrap();

    let ambisonics_decode_effect_settings = effect::AmbisonicsDecodeEffectSettings {
        speaker_layout: SpeakerLayout::Mono,
        hrtf: hrtf.clone(),
        max_order: 0,
        rendering: Rendering::Binaural,
    };

    let mut ambisonics_decode_effect = effect::AmbisonicsDecodeEffect::try_new(
        &context,
        &audio_settings,
        &ambisonics_decode_effect_settings,
    )
    .unwrap();

    let ambisonics_decode_effect_params = effect::AmbisonicsDecodeEffectParams {
        order: 0,
        hrtf,
        orientation: geometry::CoordinateSystem::default(),
    };

    let _ = ambisonics_decode_effect.apply(
        &ambisonics_decode_effect_params,
        &input_buffer,
        &mut output_buffer,
    );

    let mut interleaved =
        vec![0.0; (output_buffer.num_channels() * output_buffer.num_samples()) as usize];
    assert!(output_buffer.interleave(&context, &mut interleaved).is_ok());
}

#[test]
fn test_direct_effect() {
    let frequency = 440.0;
    let amplitude = 0.5;
    let duration_secs = 0.1;
    let sample_rate = 48000;
    let sine_wave = sine_wave(frequency, amplitude, duration_secs, sample_rate);
    let input_buffer = AudioBufferRef::try_from(&sine_wave[..]).unwrap();
    let frame_size = sine_wave.len() as u32;

    let mut output_container = vec![0.0; input_buffer.num_samples()];
    let mut output_buffer = AudioBufferMut::try_from(&mut output_container[..]).unwrap();

    let context = Context::default();

    let audio_settings = AudioSettings {
        frame_size,
        ..Default::default()
    };

    let direct_effect_settings = effect::DirectEffectSettings { num_channels: 1 };

    let mut direct_effect =
        effect::DirectEffect::try_new(&context, &audio_settings, &direct_effect_settings).unwrap();

    let direct_effect_params = effect::DirectEffectParams {
        distance_attenuation: Some(0.6),
        air_absorption: Some(Equalizer([0.9, 0.7, 0.5])),
        directivity: Some(0.7),
        occlusion: Some(0.4),
        transmission: Some(Transmission::FrequencyIndependent(Equalizer([
            0.3, 0.2, 0.1,
        ]))),
    };

    let _ = direct_effect.apply(&direct_effect_params, &input_buffer, &mut output_buffer);

    let mut interleaved =
        vec![0.0; (output_buffer.num_channels() * output_buffer.num_samples()) as usize];
    assert!(output_buffer.interleave(&context, &mut interleaved).is_ok());
}

#[cfg(static_api)]
mod pathing {
    #[cfg(not(feature = "static"))]
    use audionimbus as static_api;
    #[cfg(feature = "static")]
    use audionimbus::r#static as static_api;
    use static_api::*;

    #[test]
    fn test_pathing() {
        const MAX_ORDER: u32 = 1;

        let context = Context::default();
        let audio_settings = AudioSettings::default();

        let simulation_settings = SimulationSettings::new(&audio_settings)
            .with_direct(DirectSimulationSettings {
                max_num_occlusion_samples: 4,
            })
            .with_reflections(ConvolutionSettings {
                max_num_rays: 4096,
                num_diffuse_samples: 32,
                max_duration: 2.0,
                max_num_sources: 8,
                num_threads: 2,
                max_order: MAX_ORDER,
            })
            .with_pathing(PathingSimulationSettings {
                num_visibility_samples: 4,
            });
        let mut simulator = Simulator::try_new(&context, &simulation_settings).unwrap();

        let mut scene = Scene::try_new(&context).unwrap();
        let vertices = vec![
            Point::new(-50.0, 0.0, -50.0),
            Point::new(50.0, 0.0, -50.0),
            Point::new(50.0, 0.0, 50.0),
            Point::new(-50.0, 0.0, 50.0),
        ];
        let triangles = vec![Triangle::new(0, 1, 2), Triangle::new(0, 2, 3)];
        let materials = vec![Material::default()];
        let material_indices = vec![0, 0];
        let static_mesh = StaticMesh::try_new(
            &scene,
            &StaticMeshSettings {
                vertices: &vertices,
                triangles: &triangles,
                material_indices: &material_indices,
                materials: &materials,
            },
        )
        .unwrap();
        scene.add_static_mesh(static_mesh);
        scene.commit();
        simulator.set_scene(&scene);

        let identifier = BakedDataIdentifier::Pathing {
            variation: BakedDataVariation::Dynamic,
        };

        let mut probe_array = ProbeArray::try_new(&context).unwrap();
        let box_transform = Matrix::new([
            [100.0, 0.0, 0.0, 0.0],
            [0.0, 100.0, 0.0, 0.0],
            [0.0, 0.0, 100.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ]);
        probe_array.generate_probes(
            &scene,
            &ProbeGenerationParams::UniformFloor {
                spacing: 2.0,
                height: 1.5,
                transform: box_transform,
            },
        );

        let mut pathing_probes = ProbeBatch::try_new(&context).unwrap();
        pathing_probes.add_probe_array(&probe_array);
        pathing_probes.commit();
        simulator.add_probe_batch(&pathing_probes);

        let path_bake_params = PathBakeParams {
            identifier,
            num_samples: 1, // Trace a single ray to test if one probe can see another probe.
            visibility_range: 50.0, // Don't check visibility between probes that are > 50m apart.
            path_range: 100.0, // Don't store paths between probes that are > 100m apart.
            num_threads: 8,
            radius: 1.0,
            threshold: 0.5,
        };
        PathBaker::new()
            .bake(&context, &mut pathing_probes, &scene, path_bake_params)
            .unwrap();

        let source = Source::try_new(&simulator).unwrap();
        let simulation_inputs = SimulationInputs {
            source: CoordinateSystem::default(),
            parameters: SimulationParameters::new()
                .with_direct(
                    DirectSimulationParameters::new()
                        .with_distance_attenuation(DistanceAttenuationModel::default())
                        .with_air_absorption(AirAbsorptionModel::default())
                        .with_directivity(Directivity::default())
                        .with_occlusion(
                            Occlusion::new(OcclusionAlgorithm::Raycast).with_transmission(
                                TransmissionParameters {
                                    num_transmission_rays: 1,
                                },
                            ),
                        ),
                )
                .with_reflections(ConvolutionParameters {
                    baked_data_identifier: None,
                })
                .with_pathing(PathingSimulationParameters {
                    pathing_probes,
                    visibility_radius: 1.0,
                    visibility_threshold: 10.0,
                    visibility_range: 10.0,
                    pathing_order: 1,
                    enable_validation: true,
                    find_alternate_paths: true,
                    deviation: DeviationModel::default(),
                }),
        };
        source.set_pathing_inputs(&simulation_inputs).unwrap();
        simulator.add_source(&source);

        simulator.commit();
        assert!(simulator.run_pathing().is_ok());

        let audio_settings = AudioSettings::default();
        let path_effect_settings = PathEffectSettings {
            max_order: MAX_ORDER,
            spatialization: None,
        };
        let mut path_effect =
            PathEffect::try_new(&context, &audio_settings, &path_effect_settings).unwrap();

        let input = vec![0.5; audio_settings.frame_size as usize];
        let input_buffer = AudioBufferRef::try_from(input.as_slice()).unwrap();

        // Must have 4 channels (1st order Ambisonics) for this example.
        let mut output_container = vec![0.0; 4 * input_buffer.num_samples()];
        let mut output_buffer =
            AudioBufferMut::try_new(output_container.as_mut_slice(), 4 as usize).unwrap();

        let path_effect_params = source.get_pathing_outputs().unwrap();
        let _ = path_effect.apply(&path_effect_params, &input_buffer, &mut output_buffer);
    }
}

mod reflections {

    #[cfg(static_api)]
    mod r#static {
        #[cfg(not(feature = "static"))]
        use audionimbus as static_api;
        #[cfg(feature = "static")]
        use audionimbus::r#static as static_api;
        use static_api::*;

        /// Preserves processing through the existing static capability bound.
        fn apply_static_reflections<
            T: audionimbus::ReflectionEffectType + audionimbus::CanApplyDirectly,
        >(
            reflection_effect: &mut static_api::ReflectionEffect<T>,
            reflection_effect_params: &static_api::ReflectionEffectParams<T>,
            input_buffer: &impl audionimbus::AudioBuffer,
            output_buffer: &mut audionimbus::AudioBufferMut<'_>,
        ) -> Result<audionimbus::AudioEffectState, audionimbus::EffectError> {
            reflection_effect.apply(reflection_effect_params, input_buffer, output_buffer)
        }

        #[test]
        fn preserves_static_reflection_api() {
            use audionimbus::{
                AudioBufferMut, AudioBufferRef, AudioSettings, Convolution, Hybrid, Parametric,
                ReflectionEffectSettings,
            };
            use static_api::{ReflectionEffect, ReflectionEffectParams, ReflectionMixer};

            let context = Context::default();
            let audio_settings = AudioSettings::default();
            let reflection_effect_settings = ReflectionEffectSettings {
                impulse_response_size: audio_settings.sampling_rate,
                num_channels: 4,
            };
            let mut reflection_effect = ReflectionEffect::<Parametric>::try_new(
                &context,
                &audio_settings,
                &reflection_effect_settings,
            )
            .unwrap();
            let mut reflection_effect_params = ReflectionEffectParams::<Parametric>::new(
                [1.0; 3],
                4,
                audio_settings.sampling_rate,
            );
            reflection_effect_params.set_num_channels(1).unwrap();
            reflection_effect_params
                .set_impulse_response_size(audio_settings.sampling_rate / 2)
                .unwrap();
            let input_samples = vec![0.0; audio_settings.frame_size as usize];
            let input_buffer = AudioBufferRef::try_from(input_samples.as_slice()).unwrap();
            let mut output_samples = vec![0.0; audio_settings.frame_size as usize];
            let mut output_buffer = AudioBufferMut::try_new(&mut output_samples, 1).unwrap();
            assert!(
                apply_static_reflections(
                    &mut reflection_effect,
                    &reflection_effect_params,
                    &input_buffer,
                    &mut output_buffer
                )
                .is_ok()
            );
            let _: &mut audionimbus_sys::IPLReflectionEffect = reflection_effect.raw_ptr_mut();
            let mut reflection_mixer = ReflectionMixer::<Convolution>::try_new(
                &context,
                &audio_settings,
                &reflection_effect_settings,
            )
            .unwrap();
            let _: &mut audionimbus_sys::IPLReflectionMixer = reflection_mixer.raw_ptr_mut();
            let _: fn(
                &mut ReflectionMixer<Convolution>,
                &ReflectionEffectParams<Convolution>,
                &mut AudioBufferMut<'_>,
            )
                -> Result<audionimbus::AudioEffectState, audionimbus::EffectError> =
                ReflectionMixer::<Convolution>::apply;
            let _: ReflectionMixer<Hybrid> =
                ReflectionMixer::try_new(&context, &audio_settings, &reflection_effect_settings)
                    .unwrap();
            let _: ReflectionMixer<Parametric> =
                ReflectionMixer::try_new(&context, &audio_settings, &reflection_effect_settings)
                    .unwrap();
        }
    }

    #[cfg(feature = "dynamic")]
    mod dynamic {
        use audionimbus::dynamic::{
            ReflectionAlgorithm, ReflectionEffect, ReflectionError, ReflectionMixer,
        };
        use audionimbus::{
            AudioBufferMut, AudioSettings, ChannelRequirement, Context, EffectError,
            ReflectionEffectSettings, SteamAudioError,
        };

        #[test]
        fn processes_cpu_tails() {
            let context = Context::default();
            let audio_settings = AudioSettings::default();
            let reflection_effect_settings = ReflectionEffectSettings {
                impulse_response_size: audio_settings.sampling_rate,
                num_channels: 4,
            };

            for algorithm in [
                ReflectionAlgorithm::Convolution,
                ReflectionAlgorithm::Parametric,
                ReflectionAlgorithm::Hybrid,
            ] {
                let reflection_effect: Result<_, SteamAudioError> = ReflectionEffect::try_new(
                    &context,
                    &audio_settings,
                    &reflection_effect_settings,
                    algorithm,
                );
                let mut reflection_effect = reflection_effect.unwrap();
                assert_eq!(reflection_effect.algorithm(), algorithm);
                let cloned_effect = reflection_effect.clone();
                drop(reflection_effect);
                reflection_effect = cloned_effect;
                reflection_effect.reset();
                assert_eq!(reflection_effect.tail_size(), 0);
                let mut samples = vec![0.0; 4 * audio_settings.frame_size as usize];
                let mut output_buffer = AudioBufferMut::try_new(&mut samples, 4).unwrap();
                assert!(reflection_effect.tail(&mut output_buffer).is_ok());
            }
        }

        #[test]
        fn rejects_unsupported_mixers() {
            let context = Context::default();
            let audio_settings = AudioSettings::default();
            let reflection_effect_settings = ReflectionEffectSettings {
                impulse_response_size: audio_settings.sampling_rate,
                num_channels: 4,
            };

            for algorithm in [ReflectionAlgorithm::Parametric, ReflectionAlgorithm::Hybrid] {
                assert_eq!(
                    ReflectionMixer::try_new(
                        &context,
                        &audio_settings,
                        &reflection_effect_settings,
                        algorithm
                    )
                    .unwrap_err(),
                    ReflectionError::MixerUnsupported
                );
                let reflection_effect = ReflectionEffect::try_new(
                    &context,
                    &audio_settings,
                    &reflection_effect_settings,
                    algorithm,
                )
                .unwrap();
                let reflection_mixer = ReflectionMixer::try_new(
                    &context,
                    &audio_settings,
                    &reflection_effect_settings,
                    ReflectionAlgorithm::Convolution,
                )
                .unwrap();
                let mut samples = vec![0.0; 4 * audio_settings.frame_size as usize];
                let mut output_buffer = AudioBufferMut::try_new(&mut samples, 4).unwrap();
                assert_eq!(
                    reflection_effect.tail_into_mixer(&mut output_buffer, &reflection_mixer),
                    Err(ReflectionError::MixerUnsupported)
                );
            }
        }

        #[test]
        fn rejects_incompatible_routes() {
            let context = Context::default();
            let audio_settings = AudioSettings::default();
            let reflection_effect_settings = ReflectionEffectSettings {
                impulse_response_size: audio_settings.sampling_rate,
                num_channels: 4,
            };
            let convolution_effect = ReflectionEffect::try_new(
                &context,
                &audio_settings,
                &reflection_effect_settings,
                ReflectionAlgorithm::Convolution,
            )
            .unwrap();
            let tan_effect = ReflectionEffect::try_new(
                &context,
                &audio_settings,
                &reflection_effect_settings,
                ReflectionAlgorithm::TrueAudioNext,
            )
            .unwrap();
            let convolution_mixer = ReflectionMixer::try_new(
                &context,
                &audio_settings,
                &reflection_effect_settings,
                ReflectionAlgorithm::Convolution,
            )
            .unwrap();
            let tan_mixer = ReflectionMixer::try_new(
                &context,
                &audio_settings,
                &reflection_effect_settings,
                ReflectionAlgorithm::TrueAudioNext,
            )
            .unwrap();
            let mut samples = vec![0.0; 4 * audio_settings.frame_size as usize];
            let mut output_buffer = AudioBufferMut::try_new(&mut samples, 4).unwrap();

            assert_eq!(
                tan_effect.tail(&mut output_buffer),
                Err(ReflectionError::DirectApplyUnsupported)
            );
            assert_eq!(
                convolution_effect.tail_into_mixer(&mut output_buffer, &tan_mixer),
                Err(ReflectionError::AlgorithmMismatch {
                    expected: ReflectionAlgorithm::Convolution,
                    actual: ReflectionAlgorithm::TrueAudioNext
                })
            );
            assert_eq!(
                tan_effect.tail_into_mixer(&mut output_buffer, &convolution_mixer),
                Err(ReflectionError::AlgorithmMismatch {
                    expected: ReflectionAlgorithm::TrueAudioNext,
                    actual: ReflectionAlgorithm::Convolution
                })
            );
            assert!(
                convolution_effect
                    .tail_into_mixer(&mut output_buffer, &convolution_mixer)
                    .is_ok()
            );
            assert_eq!(
                tan_mixer.clone().algorithm(),
                ReflectionAlgorithm::TrueAudioNext
            );
        }

        #[test]
        fn preserves_channel_requirements() {
            let context = Context::default();
            let audio_settings = AudioSettings::default();
            let reflection_effect_settings = ReflectionEffectSettings {
                impulse_response_size: audio_settings.sampling_rate,
                num_channels: 4,
            };
            let parametric_effect = ReflectionEffect::try_new(
                &context,
                &audio_settings,
                &reflection_effect_settings,
                ReflectionAlgorithm::Parametric,
            )
            .unwrap();
            let convolution_effect = ReflectionEffect::try_new(
                &context,
                &audio_settings,
                &reflection_effect_settings,
                ReflectionAlgorithm::Convolution,
            )
            .unwrap();
            let mut samples = vec![0.0; audio_settings.frame_size as usize];
            let mut output_buffer = AudioBufferMut::try_new(&mut samples, 1).unwrap();

            assert!(parametric_effect.tail(&mut output_buffer).is_ok());
            assert_eq!(
                convolution_effect.tail(&mut output_buffer),
                Err(ReflectionError::Effect(
                    EffectError::InvalidOutputChannels {
                        expected: ChannelRequirement::Exactly(4),
                        actual: 1
                    }
                ))
            );
        }
    }
}

#[cfg(static_api)]
mod reflection_effect {
    use super::convolution_parameters;
    #[cfg(not(feature = "static"))]
    use audionimbus as static_api;
    #[cfg(feature = "static")]
    use audionimbus::r#static as static_api;
    use static_api::*;

    mod apply {
        use super::*;

        #[test]
        fn test_valid() {
            let context = Context::default();

            let audio_settings = AudioSettings::default();
            let impulse_response_size = 2 * audio_settings.sampling_rate;

            let reflection_effect_params = convolution_parameters(&context, &audio_settings);

            let num_output_channels = num_ambisonics_channels(1);
            let reflection_effect_settings = ReflectionEffectSettings {
                impulse_response_size,
                num_channels: num_output_channels,
            };
            let mut reflection_effect = ReflectionEffect::<Convolution>::try_new(
                &context,
                &audio_settings,
                &reflection_effect_settings,
            )
            .unwrap();

            let input_container = vec![0.5; audio_settings.frame_size as usize];
            let input_buffer = AudioBufferRef::try_from(input_container.as_slice()).unwrap();

            let mut output_container =
                vec![0.0; num_output_channels as usize * input_buffer.num_samples()];
            let mut output_buffer =
                AudioBufferMut::try_new(output_container.as_mut_slice(), 4 as usize).unwrap();

            assert!(
                reflection_effect
                    .apply(&reflection_effect_params, &input_buffer, &mut output_buffer)
                    .is_ok()
            );
        }

        #[test]
        fn test_invalid_input_num_channels() {
            let context = Context::default();

            let audio_settings = AudioSettings::default();
            let impulse_response_size = 2 * audio_settings.sampling_rate;

            let reflection_effect_params = convolution_parameters(&context, &audio_settings);

            let num_output_channels = num_ambisonics_channels(1);
            let reflection_effect_settings = ReflectionEffectSettings {
                impulse_response_size,
                num_channels: num_output_channels,
            };
            let mut reflection_effect = ReflectionEffect::<Convolution>::try_new(
                &context,
                &audio_settings,
                &reflection_effect_settings,
            )
            .unwrap();

            let input_container = vec![0.5; 2 * audio_settings.frame_size as usize];
            let input_buffer =
                AudioBufferRef::try_new(input_container.as_slice(), 2 as usize).unwrap();

            let mut output_container =
                vec![0.0; num_output_channels as usize * input_buffer.num_samples()];
            let mut output_buffer =
                AudioBufferMut::try_new(output_container.as_mut_slice(), 4 as usize).unwrap();

            assert_eq!(
                reflection_effect.apply(
                    &reflection_effect_params,
                    &input_buffer,
                    &mut output_buffer
                ),
                Err(EffectError::InvalidInputChannels {
                    expected: ChannelRequirement::Exactly(1),
                    actual: 2
                })
            );
        }

        #[test]
        fn test_invalid_output_num_channels() {
            let context = Context::default();

            let audio_settings = AudioSettings::default();
            let impulse_response_size = 2 * audio_settings.sampling_rate;

            let reflection_effect_params = convolution_parameters(&context, &audio_settings);

            let num_output_channels = num_ambisonics_channels(1);
            let reflection_effect_settings = ReflectionEffectSettings {
                impulse_response_size,
                num_channels: num_output_channels,
            };
            let mut reflection_effect = ReflectionEffect::<Convolution>::try_new(
                &context,
                &audio_settings,
                &reflection_effect_settings,
            )
            .unwrap();

            let input_container = vec![0.5; audio_settings.frame_size as usize];
            let input_buffer = AudioBufferRef::try_from(input_container.as_slice()).unwrap();

            let mut output_container = vec![0.0; (2 * input_buffer.num_samples()) as usize];
            let mut output_buffer =
                AudioBufferMut::try_new(output_container.as_mut_slice(), 2 as usize).unwrap();

            assert_eq!(
                reflection_effect.apply(
                    &reflection_effect_params,
                    &input_buffer,
                    &mut output_buffer
                ),
                Err(EffectError::InvalidOutputChannels {
                    expected: ChannelRequirement::Exactly(4),
                    actual: 2
                })
            );
        }
    }

    mod apply_into_mixer {
        use super::*;

        #[test]
        fn test_valid() {
            let context = Context::default();

            let audio_settings = AudioSettings::default();
            let impulse_response_size = 2 * audio_settings.sampling_rate;

            let reflection_effect_params = convolution_parameters(&context, &audio_settings);

            let num_output_channels = num_ambisonics_channels(1);
            let reflection_effect_settings = ReflectionEffectSettings {
                impulse_response_size,
                num_channels: num_output_channels,
            };
            let mut reflection_effect = ReflectionEffect::<Convolution>::try_new(
                &context,
                &audio_settings,
                &reflection_effect_settings,
            )
            .unwrap();

            let input_container = vec![0.5; audio_settings.frame_size as usize];
            let input_buffer = AudioBufferRef::try_from(input_container.as_slice()).unwrap();

            let mut output_container =
                vec![0.0; num_output_channels as usize * input_buffer.num_samples()];
            let mut output_buffer =
                AudioBufferMut::try_new(output_container.as_mut_slice(), 4 as usize).unwrap();

            let mixer =
                ReflectionMixer::try_new(&context, &audio_settings, &reflection_effect_settings)
                    .unwrap();

            assert!(
                reflection_effect
                    .apply_into_mixer(
                        &reflection_effect_params,
                        &input_buffer,
                        &mut output_buffer,
                        &mixer
                    )
                    .is_ok()
            );
        }

        #[test]
        fn test_invalid_input_num_channels() {
            let context = Context::default();

            let audio_settings = AudioSettings::default();
            let impulse_response_size = 2 * audio_settings.sampling_rate;

            let reflection_effect_params = convolution_parameters(&context, &audio_settings);

            let num_output_channels = num_ambisonics_channels(1);
            let reflection_effect_settings = ReflectionEffectSettings {
                impulse_response_size,
                num_channels: num_output_channels,
            };
            let mut reflection_effect = ReflectionEffect::<Convolution>::try_new(
                &context,
                &audio_settings,
                &reflection_effect_settings,
            )
            .unwrap();

            let input_container = vec![0.5; 2 * audio_settings.frame_size as usize];
            let input_buffer =
                AudioBufferRef::try_new(input_container.as_slice(), 2 as usize).unwrap();

            let mut output_container =
                vec![0.0; num_output_channels as usize * input_buffer.num_samples()];
            let mut output_buffer =
                AudioBufferMut::try_new(output_container.as_mut_slice(), 4 as usize).unwrap();

            let mixer =
                ReflectionMixer::try_new(&context, &audio_settings, &reflection_effect_settings)
                    .unwrap();

            assert_eq!(
                reflection_effect.apply_into_mixer(
                    &reflection_effect_params,
                    &input_buffer,
                    &mut output_buffer,
                    &mixer
                ),
                Err(EffectError::InvalidInputChannels {
                    expected: ChannelRequirement::Exactly(1),
                    actual: 2
                })
            );
        }

        #[test]
        fn test_invalid_output_num_channels() {
            let context = Context::default();

            let audio_settings = AudioSettings::default();
            let impulse_response_size = 2 * audio_settings.sampling_rate;

            let reflection_effect_params = convolution_parameters(&context, &audio_settings);

            let num_output_channels = num_ambisonics_channels(1);
            let reflection_effect_settings = ReflectionEffectSettings {
                impulse_response_size,
                num_channels: num_output_channels,
            };
            let mut reflection_effect = ReflectionEffect::<Convolution>::try_new(
                &context,
                &audio_settings,
                &reflection_effect_settings,
            )
            .unwrap();

            let input_container = vec![0.5; audio_settings.frame_size as usize];
            let input_buffer = AudioBufferRef::try_from(input_container.as_slice()).unwrap();

            let mut output_container = vec![0.0; (2 * input_buffer.num_samples()) as usize];
            let mut output_buffer =
                AudioBufferMut::try_new(output_container.as_mut_slice(), 2 as usize).unwrap();

            let mixer =
                ReflectionMixer::try_new(&context, &audio_settings, &reflection_effect_settings)
                    .unwrap();

            assert_eq!(
                reflection_effect.apply_into_mixer(
                    &reflection_effect_params,
                    &input_buffer,
                    &mut output_buffer,
                    &mixer
                ),
                Err(EffectError::InvalidOutputChannels {
                    expected: ChannelRequirement::Exactly(4),
                    actual: 2
                })
            );
        }
    }
}

#[cfg(static_api)]
mod reflection_mixer {
    use super::convolution_parameters;
    #[cfg(not(feature = "static"))]
    use audionimbus as static_api;
    #[cfg(feature = "static")]
    use audionimbus::r#static as static_api;
    use static_api::*;

    mod apply {
        use super::*;

        #[test]
        fn test_valid() {
            let context = Context::default();

            let audio_settings = AudioSettings::default();
            let impulse_response_size = 2 * audio_settings.sampling_rate;

            let reflection_effect_params = convolution_parameters(&context, &audio_settings);

            let num_output_channels = num_ambisonics_channels(1);
            let reflection_effect_settings = ReflectionEffectSettings {
                impulse_response_size,
                num_channels: num_output_channels,
            };

            let mut mixer = ReflectionMixer::<Convolution>::try_new(
                &context,
                &audio_settings,
                &reflection_effect_settings,
            )
            .unwrap();

            let mut output_container =
                vec![0.0; (num_output_channels * audio_settings.frame_size) as usize];
            let mut output_buffer = AudioBufferMut::try_new(
                output_container.as_mut_slice(),
                num_output_channels as usize,
            )
            .unwrap();

            assert!(
                mixer
                    .apply(&reflection_effect_params, &mut output_buffer)
                    .is_ok()
            );
        }

        #[test]
        fn test_invalid_output_num_channels() {
            let context = Context::default();

            let audio_settings = AudioSettings::default();
            let impulse_response_size = 2 * audio_settings.sampling_rate;

            let reflection_effect_params = convolution_parameters(&context, &audio_settings);

            let num_output_channels = num_ambisonics_channels(1);
            let reflection_effect_settings = ReflectionEffectSettings {
                impulse_response_size,
                num_channels: num_output_channels,
            };

            let mut mixer = ReflectionMixer::<Convolution>::try_new(
                &context,
                &audio_settings,
                &reflection_effect_settings,
            )
            .unwrap();

            let mut output_container = vec![0.0; (2 * audio_settings.frame_size) as usize];
            let mut output_buffer =
                AudioBufferMut::try_new(output_container.as_mut_slice(), 2 as usize).unwrap();

            assert_eq!(
                mixer.apply(&reflection_effect_params, &mut output_buffer),
                Err(EffectError::InvalidOutputChannels {
                    expected: ChannelRequirement::Exactly(4),
                    actual: 2
                })
            );
        }

        #[test]
        fn test_parametric_valid() {
            let context = Context::default();

            let audio_settings = AudioSettings::default();
            let impulse_response_size = 2 * audio_settings.sampling_rate;

            let simulation_settings =
                SimulationSettings::new(&audio_settings).with_reflections(ParametricSettings {
                    max_num_rays: 4096,
                    num_diffuse_samples: 32,
                    max_duration: 2.0,
                    max_num_sources: 8,
                    num_threads: 1,
                    max_order: 1,
                });
            let mut simulator = Simulator::try_new(&context, &simulation_settings).unwrap();

            let scene = Scene::try_new(&context).unwrap();
            simulator.set_scene(&scene);

            let source = Source::try_new(&simulator).unwrap();
            simulator.add_source(&source);

            let simulation_shared_inputs = SimulationSharedInputs::new(CoordinateSystem::default())
                .with_reflections(ReflectionsSharedInputs {
                    num_rays: 4096,
                    num_bounces: 16,
                    duration: 2.0,
                    order: 1,
                    irradiance_min_distance: 1.0,
                });
            simulator
                .set_shared_reflections_inputs(&simulation_shared_inputs)
                .unwrap();

            simulator.commit();

            assert!(simulator.run_reflections().is_ok());
            let reflection_effect_params = source.get_reflections_outputs().unwrap();

            let num_output_channels = num_ambisonics_channels(1);
            let reflection_effect_settings = ReflectionEffectSettings {
                impulse_response_size,
                num_channels: num_output_channels,
            };

            let mut mixer = ReflectionMixer::<Parametric>::try_new(
                &context,
                &audio_settings,
                &reflection_effect_settings,
            )
            .unwrap();

            let mut output_container =
                vec![0.0; (num_output_channels * audio_settings.frame_size) as usize];
            let mut output_buffer = AudioBufferMut::try_new(
                output_container.as_mut_slice(),
                num_output_channels as usize,
            )
            .unwrap();

            assert!(
                mixer
                    .apply(&reflection_effect_params, &mut output_buffer)
                    .is_ok()
            );
        }

        #[test]
        fn test_parametric_at_least_one_channel() {
            let context = Context::default();

            let audio_settings = AudioSettings::default();
            let impulse_response_size = 2 * audio_settings.sampling_rate;

            let simulation_settings =
                SimulationSettings::new(&audio_settings).with_reflections(ParametricSettings {
                    max_num_rays: 4096,
                    num_diffuse_samples: 32,
                    max_duration: 2.0,
                    max_num_sources: 8,
                    num_threads: 1,
                    max_order: 1,
                });
            let mut simulator = Simulator::try_new(&context, &simulation_settings).unwrap();

            let scene = Scene::try_new(&context).unwrap();
            simulator.set_scene(&scene);

            let source = Source::try_new(&simulator).unwrap();
            simulator.add_source(&source);

            let simulation_shared_inputs = SimulationSharedInputs::new(CoordinateSystem::default())
                .with_reflections(ReflectionsSharedInputs {
                    num_rays: 4096,
                    num_bounces: 16,
                    duration: 2.0,
                    order: 1,
                    irradiance_min_distance: 1.0,
                });
            simulator
                .set_shared_reflections_inputs(&simulation_shared_inputs)
                .unwrap();

            simulator.commit();

            assert!(simulator.run_reflections().is_ok());
            let reflection_effect_params = source.get_reflections_outputs().unwrap();

            let num_output_channels = num_ambisonics_channels(1);
            let reflection_effect_settings = ReflectionEffectSettings {
                impulse_response_size,
                num_channels: num_output_channels,
            };

            let mut mixer = ReflectionMixer::<Parametric>::try_new(
                &context,
                &audio_settings,
                &reflection_effect_settings,
            )
            .unwrap();

            // Test with 1 channel (minimum for parametric)
            let mut output_container = vec![0.0; audio_settings.frame_size as usize];
            let mut output_buffer =
                AudioBufferMut::try_from(output_container.as_mut_slice()).unwrap();

            assert!(
                mixer
                    .apply(&reflection_effect_params, &mut output_buffer)
                    .is_ok()
            );
        }
    }
}
