/// A ROWSxCOLS matrix of type T elements.
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Matrix<T, const ROWS: usize, const COLS: usize> {
    /// Matrix elements, in row-major order.
    pub elements: [[T; COLS]; ROWS],
}

impl Matrix<f32, 3, 3> {
    pub const IDENTITY: Self = Self {
        elements: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
    };
}

impl Default for Matrix<f32, 3, 3> {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Matrix<f32, 4, 4> {
    pub const IDENTITY: Self = Self {
        elements: [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ],
    };
}

impl Default for Matrix<f32, 4, 4> {
    fn default() -> Self {
        Self::IDENTITY
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_matrix3x3_identity() {
        let m = Matrix::<f32, 3, 3>::IDENTITY;

        #[rustfmt::skip]
        assert_eq!(
            m,
            Matrix {
                elements: [
                    [1.0, 0.0, 0.0],
                    [0.0, 1.0, 0.0],
                    [0.0, 0.0, 1.0],
                ]
            }
        );
    }

    #[test]
    fn test_matrix4x4_identity() {
        let m = Matrix::<f32, 4, 4>::IDENTITY;

        #[rustfmt::skip]
        assert_eq!(
            m,
            Matrix {
                elements: [
                    [1.0, 0.0, 0.0, 0.0],
                    [0.0, 1.0, 0.0, 0.0],
                    [0.0, 0.0, 1.0, 0.0],
                    [0.0, 0.0, 0.0, 1.0],
                ]
            }
        );
    }

    #[test]
    fn test_matrix_new() {
        let elements = [[1.0, 2.0], [3.0, 4.0]];
        let m = Matrix::new(elements);

        #[rustfmt::skip]
        assert_eq!(
            m,
            Matrix {
                elements: [
                    [1.0, 2.0],
                    [3.0, 4.0],
                ]
            }
        );
    }

    #[test]
    fn test_matrix4x4_default() {
        let m = Matrix::<f32, 4, 4>::default();
        assert_eq!(m, Matrix::<f32, 4, 4>::IDENTITY);
    }
}
