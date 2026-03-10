use audionimbus::*;

#[test]
fn test_static_mesh() {
    let context = Context::default();
    let mut scene = Scene::try_new(&context).unwrap();

    // Four vertices of a unit square in the x-y plane.
    let vertices = vec![
        geometry::Point::new(0.0, 0.0, 0.0),
        geometry::Point::new(1.0, 0.0, 0.0),
        geometry::Point::new(1.0, 1.0, 0.0),
        geometry::Point::new(0.0, 1.0, 0.0),
    ];

    let triangles = vec![
        geometry::Triangle::new(0, 1, 2),
        geometry::Triangle::new(0, 2, 2),
    ];

    let materials = vec![geometry::Material {
        absorption: [0.1, 0.1, 0.1],
        scattering: 0.5,
        transmission: [0.2, 0.2, 0.2],
    }];

    // Both triangles use the same material.
    let material_indices = vec![0, 0];

    let static_mesh_settings = geometry::StaticMeshSettings {
        vertices: &vertices,
        triangles: &triangles,
        material_indices: &material_indices,
        materials: &materials,
    };

    let static_mesh = StaticMesh::try_new(&scene, &static_mesh_settings).unwrap();

    scene.add_static_mesh(static_mesh);

    scene.commit();
}

#[test]
fn test_instanced_mesh() {
    let context = Context::default();
    let mut main_scene = Scene::try_new(&context).unwrap();
    let mut sub_scene = Scene::try_new(&context).unwrap();

    // Four vertices of a unit square in the x-y plane.
    let vertices = vec![
        geometry::Point::new(0.0, 0.0, 0.0),
        geometry::Point::new(1.0, 0.0, 0.0),
        geometry::Point::new(1.0, 1.0, 0.0),
        geometry::Point::new(0.0, 1.0, 0.0),
    ];

    let triangles = vec![
        geometry::Triangle::new(0, 1, 2),
        geometry::Triangle::new(0, 2, 2),
    ];

    let materials = vec![geometry::Material {
        absorption: [0.1, 0.1, 0.1],
        scattering: 0.5,
        transmission: [0.2, 0.2, 0.2],
    }];

    // Both triangles use the same material.
    let material_indices = vec![0, 0];

    let static_mesh_settings = geometry::StaticMeshSettings {
        vertices: &vertices,
        triangles: &triangles,
        material_indices: &material_indices,
        materials: &materials,
    };

    let static_mesh = StaticMesh::try_new(&sub_scene, &static_mesh_settings).unwrap();
    sub_scene.add_static_mesh(static_mesh);
    sub_scene.commit();

    let transform = Matrix::new([
        [1.0, 0.0, 0.0, 5.0], // Move 5 meters along the X axis.
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]);

    let instanced_mesh_settings = geometry::InstancedMeshSettings {
        sub_scene: &sub_scene,
        transform,
    };
    let instanced_mesh = InstancedMesh::try_new(&main_scene, &instanced_mesh_settings).unwrap();
    let handle = main_scene.add_instanced_mesh(instanced_mesh);
    main_scene.commit();

    let new_transform = Matrix::new([
        [1.0, 0.0, 0.0, 10.0], // Move 10 meters along the X axis.
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]);

    main_scene.update_instanced_mesh_transform(handle, new_transform);
    main_scene.commit();
}

#[test]
fn test_scene_serialization() {
    let context = Context::default();
    let scene = Scene::try_new(&context).unwrap();

    // Four vertices of a unit square in the x-y plane.
    let vertices = vec![
        geometry::Point::new(0.0, 0.0, 0.0),
        geometry::Point::new(1.0, 0.0, 0.0),
        geometry::Point::new(1.0, 1.0, 0.0),
        geometry::Point::new(0.0, 1.0, 0.0),
    ];

    let triangles = vec![
        geometry::Triangle::new(0, 1, 2),
        geometry::Triangle::new(0, 2, 2),
    ];

    let materials = vec![geometry::Material {
        absorption: [0.1, 0.1, 0.1],
        scattering: 0.5,
        transmission: [0.2, 0.2, 0.2],
    }];

    // Both triangles use the same material.
    let material_indices = vec![0, 0];

    let static_mesh_settings = geometry::StaticMeshSettings {
        vertices: &vertices,
        triangles: &triangles,
        material_indices: &material_indices,
        materials: &materials,
    };

    let static_mesh = StaticMesh::try_new(&scene, &static_mesh_settings).unwrap();

    let mut serialized_object = SerializedObject::try_new(&context).unwrap();

    static_mesh.save(&mut serialized_object);

    let loaded_static_mesh_result =
        StaticMesh::<DefaultRayTracer>::load(&scene, &mut serialized_object);
    assert!(loaded_static_mesh_result.is_ok());
}

