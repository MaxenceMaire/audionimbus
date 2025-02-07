/// A ROWSxCOLS matrix of type T elements.
#[derive(Debug, Copy, Clone)]
pub struct Matrix<T, const ROWS: usize, const COLS: usize> {
    /// Matrix elements, in row-major order.
    pub elements: [[T; COLS]; ROWS],
}

impl<T, const ROWS: usize, const COLS: usize> Matrix<T, ROWS, COLS> {
    pub fn new(elements: [[T; COLS]; ROWS]) -> Self {
        Self { elements }
    }
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
