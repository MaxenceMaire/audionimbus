/// A ROWSxCOLS matrix of type T elements.
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct Matrix<T, const ROWS: usize, const COLS: usize> {
    /// Matrix elements, in row-major order.
    pub elements: [[T; COLS]; ROWS],
}

impl From<Matrix<f32, 4, 4>> for audionimbus_sys::IPLMatrix4x4 {
    fn from(matrix: Matrix<f32, 4, 4>) -> Self {
        audionimbus_sys::IPLMatrix4x4 {
            elements: matrix.elements,
        }
    }
}

impl From<&Matrix<f32, 4, 4>> for audionimbus_sys::IPLMatrix4x4 {
    fn from(matrix: &Matrix<f32, 4, 4>) -> Self {
        audionimbus_sys::IPLMatrix4x4 {
            elements: matrix.elements,
        }
    }
}
