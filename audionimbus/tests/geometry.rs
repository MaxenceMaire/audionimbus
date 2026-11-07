#[cfg(static_api)]
mod r#static {
    #[cfg(not(feature = "static"))]
    use audionimbus as static_api;
    use audionimbus::geometry;
    #[cfg(feature = "static")]
    use audionimbus::r#static as static_api;
    use static_api::*;

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

        let instanced_mesh_settings = static_api::InstancedMeshSettings {
            sub_scene,
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
        let scene = Scene::try_new(&context).unwrap();
        let mut scene: static_api::Scene<DefaultRayTracer> = scene;

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

        let sub_scene = Scene::<DefaultRayTracer>::try_new(&context).unwrap();
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
}

#[cfg(feature = "dynamic")]
mod dynamic {

    use audionimbus::callback::{
        AnyHitCallback, BatchedAnyHitCallback, BatchedClosestHitCallback, ClosestHitCallback,
        CustomRayTracingCallbacks, ProgressCallback,
    };
    use audionimbus::dynamic::{
        InstancedMesh, InstancedMeshSettings, RayTracerKind, RayTracerSettings, Scene, SceneError,
        StaticMesh,
    };
    use audionimbus::{
        Context, Material, Matrix, Point, StaticMeshError, StaticMeshSettings, SteamAudioError,
        Triangle,
    };
    use std::sync::Arc;

    mod scene {
        use super::*;

        #[test]
        fn rejects_mixed_geometry() {
            let context = Context::default();
            let default_scene: Result<_, SteamAudioError> =
                Scene::try_new(&context, RayTracerSettings::Default);
            let mut default_scene = default_scene.unwrap();
            let mut custom_scene = Scene::try_new(
                &context,
                RayTracerSettings::Custom {
                    callbacks: CustomRayTracingCallbacks::new(
                        ClosestHitCallback::new(|_, _, _| None),
                        AnyHitCallback::new(|_, _, _| false),
                        BatchedClosestHitCallback::new(|rays, _, _| vec![None; rays.len()]),
                        BatchedAnyHitCallback::new(|rays, _, _| vec![false; rays.len()]),
                    ),
                    ray_batch_size: 16,
                },
            )
            .unwrap();
            let mesh: Result<_, StaticMeshError> = StaticMesh::try_new(
                &default_scene,
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
            );
            let mesh = mesh.unwrap();
            assert!(matches!(
                custom_scene.try_add_static_mesh(mesh.clone()),
                Err(SceneError::RayTracerMismatch {
                    expected: RayTracerKind::Custom,
                    actual: RayTracerKind::Default,
                })
            ));
            let handle = default_scene.try_add_static_mesh(mesh).unwrap();
            default_scene.commit();
            assert!(default_scene.remove_static_mesh(handle));
            assert!(!default_scene.remove_static_mesh(handle));
            default_scene.commit();

            let transform = Matrix::<f32, 4, 4>::IDENTITY;
            for (scene, sub_scene, parent_ray_tracer, sub_scene_ray_tracer) in [
                (
                    &default_scene,
                    &custom_scene,
                    RayTracerKind::Default,
                    RayTracerKind::Custom,
                ),
                (
                    &custom_scene,
                    &default_scene,
                    RayTracerKind::Custom,
                    RayTracerKind::Default,
                ),
            ] {
                assert!(matches!(
                    InstancedMesh::try_new(scene, &InstancedMeshSettings {
                        sub_scene: sub_scene.clone(),
                        transform,
                    }),
                    Err(SceneError::RayTracerMismatch { expected: expected_ray_tracer, actual: actual_ray_tracer })
                        if expected_ray_tracer == parent_ray_tracer && actual_ray_tracer == sub_scene_ray_tracer
                ));
            }
            let instance = InstancedMesh::try_new(
                &default_scene,
                &InstancedMeshSettings {
                    sub_scene: Scene::try_new(&context, RayTracerSettings::Default).unwrap(),
                    transform,
                },
            )
            .unwrap();
            assert!(matches!(
                custom_scene.try_add_instanced_mesh(instance.clone()),
                Err(SceneError::RayTracerMismatch {
                    expected: RayTracerKind::Custom,
                    actual: RayTracerKind::Default,
                })
            ));
            let handle = default_scene.try_add_instanced_mesh(instance).unwrap();
            default_scene.commit();
            assert!(default_scene.update_instanced_mesh_transform(handle, transform));
            assert!(default_scene.remove_instanced_mesh(handle));
            assert!(!default_scene.remove_instanced_mesh(handle));
            assert!(!default_scene.update_instanced_mesh_transform(handle, transform));
            default_scene.commit();
        }

