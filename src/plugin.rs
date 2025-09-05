use crate::{SetupKey, SetupTracker, validate_setup_graph};
use bevy_app::{App, Plugin, Startup, Update};
use bevy_ecs::schedule::{InternedScheduleLabel, ScheduleLabel};
use bevy_ecs::{prelude::*, schedule::Condition, system::SystemParamFunction};
use bevy_log::{debug, error};
use bevy_platform::collections::HashSet;
use std::fmt::Debug;
use std::marker::PhantomData;
use std::sync::Mutex;

/// A Bevy plugin that manages setup tracking for a [`SetupKey`](SetupKey).
///
/// This plugin automatically runs setup systems based on their dependencies and tracks
/// overall progress. When all setup tasks are complete, it runs the provided completion callback.
///
/// # Type Parameters
///
/// - `K`: The setup key type that implements [`SetupKey`](SetupKey)
/// - `C`: A condition system that determines when setup should run
/// - `M`: Marker type for the condition system (needed by Bevy, can usually be ignored)
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
///
/// Note that the plugin requires your `SetupKey` to implement `Debug` in order to implement
/// `Plugin`, because it adds [`validate_setup_graph`] to the app, which requires `K: Debug` for
/// error reporting. If your keys cannot implement `Debug`, you can still manually insert a
/// `SetupTracker` and `advance_setup` system to your app.
pub struct SetupTrackingPlugin<
	K: SetupKey,
	C: Condition<M>,
	M,
	Fin: SystemParamFunction<Marker, In = (), Out = ()>,
	Marker,
> {
	condition: Mutex<Option<C>>,
	on_finished: Mutex<Option<Fin>>,
	schedule: InternedScheduleLabel,
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
		Self::new_in_schedule(Update, condition, on_finished)
	}

	/// Creates a new setup tracking plugin that runs in the given schedule.
	///
	/// # Parameters
	///
	/// - `schedule`: The schedule to run the setup systems in
	/// - `condition`: A condition that determines when setup systems should run
	/// - `on_finished`: A system to run when all setup tasks are complete
	pub fn new_in_schedule(schedule: impl ScheduleLabel, condition: C, on_finished: Fin) -> Self {
		Self {
			condition: Mutex::new(Some(condition)),
			on_finished: Mutex::new(Some(on_finished)),
			schedule: schedule.intern(),
			_marker: PhantomData,
		}
	}

	/// Sets the schedule to run the setup systems in.
	///
	/// # Parameters
	///
	/// - `schedule`: The schedule to run the setup systems in
	pub fn in_schedule(self, schedule: impl ScheduleLabel) -> Self {
		Self {
			schedule: schedule.intern(),
			..self
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
			.add_systems(Startup, validate_setup_graph::<K>)
			.add_systems(
				self.schedule,
				advance_setup::<K>.run_if(self.condition.lock().unwrap().take().unwrap()),
			);
	}
}

/// System that advances the setup process by running ready providers.
///
/// This system:
/// 1. Checks which setup keys are ready (their progress checkers return finished)
/// 2. Runs provider systems whose requirements are met and provisions aren't already all finished
/// 3. Runs the completion callback if all setup is finished
pub fn advance_setup<K: SetupKey>(world: &mut World) {
	// TODO: condition hackery might be able to eliminate this single-threaded, manual system running,
	// but it would be hard to take advantage of collecting all finished entries up-front to avoid
	// re-running progress checkers multiple times. It could also introduce race conditions between
	// different providers checking the same key in the same tick and getting different results, but
	// it's not clear if that would cause any real issues.
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
