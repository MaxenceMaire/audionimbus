use audionimbus::*;

fn static_mesh(scene: &Scene) -> StaticMesh {
    let vertices = vec![
        Point::new(0.0, 0.0, 0.0),
        Point::new(1.0, 0.0, 0.0),
        Point::new(1.0, 1.0, 0.0),
        Point::new(0.0, 1.0, 0.0),
    ];

    let triangles = vec![
        geometry::Triangle::new(0, 1, 2),
        geometry::Triangle::new(0, 2, 3),
    ];

    let materials = vec![geometry::Material::default()];
    let material_indices = vec![0, 0];

    let settings = geometry::StaticMeshSettings {
        vertices: &vertices,
        triangles: &triangles,
        material_indices: &material_indices,
        materials: &materials,
    };

    StaticMesh::try_new(scene, &settings).expect("failed to create static mesh")
}

#[test]
fn test_static_mesh_save_load() {
    let context = Context::default();
    let scene = Scene::try_new(&context).unwrap();
    let static_mesh = static_mesh(&scene);

    let mut serialized = SerializedObject::try_new(&context).unwrap();
    static_mesh.save(&mut serialized);

    let loaded = StaticMesh::load(&scene, &mut serialized);
    assert!(loaded.is_ok());
}

#[test]
fn test_static_mesh_to_vec() {
    let context = Context::default();
    let scene = Scene::try_new(&context).unwrap();
    let static_mesh = static_mesh(&scene);

    let mut serialized = SerializedObject::try_new(&context).unwrap();
    static_mesh.save(&mut serialized);

    let data = serialized.to_vec();
    assert!(!data.is_empty());
}

#[test]
fn test_scene_save_obj() {
    let context = Context::default();
    let mut scene = Scene::try_new(&context).unwrap();
    let static_mesh = static_mesh(&scene);

    scene.add_static_mesh(static_mesh);
    scene.commit();

    // Save to a temporary file.
    let temp_file = std::env::temp_dir().join("test_scene.obj");
    scene.save_obj(temp_file.to_str().unwrap().to_string());

    // Verify file was created.
    assert!(temp_file.exists());

    // Clean up.
    let _ = std::fs::remove_file(temp_file);
}

#[test]
fn test_probe_batch_save_load() {
    let context = Context::default();
    let mut probe_batch = ProbeBatch::try_new(&context).unwrap();

    let probe = Sphere {
        center: Point::new(1.0, 2.0, 3.0),
        radius: 5.0,
    };
    probe_batch.add_probe(probe);
    probe_batch.commit();

    let mut serialized = SerializedObject::try_new(&context).unwrap();
    probe_batch.save(&mut serialized);

    let loaded_batch = ProbeBatch::load(&context, &mut serialized);
    assert!(loaded_batch.is_ok());
    assert_eq!(loaded_batch.unwrap().num_probes(), 1);
}
