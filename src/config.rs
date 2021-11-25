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
	pub render_freq: usize,
	pub output: OutputConfig,
	pub controller: ControllerConfig,
	pub layout: LayoutConfig,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "target")]
pub enum OutputConfig {
	#[serde(rename = "terminal")]
	Terminal,
	#[serde(rename = "rpi")]
	Rpi { pins: Vec<u32> }
}
