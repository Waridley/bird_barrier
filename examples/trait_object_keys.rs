//! # Trait Object-Based Setup Keys
//!
//! This example demonstrates how to create a flexible setup key system using trait objects.
//! This pattern allows you to define setup providers as traits, enabling polymorphic behavior
//! while maintaining type safety through the setup key system.
//!
//! This approach is useful when you want to:
//! - Define setup behavior through traits rather than concrete types
//! - Allow multiple implementations of the same setup concept
//! - Keep setup logic modular and extensible, rather than requiring all keys to be defined in a
//! single enum

use bevy::ecs::system::SystemId;
use bevy::prelude::*;
use bird_barrier::*;
use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

/// A trait that defines a setup provider's behavior.
pub trait SetupProvider: Any + Send + Sync {
	/// Human-readable name for debugging
	fn name(&self) -> &'static str;

	/// Estimate of relative time this provider takes
	fn time_estimate(&self) -> f32 {
		1.0
	}

	/// Check if this provider's work is complete
	fn check_progress(&self, world: &World) -> Progress;

	/// Downcast helper
	fn as_any(&self) -> &dyn Any;
}

/// Example asset loading provider
#[derive(Debug)]
struct AssetLoader;

impl SetupProvider for AssetLoader {
	fn name(&self) -> &'static str {
		"Asset Loader"
	}

	fn time_estimate(&self) -> f32 {
		2.0
	} // Takes longer than average

	fn check_progress(&self, world: &World) -> Progress {
		if world.contains_resource::<AssetsLoaded>() {
			Progress::DONE
		} else {
			Progress::ZERO
		}
	}

	fn as_any(&self) -> &dyn Any {
		self
	}
}

/// Example scene building provider
#[derive(Debug)]
struct SceneBuilder;

impl SetupProvider for SceneBuilder {
	fn name(&self) -> &'static str {
		"Scene Builder"
	}

	fn check_progress(&self, world: &World) -> Progress {
		if world.contains_resource::<SceneBuilt>() {
			Progress::DONE
		} else {
			Progress::ZERO
		}
	}

	fn as_any(&self) -> &dyn Any {
		self
	}
}

/// Example player spawning provider
#[derive(Debug)]
struct PlayerSpawner;

impl SetupProvider for PlayerSpawner {
	fn name(&self) -> &'static str {
		"Player Spawner"
	}

	fn check_progress(&self, world: &World) -> Progress {
		if world.contains_resource::<PlayerSpawned>() {
			Progress::DONE
		} else {
			Progress::ZERO
		}
	}

	fn as_any(&self) -> &dyn Any {
		self
	}
}

// Example resources to track completion
#[derive(Resource)]
struct AssetsLoaded;

#[derive(Resource)]
struct SceneBuilt;

#[derive(Resource)]
struct PlayerSpawned;

/// A setup key that wraps trait objects using interning for efficient comparison.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct GameSetupKey {
	type_id: TypeId,
	name: &'static str,
}

impl GameSetupKey {
	/// Create a new setup key from a provider trait object.
	/// Uses interning to ensure efficient comparison and storage.
	pub fn new<T: SetupProvider>(provider: &T) -> Self {
		Self {
			type_id: provider.as_any().type_id(),
			name: provider.name(),
		}
	}

	/// Get the human-readable name of this key
	pub fn name(&self) -> &'static str {
		self.name
	}
}

/// Global registry for provider instances, keyed by their type ID.
/// This allows us to retrieve the original provider for progress checking.
static PROVIDER_REGISTRY: OnceLock<Mutex<HashMap<TypeId, Arc<dyn SetupProvider>>>> =
	OnceLock::new();

impl GameSetupKey {
	/// Register a provider instance in the global registry
	pub fn register_provider<T: SetupProvider + 'static>(provider: T) -> Self {
		let provider = Arc::new(provider);
		let key = Self::new(provider.as_ref());

		let registry = PROVIDER_REGISTRY.get_or_init(|| Mutex::new(HashMap::new()));
		registry.lock().unwrap().insert(key.type_id, provider);

		key
	}

	/// Get the provider instance for this key
	fn get_provider(&self) -> Option<Arc<dyn SetupProvider>> {
		let registry = PROVIDER_REGISTRY.get()?;
		registry.lock().unwrap().get(&self.type_id).cloned()
	}
}

impl SetupKey for GameSetupKey {
	fn register_progress_checker(&self, world: &mut World) -> SystemId<(), Progress> {
		let provider = self
			.get_provider()
			.expect("Provider not registered - call register_provider first");

		world.register_system(move |world: &World| provider.check_progress(world))
	}

	fn relative_time_estimate(&self) -> f32 {
		self.get_provider()
			.map(|p| p.time_estimate())
			.unwrap_or(1.0)
	}
}

// Setup systems that actually perform the work
fn load_assets(mut commands: Commands) {
	println!("📦 Loading assets...");
	commands.insert_resource(AssetsLoaded);
}

fn build_scene(mut commands: Commands) {
	println!("🏗️ Building scene...");
	commands.insert_resource(SceneBuilt);
}

fn spawn_player(mut commands: Commands) {
	println!("👤 Spawning player...");
	commands.insert_resource(PlayerSpawned);
}

fn setup_complete() {
	println!("🎉 All setup complete! Game ready to start.");
}

fn main() {
	println!("🐦 Bird Barrier Trait Object Keys Example");
	println!("This example shows how to use trait objects for flexible setup providers.");
	println!();

	let mut app = App::new();
	app.add_plugins(MinimalPlugins);

	// Register providers and get their keys
	let asset_key = GameSetupKey::register_provider(AssetLoader);
	let scene_key = GameSetupKey::register_provider(SceneBuilder);
	let player_key = GameSetupKey::register_provider(PlayerSpawner);

	println!("Registered providers:");
	println!(
		"- {} (time estimate: {:.1}x)",
		asset_key.name(),
		asset_key.relative_time_estimate()
	);
	println!(
		"- {} (time estimate: {:.1}x)",
		scene_key.name(),
		scene_key.relative_time_estimate()
	);
	println!(
		"- {} (time estimate: {:.1}x)",
		player_key.name(),
		player_key.relative_time_estimate()
	);
	println!();

	// Add the setup tracking plugin
	app.add_plugins(SetupTrackingPlugin::<GameSetupKey, _, _, _, _>::new(
		|| true, // Always run condition
		setup_complete,
	));

	// Register providers with dependencies
	app.register_provider(load_assets.provides([asset_key.clone()]));

	app.register_provider(
		build_scene
			.requires([asset_key.clone()]) // Scene needs assets loaded first
			.provides([scene_key.clone()]),
	);

	app.register_provider(
		spawn_player
			.requires([scene_key.clone()]) // Player needs scene built first
			.provides([player_key.clone()]),
	);

	println!(
		"Dependencies: {} → {} → {}",
		asset_key.name(),
		scene_key.name(),
		player_key.name()
	);
	println!();

	// Run a few updates to let setup complete
	for i in 1..=5 {
		println!("--- Update {} ---", i);
		app.update();

		if app.world().contains_resource::<PlayerSpawned>() {
			break;
		}
	}

	println!();
	println!("Example complete! The trait object keys allowed for:");
	println!("- Polymorphic setup providers with different implementations");
	println!("- Type-safe interning for efficient comparison");
	println!("- Separation of interface (trait) from implementation (structs)");
	println!("- Flexible time estimates and progress checking per provider type");
}
