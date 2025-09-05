//! # Graph Visualization Example
//!
//! This example demonstrates the graph visualization feature of Bird Barrier.
//! It shows how to set up the visualization plugin and interact with the dependency graph.
//!
//! ## Controls
//!
//! - Press 'G' to toggle the graph visualization window
//! - Press 'H' to toggle a custom graph panel
//! - Use mouse to pan and zoom in the graph view
//! - Nodes represent setup providers
//! - Colored pins represent different setup keys
//! - Lines show dependency relationships

use bevy::ecs::system::SystemId;
use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPlugin, EguiPrimaryContextPass};
use bird_barrier::*;

/// Setup keys for this example
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
enum GameSetup {
	LoadConfig,
	LoadAssets,
	InitializeAudio,
	BuildMainMenu,
	BuildGameWorld,
	SpawnPlayer,
	StartGame,
}

impl SetupKey for GameSetup {
	fn register_progress_checker(&self, world: &mut World) -> SystemId<(), Progress> {
		match self {
			GameSetup::LoadConfig => world.register_system(|world: &World| {
				if world.contains_resource::<ConfigLoaded>() {
					Progress::DONE
				} else {
					Progress::ZERO
				}
			}),
			GameSetup::LoadAssets => world.register_system(|world: &World| {
				if world.contains_resource::<AssetsLoaded>() {
					Progress::DONE
				} else {
					Progress::ZERO
				}
			}),
			GameSetup::InitializeAudio => world.register_system(|world: &World| {
				if world.contains_resource::<AudioInitialized>() {
					Progress::DONE
				} else {
					Progress::ZERO
				}
			}),
			GameSetup::BuildMainMenu => world.register_system(|world: &World| {
				if world.contains_resource::<MainMenuBuilt>() {
					Progress::DONE
				} else {
					Progress::ZERO
				}
			}),
			GameSetup::BuildGameWorld => world.register_system(|world: &World| {
				if world.contains_resource::<GameWorldBuilt>() {
					Progress::DONE
				} else {
					Progress::ZERO
				}
			}),
			GameSetup::SpawnPlayer => world.register_system(|world: &World| {
				if world.contains_resource::<PlayerSpawned>() {
					Progress::DONE
				} else {
					Progress::ZERO
				}
			}),
			GameSetup::StartGame => world.register_system(|world: &World| {
				if world.contains_resource::<GameStarted>() {
					Progress::DONE
				} else {
					Progress::ZERO
				}
			}),
		}
	}

	fn relative_time_estimate(&self) -> f32 {
		match self {
			GameSetup::LoadConfig => 0.5,
			GameSetup::LoadAssets => 3.0, // Takes longer
			GameSetup::InitializeAudio => 1.0,
			GameSetup::BuildMainMenu => 1.5,
			GameSetup::BuildGameWorld => 2.0,
			GameSetup::SpawnPlayer => 0.5,
			GameSetup::StartGame => 0.5,
		}
	}
}

// Resources to track completion
#[derive(Resource)]
struct ConfigLoaded;

#[derive(Resource)]
struct AssetsLoaded;

#[derive(Resource)]
struct AudioInitialized;

#[derive(Resource)]
struct MainMenuBuilt;

#[derive(Resource)]
struct GameWorldBuilt;

#[derive(Resource)]
struct PlayerSpawned;

#[derive(Resource)]
struct GameStarted;

// Setup systems
fn load_config(mut commands: Commands) {
	println!("⚙️ Loading configuration...");
	commands.insert_resource(ConfigLoaded);
}

fn load_assets(mut commands: Commands) {
	println!("📦 Loading assets...");
	commands.insert_resource(AssetsLoaded);
}

fn initialize_audio(mut commands: Commands) {
	println!("🔊 Initializing audio...");
	commands.insert_resource(AudioInitialized);
}

fn build_main_menu(mut commands: Commands) {
	println!("🏠 Building main menu...");
	commands.insert_resource(MainMenuBuilt);
}

fn build_game_world(mut commands: Commands) {
	println!("🌍 Building game world...");
	commands.insert_resource(GameWorldBuilt);
}

fn spawn_player(mut commands: Commands) {
	println!("👤 Spawning player...");
	commands.insert_resource(PlayerSpawned);
}

