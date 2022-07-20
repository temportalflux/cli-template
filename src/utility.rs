use futures::Future;
use std::pin::Pin;
use tokio::task::JoinHandle;

// Alias used to represent a future that can be returned from a trait function,
// because async is not supported for trait functions yet.
pub type PinFuture<T> = PinFutureLifetime<'static, T>;
pub type PinFutureLifetime<'l, T> = Pin<Box<dyn Future<Output = T> + 'l + Send>>;

pub trait Operation {
	fn run(self) -> PinFuture<anyhow::Result<()>>;
}

/// Spawns the command as a child process in a detached task.
/// Output and Error streams are parsed as strings in real-time, and printed to program output in their own detached tasks.
/// Ends when the child process and all stream readers are complete, returning the join-task results.
pub async fn spawn_command(
	command: &mut tokio::process::Command,
	attach: bool,
) -> anyhow::Result<()> {
	use anyhow::Context;
	use std::process::Stdio;
	use tokio::io::{AsyncBufReadExt, BufReader};
	{
		let program = command.as_std().get_program().to_str().unwrap();
		let args = command
			.as_std()
			.get_args()
			.map(|os| os.to_str().unwrap())
			.collect::<Vec<_>>()
			.join(" ");
		let dir = command.as_std().get_current_dir();
		println!("Executing \"{program} {args}\" in {dir:?}");
	}

	if !attach {
		println!("Detaching from process.");
		command
			.stdin(Stdio::null())
			.stdout(Stdio::null())
			.stderr(Stdio::null())
			.spawn()
			.context("failed to spawn process")?;
		return Ok(());
	}

	println!("Attaching console streams to execution...");

	let mut child = command
		.stdin(Stdio::null())
		.stdout(Stdio::piped())
		.stderr(Stdio::piped())
		.spawn()
		.context("failed to spawn process")?;
	let mut out_stream = BufReader::new(child.stdout.take().unwrap()).lines();
	let mut err_stream = BufReader::new(child.stderr.take().unwrap()).lines();

	let mut child = KillChildOnDrop(child);
	let out_handle: JoinHandle<anyhow::Result<()>> = tokio::task::spawn(async move {
		while let Some(line) = out_stream.next_line().await? {
			// would be better if we were using the `log` crate, but this is simpler for proof-of-concept
			// could look like: log::info!(target: "generate-project-files", "{line}");
			println!("{line}");
		}
		Ok(())
	});
	let err_handle: JoinHandle<anyhow::Result<()>> = tokio::task::spawn(async move {
		while let Some(line) = err_stream.next_line().await? {
			// would be better if we were using the `log` crate, but this is simpler for proof-of-concept
			// could look like: log::error!(target: "generate-project-files", "{line}");
			eprintln!("[ERROR] {line}");
		}
		Ok(())
	});

	let _status = child.0.wait().await?;
	out_handle.await??;
	err_handle.await??;
	Ok(())
}

// Wrapper for the child process to ensure that it kills the subprocess when dropped.
struct KillChildOnDrop(tokio::process::Child);
impl Drop for KillChildOnDrop {
	fn drop(&mut self) {
		// if the subprocess is complete, this will result in an error that we can ignore
		let _ = self.0.start_kill();
	}
}
