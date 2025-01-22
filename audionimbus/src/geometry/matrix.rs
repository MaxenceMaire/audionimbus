/// A ROWSxCOLS matrix of type T elements.
#[derive(Debug)]
pub struct Matrix<T, const ROWS: usize, const COLS: usize> {
    /// Matrix elements, in row-major order.
    pub elements: [[T; COLS]; ROWS],
}
