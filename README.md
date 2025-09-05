# Bird Barrier

A Bevy plugin for coordinating setup/loading tasks with dependency management.

Like a synchronization barrier in concurrent programming, Bird Barrier ensures 
all your setup tasks complete before continuing.

## Features

- **Dependency Management**: Define a graph of setup tasks with requirements and provisions
- **Progress Tracking**: Monitor the progress of individual tasks and overall setup
- **Automatic Scheduling**: Tasks run automatically when their dependencies are satisfied
- **Validation**: Detect missing providers, duplicate providers, and cyclic dependencies
- **Separation of Concerns**: Define all conditions for moving to the next state separately from the systems that satisfy those conditions
- **Interactive Visualization**: Optional graph visualization with egui for debugging and understanding dependencies

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
bird_barrier = "0.1"
```

## Quick Start

```rust
use bevy::prelude::*;
use bevy::ecs::system::SystemId;
use bird_barrier::*;

// Define your setup keys
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum MySetupKey {
    LoadAssets,
    BuildScene,
    InitializeGame,
}

impl SetupKey for MySetupKey {
    fn register_progress_checker(&self, world: &mut World) -> SystemId<(), Progress> {
        match self {
            MySetupKey::LoadAssets => world.register_system(check_assets_loaded),
            MySetupKey::BuildScene => world.register_system(check_scene_built),
            MySetupKey::InitializeGame => world.register_system(check_game_initialized),
        }
    }
}

fn check_assets_loaded() -> Progress {
    // Your progress checking logic here
    Progress::DONE
}

fn check_scene_built() -> Progress {
    Progress::DONE
}

fn check_game_initialized() -> Progress {
    Progress::DONE
}

fn load_assets() {
    println!("Loading assets...");
}

fn build_scene() {
    println!("Building scene...");
}

fn initialize_game() {
    println!("Initializing game...");
}

fn setup_complete() {
    println!("Setup complete!");
}

fn main() {
    App::new()
        .add_plugins(SetupTrackingPlugin::<MySetupKey, _, _, _>::new(
            // Condition: always run (you might want to use a state condition)
            || true,
            // On finished callback
            setup_complete,
        ))
        // Register your setup providers
        .register_provider(
            load_assets.provides([MySetupKey::LoadAssets])
        )
        .register_provider(
            build_scene
                .requires([MySetupKey::LoadAssets])
                .provides([MySetupKey::BuildScene])
        )
        .register_provider(
            initialize_game
                .requires([MySetupKey::BuildScene])
                .provides([MySetupKey::InitializeGame])
        )
        .run();
}
```

## Core Concepts

### Setup Keys

Setup keys represent different stages or components of your application's initialization. They must implement the `SetupKey` trait, which requires:

- A progress checker system that returns the current progress (0.0 to 1.0)
- Optionally, a relative time estimate for weighted progress calculation

### Providers

Providers are systems that contribute to the setup process. This concept is similar to:

- **Linux package dependencies**: Where multiple packages can "provide" the same capability (e.g., different web servers can all provide `httpd`)
- **C API headers**: Where an interface is defined in a header file but can be implemented by different source files
- **Abstract classes**: Where a class defines an interface but can be implemented by different concrete classes

Each provider can:

- **Require** certain setup keys to be complete before running
- **Provide** certain setup keys when it completes
- Have a custom name for debugging

This separation allows you to define what your setup steps need without tightly coupling them to specific implementations.

### Progress Tracking

The system automatically tracks progress by:

1. Running progress checkers for each setup key
2. Calculating weighted overall progress
3. Running providers whose dependencies are satisfied
4. Calling the completion callback when all setup is done

## Examples

Bird Barrier includes examples demonstrating different usage patterns:

- **[`basic_usage.rs`](examples/basic_usage.rs)** - Simple enum-based keys (recommended starting point)
- **[`trait_object_keys.rs`](examples/trait_object_keys.rs)** - Advanced trait object-based keys for polymorphic setup providers
- **[`visualization.rs`](examples/visualization.rs)** - Interactive graph visualization (requires `visualization` feature)

## Helper Functions

The crate provides several helper functions for common progress checking patterns:

- `single_spawn_progress<F>()`: Check if an entity with filter `F` exists
- `resource_progress<R>()`: Check if resource `R` exists
- `state_progress<S>(state)`: Check if the app is in a specific state
- `assets_progress<C>()`: Check asset loading progress for collection `C`

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## LLM Usage disclaimer

The original implementation was entirely written by me for use in 
[Mt. Thyrios](https://github.com/Waridley/mt-thyrios). I then used the Augment
Code plugin in CLion to help extract it out into a separate crate, add 
documentation, examples, and tests. I consider this a reasonable usage of LLM 
technology, but if it is unacceptable to you, I sincerely apologize and I
promise I do understand your concerns. We're all trying to navigate this new
paradigm in our own ways.
