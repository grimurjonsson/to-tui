use anyhow::{Context, Result, anyhow};
use std::process::Command;

pub fn check_command_exists(command: &str) -> Result<(), String> {
    let check = if cfg!(windows) {
        Command::new("where").arg(command).output()
    } else {
        Command::new("which").arg(command).output()
    };

    match check {
        Ok(output) if output.status.success() => Ok(()),
        _ => Err(format!("'{command}' not found in PATH")),
    }
}

pub fn run_command(command: &str, args: &[&str]) -> Result<String> {
    let output = Command::new(command)
        .args(args)
        .output()
        .with_context(|| format!("Failed to execute '{command}'"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!(
            "'{}' failed with exit code {:?}: {}",
            command,
            output.status.code(),
            stderr.trim()
        ));
    }

    let stdout = String::from_utf8(output.stdout)
        .with_context(|| format!("Invalid UTF-8 output from '{command}'"))?;

    Ok(stdout)
}