#[test]
fn test_probe_generation() {
    let context = Context::default();
    let scene = Scene::try_new(&context).unwrap();

    // This specifies a 100x100x100 axis-aligned box.
    let box_transform = Matrix::new([
        [100.0, 0.0, 0.0, 0.0],
        [0.0, 100.0, 0.0, 0.0],
        [0.0, 0.0, 100.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]);

    let mut probe_array = ProbeArray::try_new(&context).unwrap();

    let probe_params = ProbeGenerationParams::UniformFloor {
        spacing: 2.0,
        height: 1.5,
        transform: box_transform,
    };
    probe_array.generate_probes(&scene, &probe_params);

    let mut probe_batch = ProbeBatch::try_new(&context).unwrap();
    probe_batch.add_probe_array(&probe_array);

    probe_batch.commit();
}

#[test]
pub fn test_baking() {
    let context = Context::default();
    let sampling_rate = 48000;
    let frame_size = 1024;
    let max_order = 1;

    let simulation_settings = SimulationSettings::new(sampling_rate, frame_size, max_order)
        .with_direct(DirectSimulationSettings {
            max_num_occlusion_samples: 4,
        })
        .with_reflections(ReflectionsSimulationSettings::Convolution {
            max_num_rays: 4096,
            num_diffuse_samples: 32,
            max_duration: 2.0,
            max_num_sources: 8,
            num_threads: 2,
        })
        .with_pathing(PathingSimulationSettings {
            num_visibility_samples: 4,
        });
    let mut simulator = Simulator::try_new(&context, &simulation_settings).unwrap();

    let scene = Scene::try_new(&context).unwrap();
    simulator.set_scene(&scene);

    // This specifies a 100x100x100 axis-aligned box.
    let box_transform = Matrix::new([
        [100.0, 0.0, 0.0, 0.0],
        [0.0, 100.0, 0.0, 0.0],
        [0.0, 0.0, 100.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]);

    let mut probe_array = ProbeArray::try_new(&context).unwrap();

    let probe_params = ProbeGenerationParams::UniformFloor {
        spacing: 2.0,
        height: 1.5,
        transform: box_transform,
    };
    probe_array.generate_probes(&scene, &probe_params);

    let mut probe_batch = ProbeBatch::try_new(&context).unwrap();
    probe_batch.add_probe_array(&probe_array);

    probe_batch.commit();

    let identifier = BakedDataIdentifier::Reflections {
        variation: BakedDataVariation::StaticSource {
            endpoint_influence: Sphere {
                center: Point::default(), // World-space position of the souce.
                radius: 100.0, // Only bake reflections for probes within 100m of the source.
            },
        },
    };

    let reflections_bake_params = ReflectionsBakeParams {
        identifier,
        bake_flags: ReflectionsBakeFlags::BAKE_CONVOLUTION,
        num_rays: 32768,
        num_diffuse_samples: 1024,
        num_bounces: 64,
        simulated_duration: 2.0,
        saved_duration: 2.0,
        order: 2,
        num_threads: 8,
        irradiance_min_distance: 1.0,
        bake_batch_size: 0,
    };
    ReflectionsBaker::<DefaultRayTracer>::new()
        .bake(&context, &mut probe_batch, &scene, reflections_bake_params)
        .unwrap();

    simulator.add_probe_batch(&probe_batch);
    simulator.commit();
}
