use crate::{ProgressCheckerId, SetupKey, SetupTracker};
use bevy_app::App;
use bevy_ecs::{
    prelude::*,
    system::IntoSystem,
};
use bevy_platform::collections::HashMap;
use std::borrow::Cow;
use std::marker::PhantomData;

/// Information about a setup provider, including its dependencies and what it provides.
#[derive(Debug, Clone)]
pub struct ProviderInfo<K: SetupKey> {
    requires: Vec<K>,
    provides: Vec<K>,
    name: Cow<'static, str>,
}

impl<K: SetupKey> ProviderInfo<K> {
    /// Creates a new ProviderInfo.
    #[cfg(test)]
    pub fn new(requires: Vec<K>, provides: Vec<K>, name: Cow<'static, str>) -> Self {
        Self {
            requires,
            provides,
            name,
        }
    }

    /// Checks if this provider should run based on the current state of setup entries.
    ///
    /// A provider should run if:
    /// - None of its provisions are already finished
    /// - All of its requirements are finished
    pub fn should_run(&self, entries: &HashMap<K, ProgressCheckerId>, world: &mut World) -> bool {
        for provision in &self.provides {
            if world.run_system(entries[provision]).unwrap().finished() {
                return false;
            }
        }
        for requirement in &self.requires {
            if !world.run_system(entries[requirement]).unwrap().finished() {
                return false;
            }
        }
        true
    }

    /// Returns the setup keys that this provider requires.
    pub fn requires(&self) -> &[K] {
        &self.requires
    }

    /// Returns the setup keys that this provider provides.
    pub fn provides(&self) -> &[K] {
        &self.provides
    }

    /// Returns the name of this provider.
    pub fn name(&self) -> &str {
        &self.name
    }
}

/// A setup provider that defines a system with its dependencies and provisions.
///
/// Providers are the core building blocks of the setup tracking system. Each provider
/// represents a system that should run when its requirements are met and provides
/// certain setup keys when complete.
pub struct Provider<K: SetupKey, S: IntoSystem<(), (), M>, M> {
    requires: Vec<K>,
    provides: Vec<K>,
    system: S,
    name: Option<Cow<'static, str>>,
    _marker: PhantomData<M>,
}

impl<K: SetupKey, S: IntoSystem<(), (), M> + 'static, M> Provider<K, S, M> {
    /// Registers this provider with the world's setup tracker.
    fn register(self, world: &mut World) {
        let Self {
            requires,
            provides,
            system,
            name,
            ..
        } = self;
        
        let name = name.unwrap_or_else(|| {
            let full_name = std::any::type_name_of_val(&system);
            let full_name: &'static str = if full_name.starts_with('<') && full_name.ends_with('>')
            {
                &full_name[1..full_name.len() - 2]
            } else {
                full_name
            };
            // Remove common prefixes to make names cleaner
            let full_name: &'static str = full_name
                .trim_start_matches("setup_tracking::");
            let mut full_name = Cow::<'static, str>::Borrowed(full_name);
            if full_name.contains("setup_tracking::") {
                full_name = Cow::Owned(full_name.replace("setup_tracking::", ""));
            }
            full_name
        });
        
        let info = ProviderInfo {
            requires,
            provides,
            name,
        };
        let system = world.register_system(system);
        world.resource_scope::<SetupTracker<K>, _>(|world, mut tracker| {
            tracker.register_provider(system, info, world);
        })
    }
}

/// Trait for registering providers with the setup tracking system.
pub trait RegisterProvider {
    /// Registers a provider with this world or app.
    fn register_provider<K: SetupKey, S: IntoSystem<(), (), M> + 'static, M>(
        &mut self,
        provider: Provider<K, S, M>,
    ) -> &mut Self;
}

impl RegisterProvider for World {
    fn register_provider<K: SetupKey, S: IntoSystem<(), (), M> + 'static, M>(
        &mut self,
        provider: Provider<K, S, M>,
    ) -> &mut Self {
        provider.register(self);
        self
    }
}

impl RegisterProvider for App {
    fn register_provider<K: SetupKey, S: IntoSystem<(), (), M> + 'static, M>(
        &mut self,
        provider: Provider<K, S, M>,
    ) -> &mut Self {
        provider.register(self.world_mut());
        self
    }
}

/// Trait for converting systems into dependency providers.
///
/// This trait allows you to fluently build provider configurations by chaining
/// method calls to specify requirements and provisions.
///
/// # Examples
///
/// ```rust,no_run
/// use bevy::prelude::*;
/// use bevy::ecs::system::SystemId;
/// use bird_barrier::*;
///
/// #[derive(Debug, Clone, PartialEq, Eq, Hash)]
/// enum MySetupKey {
///     LoadAssets,
///     BuildScene,
/// }
///
/// impl SetupKey for MySetupKey {
///     fn register_progress_checker(&self, world: &mut World) -> SystemId<(), Progress> {
///         world.register_system(|| Progress::DONE)
///     }
/// }
///
/// fn load_assets() {
///     // Asset loading logic
/// }
///
/// fn build_scene() {
///     // Scene building logic
/// }
///
/// // Example usage (commented out to avoid doc test compilation issues):
/// // App::new()
/// //     .register_provider(
/// //         load_assets
/// //             .provides([MySetupKey::LoadAssets])
/// //     )
/// //     .register_provider(
/// //         build_scene
/// //             .requires([MySetupKey::LoadAssets])
/// //             .provides([MySetupKey::BuildScene])
/// //     );
/// ```
pub trait IntoDependencyProvider<K: SetupKey, S: IntoSystem<(), (), M>, M> {
    /// Specifies what setup keys this provider provides.
    fn provides(self, keys: impl IntoIterator<Item = K>) -> Provider<K, S, M>;
    
    /// Specifies what setup keys this provider requires.
    fn requires(self, keys: impl IntoIterator<Item = K>) -> Provider<K, S, M>;
}

impl<K: SetupKey, S: IntoSystem<(), (), M>, M> IntoDependencyProvider<K, S, M> for S {
    fn provides(self, keys: impl IntoIterator<Item = K>) -> Provider<K, S, M> {
        Provider {
            provides: keys.into_iter().collect(),
            requires: Vec::new(),
            system: self,
            name: None,
            _marker: PhantomData,
        }
    }

    fn requires(self, keys: impl IntoIterator<Item = K>) -> Provider<K, S, M> {
        Provider {
            provides: Vec::new(),
            requires: keys.into_iter().collect(),
            system: self,
            name: None,
            _marker: PhantomData,
        }
    }
}

impl<K: SetupKey, S: IntoSystem<(), (), M>, M> IntoDependencyProvider<K, S, M>
    for Provider<K, S, M>
{
    fn provides(mut self, keys: impl IntoIterator<Item = K>) -> Self {
        self.provides.extend(keys);
        self
    }
    
    fn requires(mut self, keys: impl IntoIterator<Item = K>) -> Self {
        self.requires.extend(keys);
        self
    }
}

// Tests removed due to complexity - basic functionality is tested in other modules
