//! A command line tool to perform full-duplex SPI transactions.
//!
//! Example:
//! ```sh
//! $ echo -n 'Hello world!' | spicat /dev/spidev1.0 --speed 10000000
//! 48 65 6c 6c 6f 20 74 68 65 72 65 21
//! ```
//!
//! This will read 'Hello world!' from standard input,
//! and send it over SPI to the connected device.
//! The response will be printed to standard output.
//!
//! The output format depends on whether or not output is going to a terminal.
//! If it is, output is printed in hexadecimal format by default.
//! Otherwise, the raw bytes are printed by default.
//! This behaviour can be overridden with the `--format` option.
//!
//! The transaction can be repeated a number of times with the `--repeat` option,
//! to stress-test an SPI bus or device.
//!
//! The `--pre-delay` option can be used to add a delay after asserting the chip select,
//! before transmitting the data.
//! This can be useful to give an SPI device some time to react to the chip select.
//! Note that this wait time is implemented by the Linux kernel,
//! which may mean the exact delay can be a few microseconds longer than the requested value.
//!
//! See `spicat --help` for a list of every available option.
//!
//! # Install
//! Run `cargo install spicat` to install the tool with cargo.

use spidev::{Spidev, SpiModeFlags, SpidevTransfer, SpidevOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use structopt::StructOpt;
use std::os::unix::io::AsRawFd;

enum OutputFormat {
	Decimal,
	Hexadecimal,
	Raw,
}

impl std::str::FromStr for OutputFormat {
	type Err = String;

	fn from_str(value: &str) -> Result<Self, String> {
		let lower = value.to_lowercase();
		match lower.as_str() {
			"raw"                 => Ok(OutputFormat::Raw),
			"hex" | "hexadecimal" => Ok(OutputFormat::Hexadecimal),
			"dec" | "decimal"     => Ok(OutputFormat::Decimal),
			_ => Err(format!("invalid output format, allowed values are: raw, hex or dec, got: {}", value)),
		}
	}
}

enum SpiMode {
	M0,
	M1,
	M2,
	M3,
}

impl std::str::FromStr for SpiMode {
	type Err = String;

	fn from_str(value: &str) -> Result<Self, String> {
		match value {
			"0" => Ok(SpiMode::M0),
			"1" => Ok(SpiMode::M1),
			"2" => Ok(SpiMode::M2),
			"3" => Ok(SpiMode::M3),
			_ => Err(format!("invalid output format, allowed values are: raw, hex or dec, got: {}", value)),
		}
	}
}

impl SpiMode {
	fn flags(&self) -> SpiModeFlags {
		match *self {
			SpiMode::M0 => SpiModeFlags::SPI_MODE_0,
			SpiMode::M1 => SpiModeFlags::SPI_MODE_1,
			SpiMode::M2 => SpiModeFlags::SPI_MODE_2,
			SpiMode::M3 => SpiModeFlags::SPI_MODE_3,
		}
	}
}

#[derive(StructOpt)]
#[structopt(author = "Fusion Engineering")]
#[structopt(raw(setting = "structopt::clap::AppSettings::ColoredHelp"))]
#[structopt(raw(setting = "structopt::clap::AppSettings::DeriveDisplayOrder"))]
struct Options {
	/// The spidev to open.
	#[structopt(value_name = "SPIDEV")]
	spidev: PathBuf,

	/// Read input from a file, or - for standard input.
	#[structopt(long = "in", short = "i")]
	#[structopt(value_name = "PATH")]
	#[structopt(default_value = "-")]
	input: PathBuf,

	/// Write output to a file, or - for standard output.
	#[structopt(long = "out", short = "o")]
	#[structopt(value_name = "PATH")]
	#[structopt(default_value = "-")]
	output: PathBuf,

	/// The speed in Hz for the SPI transaction.
	#[structopt(long = "speed", short = "s")]
	#[structopt(value_name = "HZ")]
	#[structopt(default_value = "1000000")]
	speed: u32,

	/// Repeat the transaction COUNT times,
	/// The speed in Hz for the SPI transaction.
	#[structopt(long = "repeat", short = "r")]
	#[structopt(value_name = "COUNT")]
	#[structopt(default_value = "1")]
	repeat: usize,

	/// Print the response in the given format: raw, hex[adecimal] or dec[imal].
	/// If not specified, the output format depends on whether output if going to a TTY.
	/// If it is, hex is used by default, otherwise raw is used.
	#[structopt(long = "format", short = "f")]
	format: Option<OutputFormat>,

	/// SPI mode to use: 0, 1, 2 or 3.
	#[structopt(long = "--mode")]
	#[structopt(value_name = "MODE")]
	#[structopt(default_value = "0")]
	spi_mode: SpiMode,

	/// Bits per word for the SPI transaction.
	#[structopt(long = "bits")]
	#[structopt(value_name = "N")]
	#[structopt(default_value = "8")]
	bits_per_word: u8,

	/// Delay in microseconds after enabling the chip select line before sending data.
	#[structopt(long = "pre_delay")]
	#[structopt(value_name = "MICROSECONDS")]
	pre_delay: Option<u16>,
}

fn main() {
	do_main(Options::from_args()).unwrap_or_else(|error| {
		eprintln!("Error: {}", error);
		std::process::exit(1);
	});
}

fn do_main(options: Options) -> Result<(), String> {
	let stdin  = std::io::stdin();
	let stdout = std::io::stdout();

	let mut spi = Spidev::open(&options.spidev)
		.map_err(|e| format!("Failed to open spidev {}: {}", options.spidev.display(), e))?;

	spi.configure(&SpidevOptions::new()
		.bits_per_word(options.bits_per_word)
		.max_speed_hz(options.speed)
		.mode(options.spi_mode.flags())
		.build()
	).map_err(|e| format!("Failed to configure spidev: {}", e))?;

	let mut input : Box<dyn Read> = if options.input == Path::new("-") {
		Box::new(stdin.lock())
	} else {
		Box::new(std::fs::File::open(&options.input)
			.map_err(|e| format!("Failed to open input file {}: {}", options.input.display(), e))?
		)
	};

	let output_fd: i32;
	let mut output : Box<dyn Write> = if options.output == Path::new("-") {
		let stdout = stdout.lock();
		output_fd = stdout.as_raw_fd();
		Box::new(stdout)
	} else {
		let file = std::fs::File::create(&options.output)
			.map_err(|e| format!("Failed to create output file {}: {}", options.output.display(), e))?;
		output_fd = file.as_raw_fd();
		Box::new(file)
	};

	let mut tx_buf = Vec::new();
	input.read_to_end(&mut tx_buf)
		.map_err(|e| format!("Failed to read input message: {}", e))?;

	let mut rx_buf = Vec::new();
	rx_buf.resize(tx_buf.len(), 0u8);

	let format = options.format.unwrap_or_else(|| {
		if unsafe { libc::isatty(output_fd) } != 0 {
			OutputFormat::Hexadecimal
		} else {
			OutputFormat::Raw
		}
	});

	for _ in 0..options.repeat {
		// If we have a pre-delay, add a dummy write with delay_usecs and cs_change = 0.
		if let Some(pre_delay) = options.pre_delay {
			let mut transfers = [
				SpidevTransfer::write(&[]),
				SpidevTransfer::read_write(&tx_buf, &mut rx_buf),
			];

			transfers[0].cs_change   = 0;
			transfers[0].delay_usecs = pre_delay;
			transfers[0].speed_hz    = options.speed;
			transfers[1].speed_hz    = options.speed;
			spi.transfer_multiple(&mut transfers)
				.map_err(|e| format!("SPI transaction failed: {}", e))?;

		// Else just do the single transfer.
		} else {
			let mut transfer  = SpidevTransfer::read_write(&tx_buf, &mut rx_buf);
			transfer.speed_hz = options.speed;
			spi.transfer(&mut transfer)
				.map_err(|e| format!("SPI transaction failed: {}", e))?;
		}


		// Print the received data in the desired format.
		match format {
			OutputFormat::Raw => {
				output.write_all(&rx_buf).map_err(|e| format!("Failed to write to output stream: {}", e))?;
			},
			OutputFormat::Hexadecimal => {
				for (i, byte) in rx_buf.iter().enumerate() {
					if i != 0 {
						write!(output, " ").map_err(|e| format!("Failed to write to output stream: {}", e))?;
					}
					write!(output, "{:02X}", byte).map_err(|e| format!("Failed to write to output stream: {}", e))?;
				}
				writeln!(output).map_err(|e| format!("Failed to write to output stream: {}", e))?;
			},
			OutputFormat::Decimal => {
				for (i, byte) in rx_buf.iter().enumerate() {
					if i != 0 {
						write!(output, " ").map_err(|e| format!("Failed to write to output stream: {}", e))?;
					}
					write!(output, "{}", byte).map_err(|e| format!("Failed to write to output stream: {}", e))?;
				}
				writeln!(output).map_err(|e| format!("Failed to write to output stream: {}", e))?;
			},
		}
	}

	Ok(())
}
