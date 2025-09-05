use crate::{Progress, ProgressCheckerId, ProviderInfo, SetupKey};
use bevy_ecs::{prelude::*, system::SystemId};
use bevy_log::error;
use bevy_platform::collections::{HashMap, HashSet};
use std::fmt::{Debug, Formatter};

/// The main resource that tracks setup progress and manages provider systems.
///
/// This resource maintains the state of all setup entries, their progress checkers,
/// and the provider systems that contribute to setup completion.
// TODO: A Schedule would ideally be better than manually running systems
#[derive(Resource, Debug)]
pub struct SetupTracker<K: SetupKey> {
	pub(crate) entries: HashMap<K, ProgressCheckerId>,
	pub(crate) providers: HashMap<SystemId, ProviderInfo<K>>,
	pub(crate) on_finished: SystemId,
	pub(crate) last_progress: Progress,
}

impl<K: SetupKey> SetupTracker<K> {
	/// Creates a new setup tracker with the given completion callback system.
	pub fn new(on_finished: SystemId) -> Self {
		Self {
			entries: Default::default(),
			providers: Default::default(),
			on_finished,
			last_progress: Default::default(),
		}
	}

	/// Registers a provider system with its dependency information.
	///
	/// This method automatically registers progress checkers for any setup keys
	/// that haven't been seen before.
	pub fn register_provider(
		&mut self,
		system: SystemId,
		provider: ProviderInfo<K>,
		world: &mut World,
	) {
		for req in provider.requires() {
			if !self.entries.contains_key(req) {
				self.entries
					.insert(req.clone(), req.register_progress_checker(world));
			}
		}
		for prov in provider.provides() {
			if !self.entries.contains_key(prov) {
				self.entries
					.insert(prov.clone(), prov.register_progress_checker(world));
			}
		}
		self.providers.insert(system, provider);
	}

	/// Validates the setup graph for common configuration errors.
	///
	/// This method checks for:
	/// - Unprovided setup keys (keys that are required but never provided)
	/// - Duplicate providers (multiple providers for the same key)
	/// - Cyclic dependencies (circular dependency chains)
	///
	/// This can only be used with keys that implement `Debug`, because [`InvalidSetupGraph`]
	/// requires `K: Debug` for its `Display` implementation.
	pub fn validate(world: &mut World) -> Result<(), InvalidSetupGraph<K>>
	where
		K: Debug,
	{
		world.resource_scope::<SetupTracker<K>, _>(|_, tracker| {
			let mut unprovided = tracker.entries.keys().cloned().collect::<HashSet<_>>();
			let mut providers = HashMap::<K, Vec<SystemId>>::new();

			for (system, info) in tracker.providers.iter() {
				for provision in info.provides() {
					providers
						.entry(provision.clone())
						.or_insert_with(Vec::new)
						.push(*system);
					unprovided.remove(provision);
				}
			}

			providers.retain(|_, providers| providers.len() > 1);

			let cyclic_dependencies = Self::detect_cycles(&tracker);

			if !unprovided.is_empty() || !providers.is_empty() || !cyclic_dependencies.is_empty() {
				Err(InvalidSetupGraph {
					unprovided,
					duplicate_providers: providers,
					cyclic_dependencies,
				})
			} else {
				Ok(())
			}
		})
	}

	/// Calculates the overall progress of the setup process.
	///
	/// Progress is calculated as a weighted average based on each setup key's
	/// relative time estimate and current progress.
	pub fn progress(&self, world: &mut World) -> Progress {
		let total: f32 = self.entries.keys().map(K::relative_time_estimate).sum();
		let sum: f32 = self
			.entries
			.iter()
			.map(|(key, checker)| {
				*world.run_system(*checker).unwrap() * key.relative_time_estimate()
			})
			.sum();
		Progress::new(sum / total)
	}

	/// Returns the last calculated progress value.
	pub fn last_progress(&self) -> Progress {
		self.last_progress
	}

	/// Returns a reference to the setup entries map.
	pub fn entries(&self) -> &HashMap<K, ProgressCheckerId> {
		&self.entries
	}

	/// Returns a reference to the providers map.
	pub fn providers(&self) -> &HashMap<SystemId, ProviderInfo<K>> {
		&self.providers
	}

