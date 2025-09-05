use nutype::nutype;
use std::fmt::Formatter;

/// Represents the progress of a setup task as a value between 0.0 and 1.0.
///
/// Finite progress values are automatically clamped to the valid range [0.0, 1.0].
/// Non-finite values (NaN, infinity) are preserved to allow applications to represent
/// special states such as:
/// - Failed or errored tasks
/// - Tasks with unknown or infinite duration
/// - Undefined or uninitialized progress states
/// - Other application-specific conditions
///
/// The interpretation of non-finite values is left to the application, providing
/// flexibility for different use cases and semantic meanings.
///
/// # Examples
///
/// ```rust
/// use bird_barrier::Progress;
///
/// let progress = Progress::new(0.5);
/// assert_eq!(*progress, 0.5);
///
/// let done = Progress::DONE;
/// assert!(done.finished());
///
/// let from_bool: Progress = true.into();
/// assert_eq!(from_bool, Progress::DONE);
///
/// // Non-finite values are preserved for application-specific meaning
/// let failed_task = Progress::new(f32::NAN);
/// assert!(!failed_task.is_finite());
///
/// let infinite_task = Progress::new(f32::INFINITY);
/// assert!(!infinite_task.is_finite());
/// ```
#[nutype(
    const_fn,
    sanitize(with = clamp_finite_0_to_1),
    derive(Default, Debug, Deref, Clone, Copy, PartialEq, PartialOrd),
    default = 0.0,
)]
pub struct Progress(f32);

/// Clamps finite values to [0.0, 1.0], preserves non-finite values.
const fn clamp_finite_0_to_1(val: f32) -> f32 {
	if val.is_finite() {
		val.clamp(0.0, 1.0)
	} else {
		val
	}
}

impl Progress {
	/// Progress value representing no progress (0.0).
	pub const ZERO: Self = Self::new(0.0);

	/// Progress value representing complete progress (1.0).
	pub const DONE: Self = Self::new(1.0);

	/// Returns true if the progress is considered finished (>= 1.0 - f32::EPSILON).
	///
	/// Non-finite values are never considered finished.
	pub fn finished(self) -> bool {
		let value = *self;
		value.is_finite() && value >= 1.0 - f32::EPSILON
	}

	/// Returns true if this progress has a finite, meaningful value.
	///
	/// Non-finite values (NaN, infinity) are preserved for application-specific
	/// semantic meaning, such as representing failed states, unknown durations,
	/// or other special conditions.
	pub fn is_finite(self) -> bool {
		(*self).is_finite()
	}
}

impl std::fmt::Display for Progress {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		// Keeps formatting args (precision, padding, etc.)
		// Could maybe be improved to incorporate extra character in padding
		std::fmt::Display::fmt(&(**self * 100.0), f)?;
		f.write_str("%")
	}
}

impl From<bool> for Progress {
	/// Converts a boolean to Progress: true becomes DONE, false becomes ZERO.
	fn from(val: bool) -> Self {
		Self::new(if val { 1.0 } else { 0.0 })
	}
}

// Note: The question mark operator for Option types would require unstable features.
// For now, users can use .is_some().into() or similar patterns.

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_progress_creation() {
		let progress = Progress::new(0.5);
		assert_eq!(*progress, 0.5);

		let zero = Progress::ZERO;
		assert_eq!(*zero, 0.0);

		let done = Progress::DONE;
		assert_eq!(*done, 1.0);
	}

	#[test]
	fn test_progress_clamping() {
		// Values above 1.0 should be clamped to 1.0
		let over = Progress::new(1.5);
		assert_eq!(*over, 1.0);

		// Values below 0.0 should be clamped to 0.0
		let under = Progress::new(-0.5);
		assert_eq!(*under, 0.0);

		// Normal values should pass through
		let normal = Progress::new(0.75);
		assert_eq!(*normal, 0.75);
	}

	#[test]
	fn test_progress_finished() {
		assert!(Progress::DONE.finished());
		assert!(Progress::new(1.0).finished());
		assert!(!Progress::new(0.99).finished());
		assert!(!Progress::ZERO.finished());

		// Test near 1.0 values
		let almost_done = Progress::new(1.0 - f32::EPSILON / 2.0);
		assert!(almost_done.finished());

		// Non-finite values are never finished
		assert!(!Progress::new(f32::NAN).finished());
		assert!(!Progress::new(f32::INFINITY).finished());
	}

	#[test]
	fn test_progress_from_bool() {
		let from_true: Progress = true.into();
		assert_eq!(from_true, Progress::DONE);

		let from_false: Progress = false.into();
		assert_eq!(from_false, Progress::ZERO);
	}

	#[test]
	fn test_progress_comparison() {
		let low = Progress::new(0.3);
		let high = Progress::new(0.7);

		assert!(low < high);
		assert!(high > low);
		assert_eq!(low, Progress::new(0.3));
		assert_ne!(low, high);
	}

	#[test]
	fn test_progress_default() {
		let default_progress = Progress::default();
		assert_eq!(default_progress, Progress::ZERO);
	}

	#[test]
	fn test_progress_non_finite_values() {
		// NaN should be preserved
		let nan_progress = Progress::new(f32::NAN);
		assert!(!nan_progress.is_finite());
		assert!((*nan_progress).is_nan());

		// Infinity should be preserved
		let inf_progress = Progress::new(f32::INFINITY);
		assert!(!inf_progress.is_finite());
		assert!((*inf_progress).is_infinite());

		// Negative infinity should be preserved
		let neg_inf_progress = Progress::new(f32::NEG_INFINITY);
		assert!(!neg_inf_progress.is_finite());
		assert!((*neg_inf_progress).is_infinite());
	}

	#[test]
	fn test_progress_constants() {
		// Test the basic constants
		assert_eq!(*Progress::ZERO, 0.0);
		assert_eq!(*Progress::DONE, 1.0);

		// Test finite checking
		assert!(Progress::DONE.is_finite());
		assert!(Progress::ZERO.is_finite());

		// Test non-finite values
		let nan_progress = Progress::new(f32::NAN);
		let inf_progress = Progress::new(f32::INFINITY);
		assert!(!nan_progress.is_finite());
		assert!(!inf_progress.is_finite());
	}
}
