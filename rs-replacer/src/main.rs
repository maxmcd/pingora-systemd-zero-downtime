use nix::sys::signal::{self, Signal};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::{env, thread, time::Duration};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Set up signal handling
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    signal_hook::flag::register(signal_hook::consts::SIGQUIT, Arc::clone(&running))?;

    // Get our own executable path
    let current_exe = env::current_exe()?;

    println!("Starting manager...");
    sd_notify::notify(false, &[sd_notify::NotifyState::Ready])?;

    // Start initial child
    let mut child = Command::new(&current_exe).arg("--worker").spawn()?;

    println!("Started worker process: {}", child.id());

    // Main management loop
    while running.load(Ordering::Relaxed) {
        // Check if child is still running
        match child.try_wait()? {
            Some(status) => {
                println!("Child {} exited with status {}", child.id(), status);
                break;
            }
            None => {
                thread::sleep(Duration::from_secs(1));
            }
        }
    }

    if !running.load(Ordering::Relaxed) {
        println!("SIGQUIT received, starting replacement...");

        // Start new child
        let new_child = Command::new(&current_exe).arg("--worker").spawn()?;

        println!("Started new worker: {}", new_child.id());

        // Signal old child to hand over and exit
        if let Ok(()) = signal::kill(
            nix::unistd::Pid::from_raw(child.id() as i32),
            Signal::SIGQUIT,
        ) {
            println!("Sent SIGQUIT to old worker {}", child.id());
        }

        // Wait for old child to exit
        let status = child.wait()?;
        println!("Old worker exited: {}", status);

        // Replace our child handle
        child = new_child;
    }

    // Wait for final child to exit
    let status = child.wait()?;
    println!("Final worker exited: {}", status);

    Ok(())
}
