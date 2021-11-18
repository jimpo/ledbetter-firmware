use std::{
	thread,
	time::{Instant, Duration},
	sync::{Arc, mpsc::{self, Receiver}},
};
use log;
use serde::{Deserialize, Serialize};
use smart_leds_trait::{SmartLedsWrite, RGB8};

use crate::config::LayoutConfig;
use crate::error::Error;
use crate::program::{Program, leds_iter};
use crate::wasm_program::{WasmProgram, create_runtime};


#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub enum Status {
	NotPlaying,
	Playing,
	Paused,
}

#[derive(Debug, Clone, Copy)]
pub enum CtrlAction {
	Play,
	Pause,
	Exit,
}

#[cfg_attr(test, mockall::automock)]
pub trait Driver {
	fn start(&mut self, wasm_bin: Vec<u8>) -> Status;
	fn stop(&mut self) -> Status;
	fn play(&mut self) -> Status;
	fn pause(&mut self) -> Status;
}

pub struct DriverImpl<SLW, SLWF>
	where
		SLW: SmartLedsWrite<Error=Error, Color=RGB8>,
		SLWF: Fn(&LayoutConfig) -> Result<SLW, Error>,
{
	led_write_factory: Arc<SLWF>,
	render_freq: usize,
	layout: Arc<LayoutConfig>,
	thread_handle: Option<thread::JoinHandle<Result<(), Error>>>,
	ctrl_sender: Option<mpsc::SyncSender<CtrlAction>>,
	status: Status,
}

impl<SLW, SLWF> DriverImpl<SLW, SLWF>
	where
		SLW: SmartLedsWrite<Error=Error, Color=RGB8>,
		SLWF: Fn(&LayoutConfig) -> Result<SLW, Error>,
{
	pub fn new(led_write_factory: SLWF, render_freq: usize, layout: LayoutConfig) -> Self {
		DriverImpl {
			led_write_factory: Arc::new(led_write_factory),
			render_freq,
			layout: Arc::new(layout),
			thread_handle: None,
			ctrl_sender: None,
			status: Status::NotPlaying,
		}
	}
}

impl<SLW, SLWF> Driver for DriverImpl<SLW, SLWF>
	where
		SLW: SmartLedsWrite<Error=Error, Color=RGB8>,
		SLWF: (Fn(&LayoutConfig) -> Result<SLW, Error>) + Send + Sync + 'static,
{
	fn start(&mut self, wasm_bin: Vec<u8>) -> Status {
		if let None = self.thread_handle {
			let (sender, receiver) = mpsc::sync_channel(1);
			let led_write_factory = self.led_write_factory.clone();
			let render_period = Duration::from_millis((1000 / self.render_freq) as u64);
			let layout_clone = self.layout.clone();
			let wasm_bin = wasm_bin.clone();
			self.thread_handle = thread::spawn(move || {
				run_driver(&*led_write_factory, render_period, receiver, wasm_bin, &*layout_clone)
			}).into();
			self.ctrl_sender = Some(sender);
			self.play();
		}
		self.status
	}

	fn stop(&mut self) -> Status {
		match (self.thread_handle.take(), self.ctrl_sender.take()) {
			(Some(thread_handle), Some(ctrl_sender)) => {
				if let Err(err) = ctrl_sender.send(CtrlAction::Exit) {
					log::error!("could not send Exit message to driver thread: {}", err);
				}
				if let Err(_) = thread_handle.join() {
					log::error!("driver thread panicked");
				}
				self.status = Status::NotPlaying;
			},
			_ => {}
		}
		self.status
	}

	fn play(&mut self) -> Status {
		if let Some(ref mut ctrl_sender) = self.ctrl_sender {
			if let Err(err) = ctrl_sender.send(CtrlAction::Play) {
				log::error!("could not send Play message to driver thread: {}", err);
			}
			self.status = Status::Playing;
		}
		self.status
	}

	fn pause(&mut self) -> Status {
		if let Some(ref mut ctrl_sender) = self.ctrl_sender {
			if let Err(err) = ctrl_sender.send(CtrlAction::Pause) {
				log::error!("could not send Pause message to driver thread: {}", err);
			}
			self.status = Status::Paused;
		}
		self.status
	}
}

