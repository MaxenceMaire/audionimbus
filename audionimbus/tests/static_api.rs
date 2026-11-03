use audionimbus::ray_tracing::{DefaultRayTracer, RayTracer};
use audionimbus::{
    Context, InstancedMesh, InstancedMeshSettings, Material, Matrix4, Point, Scene, StaticMesh,
    StaticMeshSettings, Triangle,
};

/// Accepts mesh declarations without ray tracer bounds.
fn accepts_unconstrained_static_mesh<T>(_: Option<StaticMesh<T>>) {}

/// Exercises insertion through the existing static generic bound.
fn add_static_mesh<T: RayTracer>(scene: &mut Scene<T>, mesh: StaticMesh<T>) {
    scene.add_static_mesh(mesh);
}

#[test]
fn preserves_static_geometry_api() {
    accepts_unconstrained_static_mesh::<String>(None);

    let context = Context::default();
    let mut scene = Scene::try_new(&context).unwrap();
    let explicit_scene = Scene::<DefaultRayTracer>::try_new(&context).unwrap();
    let _: Scene = explicit_scene;

    let mesh = StaticMesh::try_new(
        &scene,
        &StaticMeshSettings {
            vertices: &[
                Point::new(0.0, 0.0, 0.0),
                Point::new(1.0, 0.0, 0.0),
                Point::new(0.0, 1.0, 0.0),
            ],
            triangles: &[Triangle::new(0, 1, 2)],
            materials: &[Material::CONCRETE],
            material_indices: &[0],
        },
    )
    .unwrap();
    add_static_mesh(&mut scene, mesh);
    scene.commit();

    let sub_scene = Scene::try_new(&context).unwrap();
    let instance = InstancedMesh::try_new(
        &scene,
        &InstancedMeshSettings {
            sub_scene,
            transform: Matrix4::IDENTITY,
        },
    )
    .unwrap();
    scene.add_instanced_mesh(instance);
    scene.commit();
}

/// Preserves processing through the existing static capability bound.
fn apply_static_reflections<
    T: audionimbus::ReflectionEffectType + audionimbus::CanApplyDirectly,
>(
    reflection_effect: &mut audionimbus::ReflectionEffect<T>,
    reflection_effect_params: &audionimbus::ReflectionEffectParams<T>,
    input_buffer: &impl audionimbus::AudioBuffer,
    output_buffer: &mut audionimbus::AudioBufferMut<'_>,
) -> Result<audionimbus::AudioEffectState, audionimbus::EffectError> {
    reflection_effect.apply(reflection_effect_params, input_buffer, output_buffer)
}

#[test]
fn preserves_static_reflection_api() {
    use audionimbus::{
        AudioBufferMut, AudioBufferRef, AudioSettings, Convolution, Hybrid, Parametric,
        ReflectionEffect, ReflectionEffectParams, ReflectionEffectSettings, ReflectionMixer,
    };

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
    let mut reflection_effect_params =
        ReflectionEffectParams::<Parametric>::new([1.0; 3], 4, audio_settings.sampling_rate);
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
        &mut ReflectionEffectParams<Convolution>,
        &mut AudioBufferMut<'_>,
    ) -> Result<audionimbus::AudioEffectState, audionimbus::EffectError> =
        ReflectionMixer::<Convolution>::apply;
    let _: ReflectionMixer<Hybrid> =
        ReflectionMixer::try_new(&context, &audio_settings, &reflection_effect_settings).unwrap();
    let _: ReflectionMixer<Parametric> =
        ReflectionMixer::try_new(&context, &audio_settings, &reflection_effect_settings).unwrap();
}
