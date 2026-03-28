use audionimbus::{Material, Point, RayTracer, Scene, StaticMesh, StaticMeshSettings, Triangle};

/// Constructs a closed rectangular room and returns it as a [`StaticMesh`].
///
/// The floor lies at `y = 0` and the room is centered on the xz-plane.
/// `material` is applied uniformly to all six faces.
pub fn room<T: RayTracer>(
    scene: &Scene<T>,
    width: f32,
    height: f32,
    depth: f32,
    material: Material,
) -> StaticMesh<T> {
    let half_width = width / 2.0;
    let half_depth = depth / 2.0;

    #[rustfmt::skip]
    let vertices = [
        // Floor (y = 0)
        Point::new(-half_width, 0.0,    -half_depth),
        Point::new( half_width, 0.0,    -half_depth),
        Point::new( half_width, 0.0,     half_depth),
        Point::new(-half_width, 0.0,     half_depth),
        // Ceiling (y = height)
        Point::new(-half_width, height, -half_depth),
        Point::new( half_width, height, -half_depth),
        Point::new( half_width, height,  half_depth),
        Point::new(-half_width, height,  half_depth),
    ];

    let triangles = vec![
        // Floor
        Triangle::new(0, 2, 1),
        Triangle::new(0, 3, 2),
        // Ceiling
        Triangle::new(4, 5, 6),
        Triangle::new(4, 6, 7),
        // Rront (z = -half_depth)
        Triangle::new(0, 1, 5),
        Triangle::new(0, 5, 4),
        // Back (z = half_depth)
        Triangle::new(3, 7, 6),
        Triangle::new(3, 6, 2),
        // Left (x = -half_width)
        Triangle::new(0, 4, 7),
        Triangle::new(0, 7, 3),
        // Right (x = half_width)
        Triangle::new(1, 2, 6),
        Triangle::new(1, 6, 5),
    ];

    StaticMesh::try_new(
        scene,
        &StaticMeshSettings {
            vertices: &vertices,
            triangles: &triangles,
            material_indices: &vec![0; triangles.len()],
            materials: &[material],
        },
    )
    .unwrap()
}