fn run_driver<SLW, SLWF>(
	led_write_factory: &SLWF,
	render_period: Duration,
	ctrl_receiver: Receiver<CtrlAction>,
	wasm_bin: Vec<u8>,
	layout: &LayoutConfig,
) -> Result<(), Error>
	where
		SLW: SmartLedsWrite<Error=Error, Color=RGB8>,
		SLWF: Fn(&LayoutConfig) -> Result<SLW, Error>,
{
	let mut led_write = led_write_factory(layout)?;
	let mut playing = false;
	let mut render_at = Instant::now();
	let runtime = create_runtime()?;
	let mut program = WasmProgram::new(layout, &runtime, wasm_bin)?;

	loop {
		let timeout = render_at.saturating_duration_since(Instant::now());
		match ctrl_receiver.recv_timeout(timeout) {
			Ok(CtrlAction::Play) => playing = true,
			Ok(CtrlAction::Pause) => playing = false,
			Ok(CtrlAction::Exit) => break,
			Err(mpsc::RecvTimeoutError::Disconnected) => {
				log::warn!("Driver control channel unexpectedly disconnected");
				break;
			},
			Err(mpsc::RecvTimeoutError::Timeout) => {
				if playing {
					program.tick()?;
					led_write.write(leds_iter(&program))?;
				}
				render_at += render_period;
			},
		}
	}
	log::info!("exiting driver thread");
	Ok(())
}

#[cfg(test)]
mod tests {
	use super::*;
	use mockall::mock;
	use serde_json::value::Value;
	use std::sync::{Arc, Mutex};

	fn layout_config() -> LayoutConfig {
		let ys = (0..150).map(|i| (i as f32) / 60.0).collect::<Vec<_>>();
		LayoutConfig {
			pixel_locations: vec![
				ys.iter().map(|&y| (-10.0, y)).collect(),
				ys.iter().map(|&y| (10.0, y)).collect(),
			],
		}
	}

	mock! {
		#[derive(Clone)]
		SmartLedsWrite {
			fn write(&mut self, items: Vec<RGB8>) -> Result<(), Error>;
		}
	}

	#[derive(Clone)]
	struct MockSmartLedsWriteRef(Arc<Mutex<MockSmartLedsWrite>>);

	impl MockSmartLedsWriteRef {
		fn new(inner: MockSmartLedsWrite) -> Self {
			MockSmartLedsWriteRef(Arc::new(Mutex::new(inner)))
		}
	}

	impl SmartLedsWrite for MockSmartLedsWriteRef {
		type Error=Error;
		type Color=RGB8;

		fn write<T, I>(&mut self, iterator: T) -> Result<(), Error>
			where
				T: Iterator<Item = I>,
				I: Into<RGB8>,
		{
			let mut inner = self.0.lock().unwrap();
			inner.write(iterator.map(|rgb| rgb.into()).collect())
		}
	}

	#[test]
	fn test_driver() {
		let layout = layout_config();
		let mut led_write = MockSmartLedsWrite::new();
		led_write.expect_write()
			.returning(|_| Ok(()));

		let led_write_ref = MockSmartLedsWriteRef::new(led_write);
		let led_write_factory = move |layout: &LayoutConfig| Ok(led_write_ref.clone());

		let mut driver = DriverImpl::new(led_write_factory, 1000, layout);
		assert_eq!(driver.start(vec![]), Status::Playing);
		thread::sleep(Duration::from_millis(10));
		assert_eq!(driver.stop(), Status::NotPlaying);
	}

	#[test]
	fn test_status_serialization() {
		assert_eq!(
			serde_json::to_value(&Status::NotPlaying).unwrap(),
			Value::String("NotPlaying".into())
		);
		assert_eq!(
			serde_json::to_value(&Status::Playing).unwrap(),
			Value::String("Playing".into())
		);
		assert_eq!(
			serde_json::to_value(&Status::Paused).unwrap(),
			Value::String("Paused".into())
		);
	}
}