	/// Returns an iterator over all providers that provide the given key.
	pub fn providers_of<'a, 'b>(
		&'a self,
		key: &'b K,
	) -> impl Iterator<Item = (SystemId, usize)> + use<'a, 'b, K> {
		self.providers.iter().filter_map(|(id, info)| {
			info.provides()
				.iter()
				.enumerate()
				.find(|(_, item)| **item == *key)
				.map(|(i, _)| (*id, i))
		})
	}

	/// Returns an iterator over all providers that depend on the given key.
	pub fn dependants_of<'a, 'b>(
		&'a self,
		key: &'b K,
	) -> impl Iterator<Item = (SystemId, usize)> + use<'a, 'b, K> {
		self.providers.iter().filter_map(|(id, info)| {
			info.requires()
				.iter()
				.enumerate()
				.find(|(_, item)| **item == *key)
				.map(|(i, _)| (*id, i))
		})
	}

	/// Detects cycles in the dependency graph using depth-first search.
	///
	/// Returns a set of setup keys that are part of dependency cycles.
	fn detect_cycles(tracker: &SetupTracker<K>) -> HashSet<K> {
		let mut visited = HashSet::new();
		let mut rec_stack = HashSet::new();
		let mut cycles = HashSet::new();

		// Build a dependency graph: key -> keys it depends on
		let mut dependencies = HashMap::<K, Vec<K>>::new();

		// Initialize all keys
		for key in tracker.entries.keys() {
			dependencies.entry(key.clone()).or_default();
		}

		// Populate dependencies from provider requirements
		for (_, info) in tracker.providers.iter() {
			for provided in info.provides() {
				for required in info.requires() {
					dependencies
						.entry(provided.clone())
						.or_default()
						.push(required.clone());
				}
			}
		}

		// Perform DFS for each unvisited node
		for key in tracker.entries.keys() {
			if !visited.contains(key) {
				Self::dfs_cycle_detection(
					key,
					&dependencies,
					&mut visited,
					&mut rec_stack,
					&mut cycles,
				);
			}
		}

		cycles
	}

	/// Depth-first search helper for cycle detection.
	fn dfs_cycle_detection(
		key: &K,
		dependencies: &HashMap<K, Vec<K>>,
		visited: &mut HashSet<K>,
		rec_stack: &mut HashSet<K>,
		cycles: &mut HashSet<K>,
	) {
		visited.insert(key.clone());
		rec_stack.insert(key.clone());

		if let Some(deps) = dependencies.get(key) {
			for dep in deps {
				if !visited.contains(dep) {
					Self::dfs_cycle_detection(dep, dependencies, visited, rec_stack, cycles);
				} else if rec_stack.contains(dep) {
					// Found a cycle - mark all nodes in the current recursion stack as cyclic
					// This includes all nodes from the current path back to the cycle start
					for node in rec_stack.iter() {
						cycles.insert(node.clone());
					}
					cycles.insert(dep.clone()); // Also mark the target of the back edge
				}
			}
		}

		rec_stack.remove(key);
	}

	/// Returns the setup stages in dependency order.
	///
	/// Each stage contains provider systems that can run in parallel,
	/// with later stages depending on earlier stages.
	pub fn stages(&self) -> Vec<Vec<SystemId>> {
		let mut provided_so_far = HashSet::new();
		let mut stages: Vec<Vec<SystemId>> = Vec::new();
		let mut providers = self.providers.clone();

		while !providers.is_empty() {
			let mut stage = Vec::new();
			let mut provided_this_stage = Vec::new();

			providers.retain(|id, info| {
				for req in info.requires().iter() {
					if !provided_so_far.contains(req) {
						return true;
					}
				}
				stage.push(*id);
				provided_this_stage.extend_from_slice(info.provides());
				false
			});

			if stage.is_empty() {
				error!("Not all keys are provided");
				break;
			}

			provided_so_far.extend(provided_this_stage);
			stages.push(stage);
		}

		stages
	}
}

/// Error type for invalid setup graph configurations.
#[derive(Debug, Clone)]
pub struct InvalidSetupGraph<K: SetupKey> {
	pub unprovided: HashSet<K>,
	pub duplicate_providers: HashMap<K, Vec<SystemId>>,
	pub cyclic_dependencies: HashSet<K>,
}

impl<K: SetupKey + Debug> std::fmt::Display for InvalidSetupGraph<K> {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		<Self as Debug>::fmt(self, f)
	}
}

impl<K: SetupKey + Debug> std::error::Error for InvalidSetupGraph<K> {}

