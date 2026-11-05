#[cfg(any(feature = "static", not(feature = "dynamic")))]
mod r#static {
    #[cfg(not(feature = "static"))]
    use audionimbus as static_api;
    #[cfg(feature = "static")]
    use audionimbus::r#static as static_api;
    use static_api::*;

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
}

#[cfg(feature = "dynamic")]
mod dynamic {
    use audionimbus::dynamic as runtime;
    use audionimbus::*;

    #[test]
    fn generates_custom_scene_probes() {
        use audionimbus::callback::{
            AnyHitCallback, BatchedAnyHitCallback, BatchedClosestHitCallback, ClosestHitCallback,
            CustomRayTracingCallbacks,
        };
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };
        let context = Context::default();
        let ray_calls = Arc::new(AtomicUsize::new(0));
        let floor_hit = |ray: Ray, min_distance: f32, max_distance: f32| {
            let distance = -ray.origin.y / ray.direction.y;
            if ray.direction.y < 0.0 && distance >= min_distance && distance <= max_distance {
                Some(Hit {
                    distance,
                    triangle_index: Some(0),
                    object_index: Some(0),
                    material_index: Some(0),
                    normal: Vector3::new(0.0, 1.0, 0.0),
                    material: Some(Material::CONCRETE),
                })
            } else {
                None
            }
        };
        let scene = runtime::Scene::try_new(
            &context,
            runtime::RayTracerSettings::Custom {
                callbacks: CustomRayTracingCallbacks::new(
                    ClosestHitCallback::new({
                        let ray_calls = ray_calls.clone();
                        move |ray, min, max| {
                            ray_calls.fetch_add(1, Ordering::Relaxed);
                            floor_hit(ray, min, max)
                        }
                    }),
                    AnyHitCallback::new(|_, _, _| false),
                    BatchedClosestHitCallback::new(move |rays, min, max| {
                        rays.iter()
                            .zip(min)
                            .zip(max)
                            .map(|((ray, min), max)| floor_hit(*ray, *min, *max))
                            .collect()
                    }),
                    BatchedAnyHitCallback::new(|rays, _, _| vec![false; rays.len()]),
                ),
                ray_batch_size: 16,
            },
        )
        .unwrap();
        let mut probe_array = ProbeArray::try_new(&context).unwrap();
        probe_array.generate_probes(
            &scene,
            &ProbeGenerationParams::UniformFloor {
                spacing: 2.0,
                height: 1.0,
                transform: Matrix4::new([
                    [4.0, 0.0, 0.0, 0.0],
                    [0.0, 4.0, 0.0, 0.0],
                    [0.0, 0.0, 4.0, 0.0],
                    [0.0, 0.0, 0.0, 1.0],
                ]),
            },
        );
        assert!(ray_calls.load(Ordering::Relaxed) > 0);
        assert!(probe_array.num_probes() > 0);
        assert!(
            probe_array
                .probes()
                .all(|probe| (probe.center.y - 1.0).abs() < 0.01)
        );
    }
}