        #[test]
        fn retains_custom_callbacks() {
            let context = Context::default();
            let callback_lifetime = Arc::new(());
            let retained_callbacks = Arc::downgrade(&callback_lifetime);
            let scene = Scene::try_new(
                &context,
                RayTracerSettings::Custom {
                    callbacks: CustomRayTracingCallbacks::new(
                        ClosestHitCallback::new(move |_, _, _| {
                            let _ = &callback_lifetime;
                            None
                        }),
                        AnyHitCallback::new(|_, _, _| false),
                        BatchedClosestHitCallback::new(|rays, _, _| vec![None; rays.len()]),
                        BatchedAnyHitCallback::new(|rays, _, _| vec![false; rays.len()]),
                    ),
                    ray_batch_size: 16,
                },
            )
            .unwrap();
            let scene_clone = scene.clone();
            drop(scene);
            scene_clone.commit();
            assert!(retained_callbacks.upgrade().is_some());
            assert!(matches!(
                scene_clone.try_save(&context),
                Err(SceneError::SerializationUnsupported(RayTracerKind::Custom))
            ));
            assert!(matches!(
                scene_clone.try_save_obj(String::new()),
                Err(SceneError::ObjUnsupported(RayTracerKind::Custom))
            ));
            drop(scene_clone);
            assert!(retained_callbacks.upgrade().is_none());
        }

        #[test]
        fn releases_load_callbacks() {
            let context = Context::default();
            let mut scene = Scene::try_new(&context, RayTracerSettings::Default).unwrap();
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
            let mesh_bytes = mesh.try_save(&context).unwrap();
            scene.try_add_static_mesh(mesh).unwrap();
            scene.commit();
            let scene_bytes = scene.try_save(&context).unwrap();
            drop(scene);

            let progress_lifetime = Arc::new(());
            let retained_progress_lifetime = Arc::clone(&progress_lifetime);
            let scene = Scene::load_with_progress(
                &context,
                RayTracerSettings::Default,
                &scene_bytes,
                ProgressCallback::new(move |_| {
                    let _ = &retained_progress_lifetime;
                }),
            )
            .unwrap();
            assert_eq!(Arc::strong_count(&progress_lifetime), 1);
            let retained_progress_lifetime = Arc::clone(&progress_lifetime);
            let mesh = StaticMesh::load_with_progress_callback(
                &scene,
                &mesh_bytes,
                ProgressCallback::new(move |_| {
                    let _ = &retained_progress_lifetime;
                }),
            )
            .unwrap();
            assert_eq!(Arc::strong_count(&progress_lifetime), 1);
            let mut scene_clone = scene.clone();
            drop(scene);
            let handle = scene_clone.try_add_static_mesh(mesh).unwrap();
            scene_clone.commit();
            assert!(scene_clone.remove_static_mesh(handle));
            scene_clone.commit();
            let loaded: Result<_, SteamAudioError> =
                Scene::load(&context, RayTracerSettings::Default, &scene_bytes);
            let loaded = loaded.unwrap();
            let mesh: Result<_, SteamAudioError> = StaticMesh::load(&loaded, &mesh_bytes);
            let mesh = mesh.unwrap();
            assert_eq!(
                mesh.try_save(&context).unwrap().to_vec(),
                mesh_bytes.to_vec()
            );
        }
    }
}
