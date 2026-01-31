#[derive(Debug, PartialEq)]
pub enum BakeError {
    /// Another bake operation is already in progress.
    BakeInProgress,
}

impl std::error::Error for BakeError {}

impl std::fmt::Display for BakeError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::BakeInProgress => {
                write!(f, "another bake operation is already in progress")
            }
        }
    }
}
