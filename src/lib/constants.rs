/// Shared numeric constants for floating-point operations.

/// Epsilon for general floating-point comparisons.
/// Use for: pixel coordinates, time differences, distances, color values.
/// Handles typical f32 precision (~7 significant digits).
pub const EPSILON: f32 = 1e-6;

/// Epsilon for very small value checks (near-zero detection).
/// Use for: homogeneous scale factors, degenerate geometry detection.
pub const EPSILON_TINY: f32 = 1e-10;
