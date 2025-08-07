use crate::{SetupKey, SetupTracker};
use bevy_app::{App, Plugin, Startup, Update};
use bevy_ecs::{
    prelude::*,
    schedule::Condition,
    system::SystemParamFunction,
};
use bevy_log::{debug, error};
use bevy_platform::collections::HashSet;
use std::fmt::Debug;
use std::marker::PhantomData;
use std::sync::Mutex;

/// A Bevy plugin that manages setup tracking for a specific setup key type.
///
/// This plugin automatically runs setup systems based on their dependencies and tracks
/// overall progress. When all setup tasks are complete, it runs the provided completion callback.
///
/// # Type Parameters
///
/// - `K`: The setup key type that implements `SetupKey`
/// - `C`: A condition system that determines when setup should run
/// - `M`: Marker type for the condition system
/// - `Fin`: The completion callback system type
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
/// fn setup_complete() {
///     println!("Setup finished!");
/// }
///
/// App::new()
///     .add_plugins(SetupTrackingPlugin::<MySetupKey, _, _, _, _>::new(
///         || true, // Always run condition
///         setup_complete,
///     ));
/// ```
pub struct SetupTrackingPlugin<K: SetupKey, C: Condition<M>, M, Fin: SystemParamFunction<Marker, In = (), Out = ()>, Marker> {
    condition: Mutex<Option<C>>,
    on_finished: Mutex<Option<Fin>>,
    _marker: PhantomData<(K, M, Marker)>,
}

impl<K: SetupKey, C: Condition<M>, M, Fin: SystemParamFunction<Marker, In = (), Out = ()>, Marker>
    SetupTrackingPlugin<K, C, M, Fin, Marker>
{
    /// Creates a new setup tracking plugin.
    ///
    /// # Parameters
    ///
    /// - `condition`: A condition that determines when setup systems should run
    /// - `on_finished`: A system to run when all setup tasks are complete
    pub fn new(condition: C, on_finished: Fin) -> Self {
        Self {
            condition: Mutex::new(Some(condition)),
            on_finished: Mutex::new(Some(on_finished)),
            _marker: PhantomData,
        }
    }
}

impl<
    K: SetupKey + Debug,
    C: Condition<M> + Send + 'static,
    M: Send + Sync + 'static,
    Fin: SystemParamFunction<Marker, In = (), Out = ()> + Send + 'static,
    Marker: Send + Sync + 'static,
> Plugin for SetupTrackingPlugin<K, C, M, Fin, Marker>
{
    fn build(&self, app: &mut App) {
        let on_finished = self.on_finished.lock().unwrap().take().unwrap();
        let fin = app.register_system(IntoSystem::into_system(on_finished));
        app.insert_resource(SetupTracker::<K>::new(fin))
            .add_systems(Startup, validate_tracker::<K>)
            .add_systems(
                Update,
                advance_setup::<K>.run_if(self.condition.lock().unwrap().take().unwrap()),
            );
    }
}

/// System that validates the setup tracker configuration at startup.
///
/// This system checks for common configuration errors like missing providers,
/// duplicate providers, and cyclic dependencies.
pub fn validate_tracker<K: SetupKey + Debug>(world: &mut World) {
    SetupTracker::<K>::validate(world).unwrap()
}

/// System that advances the setup process by running ready providers.
///
/// This system:
/// 1. Checks which setup keys are ready (their progress checkers return finished)
/// 2. Runs provider systems whose requirements are met and provisions aren't already all finished
/// 3. Runs the completion callback if all setup is finished
pub fn advance_setup<K: SetupKey + Debug>(world: &mut World) {
    world.resource_scope::<SetupTracker<K>, _>(|world, mut tracker| {
        let mut pending = HashSet::new();
        let ready = tracker
            .entries
            .iter()
            .filter_map(|(key, checker)| {
                if world.run_system(*checker).unwrap().finished() {
                    Some(key.clone())
                } else {
                    pending.insert(key.clone());
                    None
                }
            })
            .collect::<HashSet<_>>();
        debug!(?ready, ?pending);
        
        let should_run = move |info: &crate::ProviderInfo<K>| {
            for provision in info.provides() {
                if ready.contains(provision) {
                    return false;
                }
            }
            for requirement in info.requires() {
                if !ready.contains(requirement) {
                    return false;
                }
            }
            true
        };
        
        for (system, info) in tracker.providers.iter() {
            if should_run(info) {
                if let Err(e) = world.run_system(*system) {
                    error!("Failed to run setup system: {e}");
                }
            }
        }
        
        let progress = tracker.progress(world);
        debug!(?progress);
        if progress.finished() {
            world.run_system(tracker.on_finished).unwrap();
        }
        if tracker.last_progress != progress {
            tracker.last_progress = progress;
        }
    });
}
