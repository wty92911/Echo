use std::process::Command;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::configure()
        .out_dir("src/pb")
        .compile_protos(&["protos/echo.proto"], &["proto"])?;
    Command::new("cargo").args(["fmt"]).output().unwrap();
    Ok(())
}
