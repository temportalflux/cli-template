use std::sync::{
	atomic::{self, AtomicBool},
	Arc,
};

use anyhow::Context;
use clap::Parser;

pub mod utility;

#[derive(Parser, Debug)]
#[clap(version, about, long_about = None)]
pub enum Cli {}

/// Delegate execution to the various subcommands.
impl utility::Operation for Cli {
	fn run(self) -> utility::PinFuture<anyhow::Result<()>> {
		Box::pin(async move { Ok(()) })
	}
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	let terminate_signal = Arc::new(AtomicBool::new(false));
	let _ = signal_hook::flag::register(signal_hook::consts::SIGINT, terminate_signal.clone());

	let term_handle = tokio::task::spawn(async move {
		while !terminate_signal.load(atomic::Ordering::Relaxed) {
			tokio::time::sleep(std::time::Duration::from_millis(100)).await;
		}
		println!("Encountered terminate signal, cli task will be aborted");
	});
	let cli_handle = tokio::task::spawn(async move {
		if let Err(err) = run_cli().await {
			eprintln!("{err:?}");
		}
	});

	futures::future::select(cli_handle, term_handle).await;

	Ok(())
}

async fn run_cli() -> anyhow::Result<()> {
	use utility::Operation;
	// Parse the command line args as a cli operation
	let cli = Cli::parse();
	// Construct the error context because `run` takes ownership of `cli`
	let failed_context = format!("failed to run {cli:?}");
	// Actually run the desired commmand with the loaded configuration
	cli.run().await.context(failed_context)?;
	Ok(())
}
