use serde::{Deserialize};

#[derive(Debug, Clone, Deserialize)]
pub struct ControllerConfig {
	pub host: String,
	pub port: u16,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LayoutConfig {
	pub pixel_locations: Vec<Vec<(f32, f32)>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
	pub name: String,
	pub gpio_label: String,
	pub gpio_line: u32,
	pub render_freq: usize,
	pub controller: ControllerConfig,
}