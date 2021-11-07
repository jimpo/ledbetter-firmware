#[derive(Debug, derive_more::Display, derive_more::Error, derive_more::From)]
pub enum Error {
	#[from(ignore)]
	#[display(fmt = "No GPIO cdev found with label {}", label)]
	GpioCdevNotFound { label: String },
	GpioCdev(gpio_cdev::Error),
}
