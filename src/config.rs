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

#[cfg(test)]
mod tests {
	use super::*;
	use assert_matches::assert_matches;

	const EXAMPLE_CONFIG: &str = include_str!("../config.toml");

	#[test]
	fn test_deserialize_example_config() {
		let config = toml::from_str(EXAMPLE_CONFIG).unwrap();
		assert_matches!(config, Config {
			name,
			render_freq,
			output: OutputConfig::Terminal,
			controller: ControllerConfig { host, port },
			layout: _layout,
		} => {
			assert_eq!(&name, "Local test");
			assert_eq!(render_freq, 1);
			assert_eq!(&host, "127.0.0.1");
			assert_eq!(port, 3000);
		});
	}
}