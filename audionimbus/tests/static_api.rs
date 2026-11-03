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
