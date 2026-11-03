#![cfg(feature = "dynamic")]

use audionimbus::callback::{
    AnyHitCallback, BatchedAnyHitCallback, BatchedClosestHitCallback, ClosestHitCallback,
    CustomRayTracingCallbacks, ProgressCallback,
};
use audionimbus::dynamic::{
    InstancedMesh, InstancedMeshSettings, RayTracerKind, RayTracerSettings, Scene, SceneError,
    StaticMesh,
};
use audionimbus::{Context, Material, Matrix, Point, StaticMeshSettings, Triangle};
use std::sync::Arc;

mod scene {
    use super::*;

    #[test]
    fn rejects_mixed_geometry() {
        let context = Context::default();
        let mut default_scene = Scene::try_new(&context, RayTracerSettings::Default).unwrap();
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
        let mesh = StaticMesh::try_new(
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
        )
        .unwrap();
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
        let loaded = Scene::load(&context, RayTracerSettings::Default, &scene_bytes).unwrap();
        let mesh = StaticMesh::load(&loaded, &mesh_bytes).unwrap();
        assert_eq!(
            mesh.try_save(&context).unwrap().to_vec(),
            mesh_bytes.to_vec()
        );
    }
}
