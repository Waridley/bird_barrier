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
//!   single enum

use bevy::ecs::system::SystemId;
use bevy::prelude::*;
use bevy_ecs::define_label;
use bevy_ecs::intern::Interned;
use bird_barrier::*;
use std::hash::Hash;

define_label!(
	GameSetupLabel,
	GAME_SETUP_LABEL_INTERNER,
	extra_methods: {
		fn register_progress_checker(&self, world: &mut World) -> SystemId<(), Progress>;
		fn relative_time_estimate(&self) -> f32 { 1.0 }
		fn key(self) -> GameSetupKey where Self: Sized {
			GameSetupKey(self.intern())
		}
	},
	extra_methods_impl: {
		fn register_progress_checker(&self, world: &mut World) -> SystemId<(), Progress> {
			self.0.register_progress_checker(world)
		}
		fn relative_time_estimate(&self) -> f32 { self.0.relative_time_estimate() }
		fn key(self) -> GameSetupKey {
			GameSetupKey(self)
		}
	}
);

#[macro_export]
macro_rules! new_game_setup_label {
	($T:ident, $progress:expr) => {
		#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, Hash)]
		pub struct $T;

		impl $crate::GameSetupLabel for $T {
			fn register_progress_checker(
				&self,
				world: &mut World,
			) -> ::bevy::ecs::system::SystemId<(), ::bird_barrier::Progress> {
				world.register_system($progress)
			}

			fn dyn_clone(&self) -> Box<dyn $crate::GameSetupLabel> {
				Box::new(self.clone())
			}

			fn as_dyn_eq(&self) -> &dyn ::bevy::ecs::label::DynEq {
				self
			}

			fn dyn_hash(&self, mut state: &mut dyn std::hash::Hasher) {
				let ty_id = ::std::any::TypeId::of::<Self>();
				::std::hash::Hash::hash(&ty_id, &mut state);
				::std::hash::Hash::hash(self, &mut state);
			}
		}
	};
}

fn dummy_time_progress(dur_secs: f32) -> impl System<In = (), Out = Progress> {
	IntoSystem::into_system(move |t: Res<Time>| Progress::new(t.elapsed_secs() / dur_secs))
}

// Example resources to track completion
new_game_setup_label!(AssetsLoaded, dummy_time_progress(2.0));
new_game_setup_label!(SceneBuilt, dummy_time_progress(3.0));
new_game_setup_label!(PlayerSpawned, dummy_time_progress(3.5));

/// A setup key that wraps trait objects using interning for efficient comparison.
///
// Newtype to circumvent orphan rule
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub struct GameSetupKey(pub Interned<dyn GameSetupLabel>);

impl SetupKey for GameSetupKey {
	fn register_progress_checker(&self, world: &mut World) -> SystemId<(), Progress> {
		self.0.register_progress_checker(world)
	}

	fn relative_time_estimate(&self) -> f32 {
		self.0.relative_time_estimate()
	}
}

// Setup systems that actually perform the work
fn load_assets() {
	println!("📦 Loading assets...");
}

fn build_scene() {
	println!("🏗️ Building scene...");
}

fn spawn_player() {
	println!("👤 Spawning player...");
}

fn setup_complete(mut events: EventWriter<AppExit>) {
	println!("🎉 All setup complete! Exiting example.");
	events.write(AppExit::Success);
}

fn main() -> AppExit {
	println!("🐦 Bird Barrier Trait Object Keys Example");
	println!("This example shows how to use trait objects for flexible setup providers.\n");

	let mut app = App::new();
	app.add_plugins(MinimalPlugins);

	// Register providers and get their keys
	let asset_loaded = AssetsLoaded.key();
	let scene_built = SceneBuilt.key();
	let player_spawned = PlayerSpawned.key();

	println!("Registered providers:");
	println!(
		"- {asset_loaded:?} (time estimate: {:.1}x)",
		asset_loaded.relative_time_estimate()
	);
	println!(
		"- {scene_built:?} (time estimate: {:.1}x)",
		scene_built.relative_time_estimate()
	);
	println!(
		"- {player_spawned:?} (time estimate: {:.1}x)",
		player_spawned.relative_time_estimate()
	);
	println!();

	// Add the setup tracking plugin
	app.add_plugins(SetupTrackingPlugin::<GameSetupKey, _, _, _, _>::new(
		|| true, // Always run condition
		setup_complete,
	));

	// Register providers with dependencies
	app.register_provider(load_assets.provides([asset_loaded]));

	app.register_provider(
		build_scene
			.requires([asset_loaded]) // Scene needs assets loaded first
			.provides([scene_built]),
	);

	app.register_provider(
		spawn_player
			.requires([scene_built]) // Player needs scene built first
			.provides([player_spawned]),
	);

	// Slow down tick rate just to avoid spamming stdout
	app.add_systems(Update, |tracker: Res<SetupTracker<GameSetupKey>>| {
		println!("{:.2}", tracker.last_progress());
		std::thread::sleep(std::time::Duration::from_secs_f32(0.05))
	});

	println!("Dependencies: {asset_loaded:?} → {scene_built:?} → {player_spawned:?}");
	println!();

	app.run()
}
