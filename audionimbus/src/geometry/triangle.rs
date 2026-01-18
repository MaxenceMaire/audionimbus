/// A triangle in 3D space.
///
/// Triangles are specified by their three vertices, which are in turn specified using indices into a vertex array.
///
/// Steam Audio uses a counter-clockwise winding order.
/// This means that when looking at the triangle such that the normal is pointing towards you, the vertices are specified in counter-clockwise order.
///
/// Each triangle must be specified using three vertices; triangle strip or fan representations are not supported.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Triangle {
    /// Indices of the three vertices of this triangle.
    pub indices: [i32; 3],
}

impl Triangle {
    /// Creates a new triangle.
    pub fn new(vertex_index_0: i32, vertex_index_1: i32, vertex_index_2: i32) -> Self {
        Self {
            indices: [vertex_index_0, vertex_index_1, vertex_index_2],
        }
    }
}

impl From<Triangle> for audionimbus_sys::IPLTriangle {
    fn from(triangle: Triangle) -> Self {
        audionimbus_sys::IPLTriangle {
            indices: triangle.indices,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_triangle_new() {
        let t = Triangle::new(0, 1, 2);
        assert_eq!(t, Triangle { indices: [0, 1, 2] });
    }
}
