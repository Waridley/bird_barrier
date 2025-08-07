//! # Basic Bird Barrier Usage
//!
//! This example demonstrates the fundamental usage of Bird Barrier with simple enum-based keys.
//! This shows how to set up the plugin, define setup keys, and register providers with dependencies.

use bevy::prelude::*;
use bevy::ecs::system::SystemId;
use bird_barrier::*;

/// Simple enum-based setup keys
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
enum GameSetup {
    LoadAssets,
    BuildScene,
    SpawnPlayer,
}

impl SetupKey for GameSetup {
    fn register_progress_checker(&self, world: &mut World) -> SystemId<(), Progress> {
        match self {
            GameSetup::LoadAssets => world.register_system(|world: &World| {
                if world.contains_resource::<AssetsLoaded>() {
                    Progress::DONE
                } else {
                    Progress::ZERO
                }
            }),
            GameSetup::BuildScene => world.register_system(|world: &World| {
                if world.contains_resource::<SceneBuilt>() {
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
        }
    }
    
    fn relative_time_estimate(&self) -> f32 {
        match self {
            GameSetup::LoadAssets => 2.0,  // Takes longer
            GameSetup::BuildScene => 1.0,  // Average time
            GameSetup::SpawnPlayer => 0.5, // Quick
        }
    }
}

// Resources to track completion
#[derive(Resource)]
struct AssetsLoaded;

#[derive(Resource)]
struct SceneBuilt;

#[derive(Resource)]
struct PlayerSpawned;

// Setup systems
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
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    
    // Add the setup tracking plugin
    app.add_plugins(SetupTrackingPlugin::<GameSetup, _, _, _, _>::new(
        || true, // Always run condition
        setup_complete,
    ));
    
    // Register providers with dependencies
    app.register_provider(
        load_assets.provides([GameSetup::LoadAssets])
    );
    
    app.register_provider(
        build_scene
            .requires([GameSetup::LoadAssets])  // Scene needs assets loaded first
            .provides([GameSetup::BuildScene])
    );
    
    app.register_provider(
        spawn_player
            .requires([GameSetup::BuildScene])  // Player needs scene built first
            .provides([GameSetup::SpawnPlayer])
    );
    
    println!("🐦 Starting Bird Barrier example...");
    println!("Dependencies: Assets → Scene → Player");
    
    // Run a few updates to let setup complete
    for i in 1..=5 {
        println!("\n--- Update {} ---", i);
        app.update();
        
        if app.world().contains_resource::<PlayerSpawned>() {
            break;
        }
    }
}
