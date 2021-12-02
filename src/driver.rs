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
use crate::program::{Program, leds_iter, TrivialProgram, PixelVal};
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
	fn status(&self) -> Status;
	fn start(&mut self, wasm_bin: Vec<u8>) -> Result<Status, Error>;
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
	fn status(&self) -> Status {
		self.status
	}

	fn start(&mut self, wasm_bin: Vec<u8>) -> Result<Status, Error> {
		self.stop();

		let (sender, receiver) = mpsc::sync_channel(0);
		let led_write_factory = self.led_write_factory.clone();
		let render_period = Duration::from_millis((1000 / self.render_freq) as u64);
		let layout_clone = self.layout.clone();
		let wasm_bin = wasm_bin.clone();
		let thread_handle = thread::spawn(move || {
			run_driver(&*led_write_factory, render_period, receiver, wasm_bin, &*layout_clone)
		});
		// Send control action to synchronize with driver thread
		match sender.send(CtrlAction::Play) {
			Ok(()) => {
				self.thread_handle = Some(thread_handle);
				self.ctrl_sender = Some(sender);
				self.status = Status::Playing;
			}
			Err(_) => {
				match thread_handle.join() {
					Ok(Ok(())) => log::error!("thread unexpectedly exited without error"),
					Ok(Err(err)) => return Err(err),
					#[cfg(test)]
					Err(_) => panic!("driver thread panicked"),
					#[cfg(not(test))]
					Err(_) => log::error!("driver thread panicked"),
				}
			},
		}

		Ok(self.status)
	}

	fn stop(&mut self) -> Status {
		match (self.thread_handle.take(), self.ctrl_sender.take()) {
			(Some(thread_handle), Some(ctrl_sender)) => {
				if let Err(err) = ctrl_sender.send(CtrlAction::Exit) {
					log::error!("could not send Exit message to driver thread: {}", err);
				}
				match thread_handle.join() {
					Ok(Ok(())) => {}
					Ok(Err(err)) => log::error!("error in driver thread: {}", err),
					#[cfg(test)]
					Err(_) => panic!("driver thread panicked"),
					#[cfg(not(test))]
					Err(_) => log::error!("driver thread panicked"),
				}
				self.status = Status::NotPlaying;
			},
			_ => {}
		}
		self.status
	}

	fn play(&mut self) -> Status {
		if let Some(ref mut ctrl_sender) = self.ctrl_sender {
			match ctrl_sender.send(CtrlAction::Play) {
				Ok(()) => self.status = Status::Playing,
				Err(err) => {
					log::error!("could not send Play message to driver thread: {}", err);
					self.stop();
				},
			}
		}
		self.status
	}

	fn pause(&mut self) -> Status {
		if let Some(ref mut ctrl_sender) = self.ctrl_sender {
			match ctrl_sender.send(CtrlAction::Pause) {
				Ok(()) => self.status = Status::Paused,
				Err(err) => {
					log::error!("could not send Pause message to driver thread: {}", err);
					self.stop();
				},
			}
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
	let runtime = create_runtime()?;
	let program = WasmProgram::new(layout, &runtime, wasm_bin)?;

	let result = driver_loop(program, render_period, ctrl_receiver, &mut led_write);
	if let Err(err) = clear_leds(layout, &mut led_write) {
		log::error!("error clearing LEDs before driver exit: {}", err);
	}
	log::info!("exiting driver thread");
	result
}

fn driver_loop<SLW>(
	mut program: WasmProgram,
	render_period: Duration,
	ctrl_receiver: Receiver<CtrlAction>,
	led_write: &mut SLW,
) -> Result<(), Error>
	where SLW: SmartLedsWrite<Error=Error, Color=RGB8>
{
	let mut playing = false;
	let mut render_at = Instant::now();
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
	Ok(())
}

fn clear_leds<SLW>(layout: &LayoutConfig, led_write: &mut SLW) -> Result<(), Error>
	where SLW: SmartLedsWrite<Error=Error, Color=RGB8>
{
	let clear_program = TrivialProgram::new(layout, PixelVal::new(0, 0, 0));
	led_write.write(leds_iter(&clear_program))
}

#[cfg(test)]
mod tests {
	use super::*;
	use assert_matches::assert_matches;
	use mockall::mock;
	use serde_json::value::Value;
	use std::sync::{Arc, Mutex};
	use mockall::predicate::eq;

	const TEST_PROGRAM: &[u8]  = include_bytes!("../testMain.wasm");

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
	fn test_driver_start_with_good_wam() {
		let layout = layout_config();
		let mut led_write = MockSmartLedsWrite::new();
		led_write.expect_write()
			.returning(|_| Ok(()));

		let led_write_ref = MockSmartLedsWriteRef::new(led_write);
		let led_write_factory = move |_layout: &LayoutConfig| Ok(led_write_ref.clone());

		let mut driver = DriverImpl::new(led_write_factory, 1000, layout);
		assert_matches!(driver.start(TEST_PROGRAM.to_vec()), Ok(Status::Playing));
		thread::sleep(Duration::from_millis(10));
		assert_eq!(driver.stop(), Status::NotPlaying);
	}

	#[test]
	fn test_driver_start_with_bad_wam() {
		let layout = layout_config();
		let mut led_write = MockSmartLedsWrite::new();
		led_write.expect_write()
			.returning(|_| Ok(()));

		let led_write_ref = MockSmartLedsWriteRef::new(led_write);
		let led_write_factory = move |_layout: &LayoutConfig| Ok(led_write_ref.clone());

		let mut driver = DriverImpl::new(led_write_factory, 1000, layout);
		assert_matches!(
			driver.start(vec![]),
			Err(Error::Wasm3(msg)) if msg == "underrun while parsing Wasm binary"
		);
	}

	#[test]
	fn test_driver_clears_leds_on_stop() {
		let layout = layout_config();
		let mut led_write = MockSmartLedsWrite::new();
		led_write.expect_write()
			.times(1)
			.returning(|_| Ok(()));
		led_write.expect_write()
			.with(eq(vec![RGB8 { r: 0, g: 0, b: 0 }; 300]))
			.times(1)
			.returning(|_| Ok(()));

		let led_write_ref = MockSmartLedsWriteRef::new(led_write);
		let led_write_factory = move |_layout: &LayoutConfig| Ok(led_write_ref.clone());

		let mut driver = DriverImpl::new(led_write_factory, 1, layout);
		assert_matches!(driver.start(TEST_PROGRAM.to_vec()), Ok(Status::Playing));
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