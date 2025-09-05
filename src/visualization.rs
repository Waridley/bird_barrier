//! Graph visualization for setup dependencies using egui and snarl.
//!
//! This module provides tools to visualize the setup dependency graph in real-time,
//! showing how providers depend on each other and the current state of setup progress.
//!
//! # Features
//!
//! - Interactive node-based graph visualization
//! - Color-coded pins showing different setup keys
//! - Real-time updates as setup progresses
//! - Automatic layout based on dependency stages
//!
//! # Usage
//!
//! Add the visualization plugin to your app:
//!
//! ```rust,no_run
//! use bevy::prelude::*;
//! use bird_barrier::*;
//! # #[derive(Debug, Clone, Hash, PartialEq, Eq)]
//! # enum MySetupKey { LoadAssets }
//! # impl SetupKey for MySetupKey {
//! #     fn register_progress_checker(&self, world: &mut World) -> bevy::ecs::system::SystemId<(), Progress> {
//! #         world.register_system(|world: &World| Progress::DONE)
//! #     }
//! # }
//!
//! let mut app = App::new();
//! app.add_plugins((
//!     DefaultPlugins,
//!     SetupTrackingPlugin::<MySetupKey, _, _, _, _>::new(|| true, || {}),
//!     SetupGraphVisualizationPlugin::<MySetupKey>::default(),
//! ));
//! ```
//!
//! ## Displaying the Graph
//!
//! You have several options for displaying the graph:
//!
//! ### Option 1: Dedicated Window (Automatic)
//! ```rust,no_run
//! # use bevy::prelude::*;
//! # use bird_barrier::*;
//! # #[derive(Debug, Clone, Hash, PartialEq, Eq)]
//! # enum MySetupKey { LoadAssets }
//! # impl SetupKey for MySetupKey {
//! #     fn register_progress_checker(&self, world: &mut World) -> bevy::ecs::system::SystemId<(), Progress> {
//! #         world.register_system(|world: &World| Progress::DONE)
//! #     }
//! # }
//! fn toggle_graph_on_key_press(
//!     mut commands: Commands,
//!     keys: Res<ButtonInput<KeyCode>>,
//!     state: Option<Res<SetupGraphVisState<MySetupKey>>>,
//! ) {
//!     if keys.just_pressed(KeyCode::KeyG) {
//!         commands.run_system_cached(toggle_setup_graph_window::<MySetupKey>);
//!     }
//! }
//! ```
//!
//! ### Option 2: Custom Window or Panel
//! ```rust,no_run
//! # use bevy::prelude::*;
//! # use bevy_egui::*;
//! # use bird_barrier::*;
//! # #[derive(Debug, Clone, Hash, PartialEq, Eq)]
//! # enum MySetupKey { LoadAssets }
//! # impl SetupKey for MySetupKey {
//! #     fn register_progress_checker(&self, world: &mut World) -> bevy::ecs::system::SystemId<(), Progress> {
//! #         world.register_system(|world: &World| Progress::DONE)
//! #     }
//! # }
//! fn custom_graph_window(
//!     graph: Res<SetupTracker<MySetupKey>>,
//!     mut contexts: EguiContexts,
//!     mut state: Option<ResMut<SetupGraphVisState<MySetupKey>>>,
//! ) {
//!     if let Ok(ctx) = contexts.ctx_mut() {
//!         if let Some(state) = &mut state {
//!             egui::Window::new("My Custom Graph Window")
//!                 .show(ctx, |ui| {
//!                     draw_setup_graph(ui, &*graph, state);
//!                 });
//!         }
//!     }
//! }
//! ```

use crate::{SetupKey, SetupTracker};
use bevy_app::{App, Plugin, PreUpdate};
use bevy_ecs::prelude::*;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass};
use bevy_platform::collections::HashMap;
use std::fmt::Debug;
use std::marker::PhantomData;
use std::ops::Deref;

use bevy_egui::egui::{Color32, Ui};
use bevy_log::{error, info, trace};
use egui_snarl::ui::{NodeLayout, PinInfo, SnarlPin, SnarlStyle, SnarlViewer, WireStyle};
use egui_snarl::{InPin, InPinId, NodeId, OutPin, OutPinId, Snarl};

/// Plugin that adds graph visualization capabilities for setup dependencies.
///
/// This plugin provides the core infrastructure for visualizing setup dependency graphs.
/// It does not include any automatic window spawning or hotkeys - users have full control
/// over when and how to display the visualization.
///
/// # Type Parameters
///
/// * `K` - The setup key type that implements `SetupKey + Debug`
///
/// # Features Required
///
/// This plugin requires the `visualization` feature to be enabled.
pub struct SetupGraphVisualizationPlugin<K: SetupKey> {
	_marker: PhantomData<K>,
}

impl<K: SetupKey> Default for SetupGraphVisualizationPlugin<K> {
	fn default() -> Self {
		Self {
			_marker: PhantomData,
		}
	}
}

