#[cfg(feature = "dynamic")]
use audionimbus::dynamic as runtime;
#[cfg(feature = "dynamic")]
use audionimbus::{
    BakedDataIdentifier, BakedDataVariation, Context, Material, Matrix, PathBakeParams, Point,
    ProbeArray, ProbeBatch, ProbeGenerationParams, SimulationFlags, StaticMeshSettings, Triangle,
};

// Produce samples following a sine wave.
pub fn sine_wave(frequency: f32, amplitude: f32, duration_secs: f32, sample_rate: u32) -> Vec<f32> {
    let num_samples = (duration_secs * sample_rate as f32) as usize;
    let phase_increment = 2.0 * std::f32::consts::PI * frequency / sample_rate as f32;

    (0..num_samples)
        .map(|i| {
            let phase = i as f32 * phase_increment;
            amplitude * phase.sin()
        })
        .collect()
}

#[cfg(feature = "dynamic")]
/// Creates a source with baked pathing probes.
///
/// Returns the probe scene, simulation scene, probe array, batch, and source.
///
/// # Panics
///
/// Panics if native setup or baking fails.
pub fn baked_source(
    context: &Context,
    simulator: &mut runtime::Simulator,
) -> (
    runtime::Scene,
    runtime::Scene,
    ProbeArray,
    ProbeBatch,
    runtime::Source,
) {
    let mut probe_scene =
        runtime::Scene::try_new(context, runtime::RayTracerSettings::Default).unwrap();
    let mut scene = runtime::Scene::try_new(context, runtime::RayTracerSettings::Default).unwrap();
    let vertices = [
        Point::new(-4.0, 0.0, -4.0),
        Point::new(4.0, 0.0, -4.0),
        Point::new(4.0, 0.0, 4.0),
        Point::new(-4.0, 0.0, 4.0),
    ];
    let triangles = [Triangle::new(0, 1, 2), Triangle::new(0, 2, 3)];
    let materials = [Material::CONCRETE];
    let material_indices = [0, 0];
    let mesh_settings = StaticMeshSettings {
        vertices: &vertices,
        triangles: &triangles,
        materials: &materials,
        material_indices: &material_indices,
    };
    let mesh = runtime::StaticMesh::try_new(&probe_scene, &mesh_settings).unwrap();
    probe_scene.try_add_static_mesh(mesh).unwrap();
    probe_scene.commit();
    let mesh = runtime::StaticMesh::try_new(&scene, &mesh_settings).unwrap();
    scene.try_add_static_mesh(mesh).unwrap();
    scene.commit();
    simulator.try_set_scene(&scene).unwrap();
    let mut probe_array = ProbeArray::try_new(context).unwrap();
    probe_array.generate_probes(
        &probe_scene,
        &ProbeGenerationParams::UniformFloor {
            spacing: 2.0,
            height: 1.0,
            transform: Matrix::new([
                [4.0, 0.0, 0.0, 0.0],
                [0.0, 4.0, 0.0, 0.0],
                [0.0, 0.0, 4.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ]),
        },
    );
    assert!(probe_array.num_probes() > 0);
    let mut probe_batch = ProbeBatch::try_new(context).unwrap();
    probe_batch.add_probe_array(&probe_array);
    probe_batch.commit();
    runtime::PathBaker::new()
        .bake(
            context,
            &mut probe_batch,
            &probe_scene,
            PathBakeParams {
                identifier: BakedDataIdentifier::Pathing {
                    variation: BakedDataVariation::Dynamic,
                },
                num_samples: 1,
                visibility_range: 10.0,
                path_range: 10.0,
                num_threads: 1,
                radius: 0.1,
                threshold: 0.5,
            },
        )
        .unwrap();
    simulator.add_probe_batch(&probe_batch);
    let source = runtime::Source::try_new(
        simulator,
        SimulationFlags::DIRECT | SimulationFlags::PATHING,
    )
    .unwrap();
    simulator.try_add_source(&source).unwrap();
    simulator.commit();

    (probe_scene, scene, probe_array, probe_batch, source)
}
