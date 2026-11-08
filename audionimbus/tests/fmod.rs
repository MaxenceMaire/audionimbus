#![cfg(feature = "fmod")]

#[cfg(static_api)]
#[test]
fn preserves_static_generic_calls() {
    use audionimbus::{DefaultRayTracer, RayTracer, SimulationSettings, Source, fmod};

    fn register<T: RayTracer, D, R, P, RE>(
        settings: &SimulationSettings<T, D, R, P, RE>,
        source: &Source<D, R, P, RE>,
    ) {
        fmod::set_simulation_settings::<T, D, R, P, RE>(settings);
        let handle = fmod::add_source::<D, R, P, RE>(source);
        fmod::set_reverb_source::<D, R, P, RE>(source);
        fmod::remove_source(handle);

        #[cfg(feature = "static")]
        audionimbus::r#static::fmod::set_simulation_settings::<T, D, R, P, RE>(settings);
    }

    let _ = register::<DefaultRayTracer, String, String, String, String>;
}

#[cfg(feature = "dynamic")]
#[test]
fn rejects_non_reflection_reverb() {
    use audionimbus::dynamic as runtime;
    #[cfg(not(feature = "static"))]
    use audionimbus::fmod;
    use audionimbus::{
        AudioSettings, Context, DirectSimulationSettings, ParametricSettings, SimulationFlags,
    };
    #[cfg(feature = "static")]
    use runtime::fmod;

    let context = Context::default();
    let simulation_settings = runtime::SimulationSettings::new(&AudioSettings::default())
        .with_direct(DirectSimulationSettings {
            max_num_occlusion_samples: 4,
        })
        .with_reflections(ParametricSettings {
            max_num_rays: 16,
            num_diffuse_samples: 8,
            max_duration: 0.1,
            max_num_sources: 2,
            num_threads: 1,
            max_order: 0,
        });
    let simulator = runtime::Simulator::try_new(&context, &simulation_settings).unwrap();
    let source = runtime::Source::try_new(&simulator, SimulationFlags::DIRECT).unwrap();

    assert!(matches!(
        fmod::set_reverb_source(&source),
        Err(runtime::SimulationError::DisabledModes(flags))
            if flags.bits() == SimulationFlags::REFLECTIONS.bits()
    ));
}

#[cfg(all(
    feature = "linking-static",
    any(target_os = "macos", target_os = "linux", target_os = "windows")
))]
#[test]
fn exposes_static_dsp_descriptions() {
    let context = audionimbus::Context::default();
    audionimbus::fmod::initialize(&context);

    let descriptions = unsafe {
        [
            audionimbus_sys::fmod::FMOD_SteamAudio_Spatialize_GetDSPDescription(),
            audionimbus_sys::fmod::FMOD_SteamAudio_MixerReturn_GetDSPDescription(),
            audionimbus_sys::fmod::FMOD_SteamAudio_Reverb_GetDSPDescription(),
        ]
    };
    for description in descriptions {
        assert!(!description.is_null());
    }

    audionimbus::fmod::terminate();
}