fn start_game(mut commands: Commands) {
	println!("🎮 Starting game...");
	commands.insert_resource(GameStarted);
}

fn setup_complete(mut commands: Commands, logged: Option<Res<SetupCompleteLogged>>) {
	// Only print once
	if logged.is_none() {
		println!("🎉 All setup complete! Game is ready!");
		// Insert a resource to track that we've already logged completion
		commands.insert_resource(SetupCompleteLogged);
	}
}

#[derive(Resource)]
struct SetupCompleteLogged;

// Setup a basic camera for rendering
fn setup_camera(mut commands: Commands) {
	commands.spawn(Camera2d);
}

// System to toggle the dedicated graph window with 'G' key
fn toggle_graph_window(
	mut commands: Commands,
	keys: Res<ButtonInput<KeyCode>>,
) {
	if keys.just_pressed(KeyCode::KeyG) {
		commands.run_system_cached(toggle_setup_graph_window::<GameSetup>);
	}
}

// System to show a custom graph panel with 'H' key
fn custom_graph_panel(
	graph: Res<SetupTracker<GameSetup>>,
	mut contexts: EguiContexts,
	mut state: Option<ResMut<SetupGraphVisState<GameSetup>>>,
) {
	if let Ok(ctx) = contexts.ctx_mut() {
		if let Some(state) = &mut state {
			bevy_egui::egui::TopBottomPanel::bottom("custom_graph_panel")
				.min_height(500.0)
				.resizable(true)
				.show(ctx, |ui| {
					ui.heading("Custom Setup Graph Window");
					ui.separator();
					draw_setup_graph(ui, &*graph, state);
				});
		} else {
			bevy_egui::egui::TopBottomPanel::bottom("custom_graph_panel")
				.min_height(500.0)
				.resizable(true)
				.show(ctx, |ui| {
					ui.heading("Custom Setup Graph Window");
					ui.separator();
					ui.label("Graph window closed. Press G to open.");
				});
		}
	} else {
		error!("No egui context")
	}
}

fn main() {
	println!("🐦 Bird Barrier Visualization Example");
	println!("Use mouse to pan and zoom in the graph view");
	println!();

	let mut app = App::new();
	app.add_plugins((DefaultPlugins, EguiPlugin::default()));

	// Add a camera for rendering
	app.add_systems(Startup, setup_camera);

	// Add the setup tracking plugin
	app.add_plugins(SetupTrackingPlugin::<GameSetup, _, _, _, _>::new(
		|| true, // Always run condition
		setup_complete,
	));

	// Add the visualization plugin
	app.add_plugins(SetupGraphVisualizationPlugin::<GameSetup>::default());
	// Opens the graph window by default
	app.init_resource::<SetupGraphVisState<GameSetup>>();

	// Add our custom systems for controlling the visualization
	app.add_systems(Update, toggle_graph_window);
	app.add_systems(EguiPrimaryContextPass, custom_graph_panel);

	// Register providers with complex dependencies
	app.register_provider(load_config.provides([GameSetup::LoadConfig]));

	app.register_provider(
		load_assets
			.requires([GameSetup::LoadConfig]) // Assets need config first
			.provides([GameSetup::LoadAssets]),
	);

	app.register_provider(
		initialize_audio
			.requires([GameSetup::LoadConfig]) // Audio needs config first
			.provides([GameSetup::InitializeAudio]),
	);

	app.register_provider(
		build_main_menu
			.requires([GameSetup::LoadAssets, GameSetup::InitializeAudio]) // Menu needs both assets and audio
			.provides([GameSetup::BuildMainMenu]),
	);

	app.register_provider(
		build_game_world
			.requires([GameSetup::LoadAssets]) // World needs assets
			.provides([GameSetup::BuildGameWorld]),
	);

	app.register_provider(
		spawn_player
			.requires([GameSetup::BuildGameWorld]) // Player needs world
			.provides([GameSetup::SpawnPlayer]),
	);

	app.register_provider(
		start_game
			.requires([GameSetup::BuildMainMenu, GameSetup::SpawnPlayer]) // Game needs both menu and player
			.provides([GameSetup::StartGame]),
	);

	app.run();
}
