/// Errors that can occur during baking operations.
#[derive(Debug, PartialEq, Eq)]
pub enum BakeError {
    /// Another bake operation is already in progress.
    BakeInProgress,

    /// The scene and reflection baker use different ray tracers.
    #[cfg(feature = "dynamic")]
    RayTracerMismatch {
        /// The baker's ray tracer.
        expected: crate::dynamic::RayTracerKind,
        /// The scene's ray tracer.
        actual: crate::dynamic::RayTracerKind,
    },

    /// The scene and reflection baker use different devices.
    #[cfg(feature = "dynamic")]
    DeviceMismatch,
}

impl std::error::Error for BakeError {}

impl std::fmt::Display for BakeError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::BakeInProgress => {
                write!(f, "another bake operation is already in progress")
            }
            #[cfg(feature = "dynamic")]
            Self::RayTracerMismatch { expected, actual } => write!(
                f,
                "bake ray tracer mismatch: expected {expected:?}, got {actual:?}"
            ),
            #[cfg(feature = "dynamic")]
            Self::DeviceMismatch => write!(f, "bake devices do not match"),
        }
    }
}
