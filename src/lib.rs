//! # Bird Barrier
//!
//! A Bevy plugin for coordinating setup tasks with dependency management - wait for all
//! birds to gather before flying together.
//!
//! Like a synchronization barrier in concurrent programming, Bird Barrier ensures all your
//! setup tasks complete before proceeding to the next phase of your application.
//!
//! This crate allows you to define setup tasks with dependencies and track their progress,
//! ensuring that tasks run in the correct order and providing real-time feedback on
//! initialization progress.
//!
//! ## Examples
//!
//! - **Basic Usage**: See `examples/basic_usage.rs` for simple enum-based keys (recommended starting point)
//! - **Advanced Patterns**: See `examples/trait_object_keys.rs` for trait object-based keys
//!   that enable polymorphic setup providers
//!
//! ## Features
//!
//! - **Dependency Management**: Define setup tasks with requirements and provisions
//! - **Progress Tracking**: Monitor the progress of individual tasks and overall setup
//! - **Automatic Scheduling**: Tasks run automatically when their dependencies are satisfied
//! - **Validation**: Detect missing providers, duplicate providers, and cyclic dependencies
//! - **Flexible Progress Calculation**: Custom progress checkers and relative time estimates
//!
//! ## Features
//!
//! - `assets`: Enable asset loading progress tracking helpers
//! - `reflect`: Enable reflection support for setup keys
//! - `debug`: Enable additional debugging features
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use bevy::prelude::*;
//! use bevy::ecs::system::SystemId;
//! use bird_barrier::*;
//!
//! // Define your setup keys
//! #[derive(Debug, Clone, PartialEq, Eq, Hash)]
//! enum MySetupKey {
//!     LoadAssets,
//!     BuildScene,
//!     InitializeGame,
//! }
//!
//! impl SetupKey for MySetupKey {
//!     fn register_progress_checker(&self, world: &mut World) -> SystemId<(), Progress> {
//!         match self {
//!             MySetupKey::LoadAssets => world.register_system(check_assets_loaded),
//!             MySetupKey::BuildScene => world.register_system(check_scene_built),
//!             MySetupKey::InitializeGame => world.register_system(check_game_initialized),
//!         }
//!     }
//! }
//!
//! fn check_assets_loaded() -> Progress {
//!     // Your progress checking logic here
//!     Progress::DONE
//! }
//!
//! fn check_scene_built() -> Progress {
//!     Progress::DONE
//! }
//!
//! fn check_game_initialized() -> Progress {
//!     Progress::DONE
//! }
//!
//! fn setup_complete() {
//!     println!("Setup complete!");
//! }
//!
//! fn main() {
//!     // Example setup - in a real app you'd configure the plugin properly
//!     let _app = App::new();
//!     // .add_plugins(SetupTrackingPlugin::<MySetupKey, _, _, _>::new(
//!     //     || true,  // Condition
//!     //     setup_complete,  // Completion callback
//!     // ))
//!     // .run();
//! }
//! ```

use bevy_ecs::{prelude::*, query::QueryFilter, system::SystemId};
use bevy_state::{prelude::State, state::States};
use std::hash::Hash;

#[cfg(feature = "assets")]
use bevy_asset::{AssetServer, UntypedAssetId};

mod plugin;
mod progress;
mod provider;
mod tracker;

pub use plugin::*;
pub use progress::*;
pub use provider::*;
pub use tracker::*;

/// Implement this trait for a type that defines a single unit of setup, which can be provided by
/// and/or depended on by [Provider]s.
///
/// Setup keys represent different stages or components of your application's initialization.
/// Each key must be able to register a progress checker system and optionally provide
/// a relative time estimate for progress calculation.
pub trait SetupKey: Eq + Hash + Clone + Send + Sync + 'static {
	/// Returns the system that calculates the progress of this setup entry.
	///
	/// This will be called the first time each key appears in a [Provider]'s `requires` or
	/// `provides` list. The SystemId will be cached and used for any further appearances.
	fn register_progress_checker(&self, world: &mut World) -> SystemId<(), Progress>;

	/// A scale factor to apply to this entry when calculating total progress.
	///
	/// This allows you to weight different setup tasks based on their expected duration
	/// or importance. Defaults to `1.0`.
	fn relative_time_estimate(&self) -> f32 {
		1.0
	}
}

/// Type alias for progress checker system IDs.
pub type ProgressCheckerId = SystemId<(), Progress>;

/// Helper function to check progress based on whether a single entity exists matching the given filter.
pub fn single_spawn_progress<F: QueryFilter>(q: Option<Single<(), F>>) -> Progress {
	q.is_some().into()
}

/// Helper function to check progress based on whether a resource exists.
pub fn resource_progress<R: Resource>(res: Option<Res<R>>) -> Progress {
	res.is_some().into()
}

/// Helper function to create a progress checker for a specific state.
pub fn state_progress<S: States>(state: S) -> impl System<In = (), Out = Progress> {
	IntoSystem::into_system(move |curr: Option<Res<State<S>>>| {
		curr.map(|curr| (*curr.get() == state).into())
			.unwrap_or_default()
	})
}

#[cfg(feature = "assets")]
/// Helper function to check asset loading progress for an asset collection.
pub fn assets_progress<C: AssetCollection>(
	collection: Option<Res<C>>,
	server: Res<AssetServer>,
) -> Progress {
	let Some(collection) = collection else {
		return Progress::ZERO;
	};

	let (done, total) = collection.iter_ids().fold((0, 0), |(done, total), id| {
		let Some(state) = server.get_load_state(id) else {
			return (done, total + 1);
		};

		let done = if state.is_loaded() { done + 1 } else { done };

		(done, total + 1)
	});

	Progress::new(done as f32 / total as f32)
}

#[cfg(feature = "assets")]
/// Trait for asset collections that can be tracked for loading progress.
pub trait AssetCollection: Resource {
	/// Returns an iterator over all asset IDs in this collection.
	fn iter_ids(&self) -> impl Iterator<Item = UntypedAssetId>;
}

#[cfg(feature = "assets")]
/// Helper system to load assets for an asset collection.
pub fn load_assets<C: AssetCollection + FromWorld>(mut cmds: Commands, collection: Option<Res<C>>) {
	if collection.is_some() {
		return;
	}

	cmds.init_resource::<C>();
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_setup_key_trait() {
		#[derive(Debug, Clone, PartialEq, Eq, Hash)]
		enum TestSetupKey {
			A,
			B,
		}

		impl SetupKey for TestSetupKey {
			fn register_progress_checker(&self, world: &mut World) -> SystemId<(), Progress> {
				match self {
					TestSetupKey::A => world.register_system(|| Progress::new(0.5)),
					TestSetupKey::B => world.register_system(|| Progress::DONE),
				}
			}

			fn relative_time_estimate(&self) -> f32 {
				match self {
					TestSetupKey::A => 2.0,
					TestSetupKey::B => 1.0,
				}
			}
		}

		let mut world = World::new();

		let key_a = TestSetupKey::A;
		let key_b = TestSetupKey::B;

		// Test progress checker registration
		let system_a = key_a.register_progress_checker(&mut world);
		let system_b = key_b.register_progress_checker(&mut world);

		let progress_a = world.run_system(system_a).unwrap();
		let progress_b = world.run_system(system_b).unwrap();

		assert_eq!(progress_a, Progress::new(0.5));
		assert_eq!(progress_b, Progress::DONE);

		// Test relative time estimates
		assert_eq!(key_a.relative_time_estimate(), 2.0);
		assert_eq!(key_b.relative_time_estimate(), 1.0);
	}
}
