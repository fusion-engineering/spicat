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
use std::os::unix::io::AsRawFd;

#[derive(Debug, Copy, Clone)]
#[derive(clap::ValueEnum)]
enum OutputFormat {
	#[clap(name = "dec")]
	#[clap(alias = "decimal")]
	Decimal,
	#[clap(name = "hex")]
	#[clap(alias = "hexadecimal")]
	Hexadecimal,
	Raw,
}

#[repr(u8)]
#[derive(Debug, Copy, Clone)]
#[derive(clap::ValueEnum)]
enum SpiMode {
	#[clap(name = "0")]
	M0 = 0,
	#[clap(name = "1")]
	M1 = 1,
	#[clap(name = "2")]
	M2 = 2,
	#[clap(name = "3")]
	M3 = 3,
}

impl SpiMode {
	fn flags(&self) -> SpiModeFlags {
		match *self {
			Self::M0 => SpiModeFlags::SPI_MODE_0,
			Self::M1 => SpiModeFlags::SPI_MODE_1,
			Self::M2 => SpiModeFlags::SPI_MODE_2,
			Self::M3 => SpiModeFlags::SPI_MODE_3,
		}
	}
}

#[repr(u8)]
#[derive(Debug, Copy, Clone)]
#[derive(clap::ValueEnum)]
enum ChipSelect {
	ActiveLow,
	ActiveHigh,
	Disabled,
}

impl ChipSelect {
	fn flags(&self) -> SpiModeFlags {
		match *self {
			ChipSelect::ActiveLow  => SpiModeFlags::empty(),
			ChipSelect::ActiveHigh => SpiModeFlags::SPI_CS_HIGH,
			ChipSelect::Disabled   => SpiModeFlags::SPI_NO_CS,
		}
	}
}

impl std::fmt::Display for ChipSelect {
	fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
		match self {
			ChipSelect::ActiveLow  => write!(f, "active-low"),
			ChipSelect::ActiveHigh => write!(f, "active-high"),
			ChipSelect::Disabled   => write!(f, "disabled"),
		}
	}
}

#[derive(clap::Parser)]
#[clap(author = "Fusion Engineering")]
struct Options {
	/// The spidev to open.
	#[clap(value_name = "SPIDEV")]
	spidev: PathBuf,

	/// Read input from a file, or - for standard input.
	#[clap(long = "in", short)]
	#[clap(value_name = "PATH")]
	#[clap(default_value = "-")]
	input: PathBuf,

	/// Write output to a file, or - for standard output.
	#[clap(long = "out", short)]
	#[clap(value_name = "PATH")]
	#[clap(default_value = "-")]
	output: PathBuf,

	/// The speed in Hz for the SPI transaction.
	#[clap(long, short)]
	#[clap(value_name = "HZ")]
	#[clap(default_value = "1000000")]
	speed: u32,

	/// Repeat the transaction COUNT times,
	#[clap(long, short)]
	#[clap(value_name = "COUNT")]
	#[clap(default_value = "1")]
	repeat: usize,

	/// Print the response in the given format: raw, hex[adecimal] or dec[imal].
	///
	/// If not specified, the output format depends on whether output if going to a TTY.
	/// If it is, hex is used by default, otherwise raw is used.
	#[clap(long, short)]
	format: Option<OutputFormat>,

	/// SPI mode to use: 0, 1, 2 or 3.
	#[clap(long)]
	#[clap(value_enum)]
	#[clap(value_name = "MODE")]
	#[clap(default_value = "0")]
	mode: SpiMode,

	/// Chip select mode: active-low, active-high or disabled.
	#[clap(long)]
	#[clap(value_enum)]
	#[clap(default_value = "active-low")]
	chip_select: ChipSelect,

	/// Bits per word for the SPI transaction.
	#[clap(long = "bits")]
	#[clap(value_enum)]
	#[clap(value_name = "N")]
	#[clap(default_value = "8")]
	bits_per_word: u8,

	/// Delay in microseconds after enabling the chip select line before sending data.
	#[clap(long)]
	#[clap(value_name = "MICROSECONDS")]
	pre_delay: Option<u16>,
}

fn main() {
	do_main(clap::Parser::parse())
		.unwrap_or_else(|error| {
			eprintln!("Error: {}", error);
			std::process::exit(1);
		});
}

fn do_main(options: Options) -> Result<(), String> {
	let stdin  = std::io::stdin();
	let stdout = std::io::stdout();

	let mut spi = Spidev::open(&options.spidev)
		.map_err(|e| format!("Failed to open spidev {}: {}", options.spidev.display(), e))?;
	spi.configure(&SpidevOptions::new().bits_per_word(options.bits_per_word).build())
		.map_err(|e| format!("Failed to set {} bits per word: {}", options.bits_per_word, e))?;
	spi.configure(&SpidevOptions::new().max_speed_hz(options.speed).build())
		.map_err(|e| format!("Failed to max speed to {} Hz: {}", options.speed, e))?;
	spi.configure(&SpidevOptions::new().mode(options.mode.flags()).build())
		.map_err(|e| format!("Failed to set SPI mode to {}: {}", options.mode as u8, e))?;
	spi.configure(&SpidevOptions::new().mode(options.chip_select.flags()).build())
		.map_err(|e| format!("Failed to set chip select mode to {}: {}", options.chip_select, e))?;


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