/// System to validate the setup graph at startup.
///
/// Wraps [`SetupTracker::validate`], but returns a [bevy::ecs::error::Result] so it can be used as
/// a Bevy system.
///
/// Also requires keys to be `Debug` for the same reason as [`SetupTracker::validate`].
pub fn validate_setup_graph<K: SetupKey + Debug>(world: &mut World) -> Result {
	SetupTracker::<K>::validate(world)?;
	Ok(())
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::{Progress, ProviderInfo};
	use std::borrow::Cow;

	#[derive(Debug, Clone, PartialEq, Eq, Hash)]
	enum TestSetupKey {
		A,
		B,
		C,
		D,
	}

	impl SetupKey for TestSetupKey {
		fn register_progress_checker(&self, world: &mut World) -> SystemId<(), Progress> {
			world.register_system(|| Progress::DONE)
		}
	}

	#[test]
	fn test_cycle_detection() {
		let mut world = World::new();
		let mut tracker = SetupTracker::<TestSetupKey>::new(world.register_system(|| {}));

		// Add entries for all keys
		tracker
			.entries
			.insert(TestSetupKey::A, world.register_system(|| Progress::DONE));
		tracker
			.entries
			.insert(TestSetupKey::B, world.register_system(|| Progress::DONE));
		tracker
			.entries
			.insert(TestSetupKey::C, world.register_system(|| Progress::DONE));
		tracker
			.entries
			.insert(TestSetupKey::D, world.register_system(|| Progress::DONE));

		// Create a cycle: A -> B -> C -> A
		let provider_a = ProviderInfo::new(
			vec![TestSetupKey::C],
			vec![TestSetupKey::A],
			Cow::Borrowed("provider_a"),
		);
		let provider_b = ProviderInfo::new(
			vec![TestSetupKey::A],
			vec![TestSetupKey::B],
			Cow::Borrowed("provider_b"),
		);
		let provider_c = ProviderInfo::new(
			vec![TestSetupKey::B],
			vec![TestSetupKey::C],
			Cow::Borrowed("provider_c"),
		);
		// D has no dependencies (should not be in cycle)
		let provider_d =
			ProviderInfo::new(vec![], vec![TestSetupKey::D], Cow::Borrowed("provider_d"));

		tracker
			.providers
			.insert(world.register_system(|| {}), provider_a);
		tracker
			.providers
			.insert(world.register_system(|| {}), provider_b);
		tracker
			.providers
			.insert(world.register_system(|| {}), provider_c);
		tracker
			.providers
			.insert(world.register_system(|| {}), provider_d);

		let cycles = SetupTracker::detect_cycles(&tracker);

		// A, B, C should be detected as part of the cycle
		assert!(cycles.contains(&TestSetupKey::A));
		assert!(cycles.contains(&TestSetupKey::B));
		assert!(cycles.contains(&TestSetupKey::C));
		// D should not be part of the cycle
		assert!(!cycles.contains(&TestSetupKey::D));
	}

	#[test]
	fn test_no_cycles() {
		let mut world = World::new();
		let mut tracker = SetupTracker::<TestSetupKey>::new(world.register_system(|| {}));

		// Add entries for all keys
		tracker
			.entries
			.insert(TestSetupKey::A, world.register_system(|| Progress::DONE));
		tracker
			.entries
			.insert(TestSetupKey::B, world.register_system(|| Progress::DONE));
		tracker
			.entries
			.insert(TestSetupKey::C, world.register_system(|| Progress::DONE));

		// Create a linear dependency chain: A -> B -> C (no cycle)
		let provider_a =
			ProviderInfo::new(vec![], vec![TestSetupKey::A], Cow::Borrowed("provider_a"));
		let provider_b = ProviderInfo::new(
			vec![TestSetupKey::A],
			vec![TestSetupKey::B],
			Cow::Borrowed("provider_b"),
		);
		let provider_c = ProviderInfo::new(
			vec![TestSetupKey::B],
			vec![TestSetupKey::C],
			Cow::Borrowed("provider_c"),
		);

		tracker
			.providers
			.insert(world.register_system(|| {}), provider_a);
		tracker
			.providers
			.insert(world.register_system(|| {}), provider_b);
		tracker
			.providers
			.insert(world.register_system(|| {}), provider_c);

		let cycles = SetupTracker::detect_cycles(&tracker);

		// No cycles should be detected
		assert!(cycles.is_empty());
	}

	#[test]
	fn test_progress_calculation() {
		let mut world = World::new();
		let mut tracker = SetupTracker::<TestSetupKey>::new(world.register_system(|| {}));

		// Add progress checkers that return different values
		tracker.entries.insert(
			TestSetupKey::A,
			world.register_system(|| Progress::new(0.5)), // 50% complete
		);
		tracker.entries.insert(
			TestSetupKey::B,
			world.register_system(|| Progress::new(1.0)), // 100% complete
		);
		tracker.entries.insert(
			TestSetupKey::C,
			world.register_system(|| Progress::new(0.0)), // 0% complete
		);

		let progress = tracker.progress(&mut world);
		// Should be (0.5 + 1.0 + 0.0) / 3 = 0.5
		assert!((progress.into_inner() - 0.5).abs() < f32::EPSILON);
	}

	#[test]
	fn test_weighted_progress_calculation() {
		let mut world = World::new();
		let mut tracker = SetupTracker::<TestSetupKey>::new(world.register_system(|| {}));

		// Test weighted progress with different time estimates

		tracker.entries.insert(
			TestSetupKey::A,
			world.register_system(|| Progress::new(1.0)), // Complete, weight 1.0
		);
		tracker.entries.insert(
			TestSetupKey::B,
			world.register_system(|| Progress::new(0.5)), // Half done, weight 2.0
		);
		tracker.entries.insert(
			TestSetupKey::C,
			world.register_system(|| Progress::new(0.0)), // Not started, weight 1.0
		);

		let progress = tracker.progress(&mut world);
		// Should be (1.0*1.0 + 0.5*2.0 + 0.0*1.0) / (1.0 + 2.0 + 1.0) = 2.0 / 4.0 = 0.5
		assert!((progress.into_inner() - 0.5).abs() < f32::EPSILON);
	}

	#[test]
	fn test_provider_should_run() {
		let mut world = World::new();
		let mut tracker = SetupTracker::<TestSetupKey>::new(world.register_system(|| {}));

		// Add progress checkers
		tracker
			.entries
			.insert(TestSetupKey::A, world.register_system(|| Progress::DONE));
		tracker
			.entries
			.insert(TestSetupKey::B, world.register_system(|| Progress::ZERO));
		tracker
			.entries
			.insert(TestSetupKey::C, world.register_system(|| Progress::ZERO));

		// Provider that requires A (complete) and provides B (incomplete)
		let provider = ProviderInfo::new(
			vec![TestSetupKey::A],
			vec![TestSetupKey::B],
			Cow::Borrowed("test_provider"),
		);

		// Should run because A is complete and B is not
		assert!(provider.should_run(&tracker.entries, &mut world));

		// Provider that requires B (incomplete)
		let provider2 = ProviderInfo::new(
			vec![TestSetupKey::B],
			vec![TestSetupKey::C],
			Cow::Borrowed("test_provider2"),
		);

		// Should not run because B is not complete
		assert!(!provider2.should_run(&tracker.entries, &mut world));
	}

	#[test]
	fn test_stages_calculation() {
		let mut world = World::new();
		let mut tracker = SetupTracker::<TestSetupKey>::new(world.register_system(|| {}));

		// Add entries
		tracker
			.entries
			.insert(TestSetupKey::A, world.register_system(|| Progress::DONE));
		tracker
			.entries
			.insert(TestSetupKey::B, world.register_system(|| Progress::DONE));
		tracker
			.entries
			.insert(TestSetupKey::C, world.register_system(|| Progress::DONE));
		tracker
			.entries
			.insert(TestSetupKey::D, world.register_system(|| Progress::DONE));

		// Create dependency chain: A -> B -> C, D (independent)
		let provider_a =
			ProviderInfo::new(vec![], vec![TestSetupKey::A], Cow::Borrowed("provider_a"));
		let provider_b = ProviderInfo::new(
			vec![TestSetupKey::A],
			vec![TestSetupKey::B],
			Cow::Borrowed("provider_b"),
		);
		let provider_c = ProviderInfo::new(
			vec![TestSetupKey::B],
			vec![TestSetupKey::C],
			Cow::Borrowed("provider_c"),
		);
		let provider_d =
			ProviderInfo::new(vec![], vec![TestSetupKey::D], Cow::Borrowed("provider_d"));

		let system_a = world.register_system(|| {});
		let system_b = world.register_system(|| {});
		let system_c = world.register_system(|| {});
		let system_d = world.register_system(|| {});

		tracker.providers.insert(system_a, provider_a);
		tracker.providers.insert(system_b, provider_b);
		tracker.providers.insert(system_c, provider_c);
		tracker.providers.insert(system_d, provider_d);

		let stages = tracker.stages();

		// Should have 3 stages: [A, D], [B], [C]
		assert_eq!(stages.len(), 3);
		assert_eq!(stages[0].len(), 2); // A and D can run in parallel
		assert_eq!(stages[1].len(), 1); // B depends on A
		assert_eq!(stages[2].len(), 1); // C depends on B

		// Check that A and D are in the first stage
		assert!(stages[0].contains(&system_a));
		assert!(stages[0].contains(&system_d));
		assert!(stages[1].contains(&system_b));
		assert!(stages[2].contains(&system_c));
	}

	#[test]
	fn test_validation_unprovided_keys() {
		let mut world = World::new();
		let system_id = world.register_system(|| {});
		world.insert_resource(SetupTracker::<TestSetupKey>::new(system_id));

		// Add an entry that's never provided
		world.resource_scope::<SetupTracker<TestSetupKey>, _>(|world, mut tracker| {
			tracker
				.entries
				.insert(TestSetupKey::A, world.register_system(|| Progress::DONE));

			// Add a provider that requires A but doesn't provide it
			let provider = ProviderInfo::new(
				vec![TestSetupKey::A],
				vec![TestSetupKey::B],
				Cow::Borrowed("provider"),
			);
			tracker
				.providers
				.insert(world.register_system(|| {}), provider);
		});

		let result = SetupTracker::<TestSetupKey>::validate(&mut world);
		assert!(result.is_err());

		let error = result.unwrap_err();
		assert!(error.unprovided.contains(&TestSetupKey::A));
		assert!(!error.unprovided.contains(&TestSetupKey::B));
	}

	#[test]
	fn test_validation_duplicate_providers() {
		let mut world = World::new();
		let system_id = world.register_system(|| {});
		world.insert_resource(SetupTracker::<TestSetupKey>::new(system_id));

		world.resource_scope::<SetupTracker<TestSetupKey>, _>(|world, mut tracker| {
			tracker
				.entries
				.insert(TestSetupKey::A, world.register_system(|| Progress::DONE));

			// Add two providers for the same key
			let provider1 =
				ProviderInfo::new(vec![], vec![TestSetupKey::A], Cow::Borrowed("provider1"));
			let provider2 =
				ProviderInfo::new(vec![], vec![TestSetupKey::A], Cow::Borrowed("provider2"));

			tracker
				.providers
				.insert(world.register_system(|| {}), provider1);
			tracker
				.providers
				.insert(world.register_system(|| {}), provider2);
		});

		let result = SetupTracker::<TestSetupKey>::validate(&mut world);
		assert!(result.is_err());

		let error = result.unwrap_err();
		assert!(error.duplicate_providers.contains_key(&TestSetupKey::A));
		assert_eq!(error.duplicate_providers[&TestSetupKey::A].len(), 2);
	}

	#[test]
	fn test_providers_of_and_dependants_of() {
		let mut world = World::new();
		let mut tracker = SetupTracker::<TestSetupKey>::new(world.register_system(|| {}));

		let system_a = world.register_system(|| {});
		let system_b = world.register_system(|| {});

		// Provider A provides TestSetupKey::A
		let provider_a =
			ProviderInfo::new(vec![], vec![TestSetupKey::A], Cow::Borrowed("provider_a"));

		// Provider B requires TestSetupKey::A and provides TestSetupKey::B
		let provider_b = ProviderInfo::new(
			vec![TestSetupKey::A],
			vec![TestSetupKey::B],
			Cow::Borrowed("provider_b"),
		);

		tracker.providers.insert(system_a, provider_a);
		tracker.providers.insert(system_b, provider_b);

		// Test providers_of
		let providers_of_a: Vec<_> = tracker.providers_of(&TestSetupKey::A).collect();
		assert_eq!(providers_of_a.len(), 1);
		assert_eq!(providers_of_a[0].0, system_a);
		assert_eq!(providers_of_a[0].1, 0); // First (and only) provision

		// Test dependants_of
		let dependants_of_a: Vec<_> = tracker.dependants_of(&TestSetupKey::A).collect();
		assert_eq!(dependants_of_a.len(), 1);
		assert_eq!(dependants_of_a[0].0, system_b);
		assert_eq!(dependants_of_a[0].1, 0); // First (and only) requirement
	}
}
