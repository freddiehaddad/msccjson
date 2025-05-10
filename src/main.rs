use anyhow::{Context, Result};
use clap::Parser;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::{env, path::PathBuf, process};

#[derive(Deserialize, Serialize)]
struct CompileCommand {
    file: String,
    directory: String,
    arguments: Vec<String>,
}

fn get_default_input_path(file_name: &str) -> PathBuf {
    let mut path = env::current_dir().unwrap();
    path.push(file_name);
    path
}

#[derive(Parser)]
#[command(
    version,
    about = "Utility to generate a compile_commands.json file from msbuild.log output for XStore."
)]
struct Cli {
    /// Path to msbuild.log
    #[arg(short, long, default_value = get_default_input_path("msbuild.log").into_os_string())]
    input_file: PathBuf,

    /// Output JSON file
    #[arg(short, long, default_value = get_default_input_path("compile_commands.json").into_os_string())]
    output_file: PathBuf,

    /// Name of compiler executable
    #[arg(short, long, name="EXE", default_value_t = String::from("cl.exe"))]
    compiler_executable: String,
}

fn main() -> Result<()> {
    // Parse command line arguments
    let cli = Cli::parse();

    // Get the command line arguments
    let input_file = cli.input_file;
    let output_file = cli.output_file;
    let compiler_executable = cli.compiler_executable;

    // File reader
    let input_file_handle = File::open(&input_file)
        .with_context(|| format!("Failed to open {}", input_file.to_string_lossy()))?;

    let input_file_handle = BufReader::new(input_file_handle);

    // File writer
    let output_file_handle = File::options()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&output_file)
        .with_context(|| format!("Failed to open {}", output_file.to_string_lossy()))?;

    // Collect all the compile commands from the input file
    let compile_commands: Vec<_> = input_file_handle
        .lines()
        .map_while(Result::ok)
        .filter_map(|line| {
            if line.to_lowercase().contains(&compiler_executable) {
                Some(String::from(line.trim()))
            } else {
                None
            }
        })
        .collect();

    println!("Found {} compile commands", compile_commands.len());

    // Tokenize the compile commands
    let compile_commands: Vec<_> = compile_commands
        .iter()
        .map(|compile_command| {
            let arguments: Vec<_> = compile_command
                .split_whitespace()
                .map(String::from)
                .collect();

            let target_cpp_file = match arguments.iter().last() {
                Some(target_cpp_file) => target_cpp_file,
                None => {
                    eprintln!("Input not as expected: {arguments:?}");
                    process::exit(-1);
                }
            };
            let target_cpp_file = std::path::PathBuf::from(target_cpp_file);
            let file_name = match target_cpp_file.file_name() {
                Some(file_name) => file_name.to_string_lossy().to_string(),
                None => {
                    let error_message = format!(
                        "Failed to extract file name from {}",
                        target_cpp_file.to_string_lossy()
                    );
                    eprintln!("{error_message}");
                    process::exit(-1);
                }
            };
            let directory = match target_cpp_file.parent() {
                Some(directory) => directory.display().to_string(),
                None => {
                    let error_message = format!(
                        "Failed to extract parent from {}",
                        target_cpp_file.to_string_lossy()
                    );
                    eprintln!("{error_message}");
                    process::exit(-1);
                }
            };

            CompileCommand {
                file: file_name,
                directory,
                arguments,
            }
        })
        .collect();

    // Generate the compile_commands.json file
    let serialized = match serde_json::to_string_pretty(&compile_commands) {
        Ok(serialized) => serialized,
        Err(e) => {
            let error_message = format!("Failed to serialize compile commands: {e}");
            eprintln!("{error_message}");
            process::exit(-1);
        }
    };

    let _ = writeln!(&output_file_handle, "{serialized}");
    Ok(())
}