impl<K: SetupKey + Debug + Send + Sync + 'static> Plugin for SetupGraphVisualizationPlugin<K> {
	fn build(&self, app: &mut App) {
		app.add_systems(
			PreUpdate,
			sync_snarl::<K>.run_if(resource_exists::<SetupGraphVisState<K>>),
		)
		.add_systems(EguiPrimaryContextPass, draw_setup_graph_window::<K>);
	}
}

/// Wrapper around SetupTracker that implements SnarlViewer for graph visualization.
pub struct SetupGraphViewer<'a, K: SetupKey>(&'a SetupTracker<K>);

impl<'a, K: SetupKey> Deref for SetupGraphViewer<'a, K> {
	type Target = SetupTracker<K>;

	fn deref(&self) -> &Self::Target {
		self.0
	}
}

impl<'a, K: SetupKey> SetupGraphViewer<'a, K> {
	/// Get a color for a setup key based on its position in the dependency graph.
	/// Final outputs (keys with no dependants) are colored white.
	pub fn key_color(&self, key: &K) -> Option<Color32> {
		// Final outputs are white
		if self.dependants_of(key).next().is_none() {
			return Some(Color32::WHITE);
		}

		let mut i = 0;
		for (k, _) in self.entries().iter() {
			if self.dependants_of(k).next().is_none() {
				// Skip counting outputs that will be white anyway
				continue;
			}
			if *k == *key {
				break;
			}
			i += 1;
		}

		Some(COLORS[i % COLORS.len()])
	}
}

/// Color palette for setup keys in the visualization.
const COLORS: &[Color32] = &[
	Color32::RED,
	Color32::from_rgb(255, 127, 0), // Orange
	Color32::YELLOW,
	Color32::GREEN,
	Color32::from_rgb(0, 255, 127), // Spring green
	Color32::from_rgb(0, 255, 255), // Cyan
	Color32::from_rgb(0, 127, 255), // Sky blue
	Color32::BLUE,
	Color32::from_rgb(127, 0, 255), // Purple
	Color32::from_rgb(255, 0, 255), // Magenta
	Color32::from_rgb(255, 0, 127), // Rose
];

impl<K: SetupKey + Debug> SnarlViewer<bevy_ecs::system::SystemId> for SetupGraphViewer<'_, K> {
	fn title(&mut self, node: &bevy_ecs::system::SystemId) -> String {
		self.providers()[node].name().to_owned()
	}

	fn outputs(&mut self, node: &bevy_ecs::system::SystemId) -> usize {
		self.providers()[node].provides().len()
	}

	fn inputs(&mut self, node: &bevy_ecs::system::SystemId) -> usize {
		self.providers()[node].requires().len()
	}

	fn show_input(
		&mut self,
		pin: &InPin,
		ui: &mut Ui,
		snarl: &mut Snarl<bevy_ecs::system::SystemId>,
	) -> impl SnarlPin + 'static {
		let key = &self.providers()[&snarl[pin.id.node]].requires()[pin.id.input];
		let fill = self.key_color(key);
		ui.label(format!("{key:?}"));
		PinInfo {
			fill,
			..Default::default()
		}
	}

	fn show_output(
		&mut self,
		pin: &OutPin,
		ui: &mut Ui,
		snarl: &mut Snarl<bevy_ecs::system::SystemId>,
	) -> impl SnarlPin + 'static {
		let key = &self.providers()[&snarl[pin.id.node]].provides()[pin.id.output];
		let fill = self.key_color(key);
		ui.label(format!("{key:?}"));
		PinInfo {
			fill,
			..Default::default()
		}
	}
}

/// Resource that holds the snarl graph state for visualization.
#[derive(Resource, Debug)]
pub struct SetupGraphVisState<K: SetupKey> {
	snarl: Snarl<bevy_ecs::system::SystemId>,
	_marker: PhantomData<K>,
}

impl<K: SetupKey> Default for SetupGraphVisState<K> {
	fn default() -> Self {
		Self {
			snarl: Default::default(),
			_marker: PhantomData,
		}
	}
}

/// System that synchronizes the snarl graph with the current setup tracker state.
pub fn sync_snarl<K: SetupKey>(
	mut snarl: ResMut<SetupGraphVisState<K>>,
	tracker: Res<SetupTracker<K>>,
) {
	let mut nodes = snarl
		.snarl
		.nodes_ids_data()
		.map(|(id, node)| (id, node.value))
		.collect::<HashMap<NodeId, bevy_ecs::system::SystemId>>();

	if tracker.is_changed() || snarl.is_added() {
		// Add nodes for each provider, arranged by stage
		for (i, stage) in tracker.stages().into_iter().enumerate() {
			for (j, id) in stage.into_iter().enumerate() {
				if !nodes.iter().any(|(_, node)| *node == id) {
					let node = snarl.snarl.insert_node(
						bevy_egui::egui::Pos2::new(i as f32 * 400.0, j as f32 * 96.0),
						id,
					);
					nodes.insert(node, id);
				}
			}
		}

		// Connect nodes based on dependencies
		for (id, info) in tracker.providers() {
			let Some(provider_node) = nodes
				.iter()
				.find_map(|(nid, node)| (*node == *id).then_some(*nid))
			else {
				bevy_log::error!("Missing Snarl node for provider: {id:?}");
				continue;
			};

			for (output_idx, provision) in info.provides().iter().enumerate() {
				for (dependant, input_idx) in tracker.dependants_of(provision) {
					let Some(dependant_node) = nodes
						.iter()
						.find_map(|(nid, node)| (*node == dependant).then_some(*nid))
					else {
						bevy_log::error!("Missing Snarl node for dependency: {dependant:?}");
						continue;
					};

					snarl.snarl.connect(
						OutPinId {
							node: provider_node,
							output: output_idx,
						},
						InPinId {
							node: dependant_node,
							input: input_idx,
						},
					);
				}
			}
		}
	}
}

/// Draws the setup graph visualization within the provided UI context.
///
/// This function can be called from within any egui window or panel to render
/// the setup dependency graph. It provides full control over where and how
/// the graph is displayed.
///
/// # Parameters
///
/// * `ui` - The egui UI context to draw within
/// * `graph` - The setup tracker containing the dependency graph
/// * `state` - The visualization state (must be initialized first)
///
/// # Returns
///
/// Returns the response from the snarl widget, which can be used to detect
/// interactions with the graph.
pub fn draw_setup_graph<K: SetupKey + Debug>(
	ui: &mut bevy_egui::egui::Ui,
	graph: &SetupTracker<K>,
	state: &mut SetupGraphVisState<K>,
) {
	let style = SnarlStyle {
		node_layout: Some(NodeLayout::sandwich()),
		pin_fill: Some(Color32::WHITE),
		wire_width: Some(2.0),
		wire_style: Some(WireStyle::AxisAligned { corner_radius: 8.0 }),
		bg_pattern_stroke: Some(bevy_egui::egui::Stroke {
			width: 1.0,
			color: Color32::from_gray(64),
		}),
		centering: Some(true),
		..Default::default()
	};

	state.snarl.show(
		&mut SetupGraphViewer(graph),
		&style,
		std::any::type_name::<SetupTracker<K>>(),
		ui,
	);
}

/// Opens the setup graph visualization window.
///
/// This function programmatically opens the dedicated visualization window.
/// The window will remain open until closed by the user or by calling
/// `close_setup_graph_window()`.
pub fn open_setup_graph_window<K: SetupKey>(mut commands: Commands) {
	info!("Opening setup graph window");
	commands.init_resource::<SetupGraphVisState<K>>();
}

/// Closes the setup graph visualization window.
///
/// This system programmatically closes the dedicated visualization window.
pub fn close_setup_graph_window<K: SetupKey>(mut commands: Commands) {
	info!("Closing setup graph window");
	commands.remove_resource::<SetupGraphVisState<K>>();
}

/// Toggles the setup graph visualization window.
///
/// This system opens the window if it's closed, or closes it if it's open.
/// Returns true if the window is now open, false if it's now closed.
pub fn toggle_setup_graph_window<K: SetupKey>(
	mut commands: Commands,
	state: Option<Res<SetupGraphVisState<K>>>,
) {
	if state.is_some() {
		info!("Closing setup graph window");
		commands.remove_resource::<SetupGraphVisState<K>>();
	} else {
		info!("Opening setup graph window");
		commands.init_resource::<SetupGraphVisState<K>>();
	}
}

/// System that renders the setup graph visualization in a dedicated window.
///
/// This system automatically creates and manages a window for the graph visualization.
/// The window can be opened/closed programmatically using the provided functions.
pub fn draw_setup_graph_window<K: SetupKey + Debug>(
	mut commands: Commands,
	graph: Res<SetupTracker<K>>,
	mut contexts: EguiContexts,
	mut state: Option<ResMut<SetupGraphVisState<K>>>,
) {
	let Ok(ctx) = contexts.ctx_mut() else {
		error!("No egui context");
		return;
	};

	let mut open = state.is_some();
	trace!(open);
	let was_open = open;
	bevy_egui::egui::Window::new(format!(
		"SetupTracker<{}> Graph",
		disqualified::ShortName::of::<K>()
	))
	.open(&mut open)
	.default_width(1200.0)
	.default_height(800.0)
	.show(ctx, |ui| {
		if let Some(state) = &mut state {
			draw_setup_graph(ui, &*graph, &mut *state);
		}
	});

	if was_open && !open {
		commands.remove_resource::<SetupGraphVisState<K>>();
	} else if !was_open && open {
		commands.init_resource::<SetupGraphVisState<K>>();
	}
}
