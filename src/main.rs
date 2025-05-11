use anyhow::{Context, Result};
use clap::Parser;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::{env, path::Path, path::PathBuf};

#[derive(Deserialize, Serialize)]
struct CompileCommand {
    file: String,
    directory: String,
    arguments: Vec<String>,
}

/// Get the path to the executable for the CLI default value.
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

/// Returns all lines from `handle` that contain the substring `pattern`.
fn filter_compile_commands(handle: BufReader<File>, filter: String) -> Vec<String> {
    handle
        .lines()
        .map_while(Result::ok)
        .filter_map(|line| {
            if line.to_lowercase().contains(&filter) {
                Some(String::from(line.trim()))
            } else {
                None
            }
        })
        .collect()
}

/// Extract the target cpp file name (i.e. main.cpp) from the compile commands.
fn get_target_cpp_file(arguments: &[String]) -> Result<&String> {
    let target_cpp_file = arguments
        .iter()
        .last()
        .ok_or_else(|| anyhow::anyhow!("Unexpected input: {:?}", arguments))?;
    Ok(target_cpp_file)
}

/// Converts a vector of compile commands into a CompileCommand.
fn generate_entries(compile_commands: Vec<String>) -> Result<Vec<CompileCommand>> {
    let mut entries = Vec::new();
    for compile_command in &compile_commands {
        let arguments: Vec<_> = compile_command
            .split_whitespace()
            .map(String::from)
            .collect();

        let target_cpp_file = Path::new(get_target_cpp_file(&arguments)?);
        let directory = match target_cpp_file.parent() {
            Some(parent) => parent.display().to_string(),
            None => {
                eprintln!(
                    "Unable to extract parent component: {}",
                    target_cpp_file.display()
                );
                continue;
            }
        };

        if directory.is_empty() {
            eprintln!("Parent component is empty: {}", target_cpp_file.display());
            continue;
        }

        let file_name = match target_cpp_file.file_name() {
            Some(file_name) => file_name.to_string_lossy().to_string(),
            None => {
                eprintln!(
                    "Unable to extract file_name component: {}",
                    target_cpp_file.display()
                );
                continue;
            }
        };

        if file_name.is_empty() {
            eprintln!(
                "File name component is empty: {}",
                target_cpp_file.display()
            );
            continue;
        }

        entries.push(CompileCommand {
            file: file_name,
            directory,
            arguments,
        });
    }
    Ok(entries)
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
    let compile_commands: Vec<_> = filter_compile_commands(input_file_handle, compiler_executable);

    println!("Found {} compile commands", compile_commands.len());

    // Tokenize the compile commands
    let compile_commands = generate_entries(compile_commands)?;

    // Generate the compile_commands.json file
    let serialized = serde_json::to_string_pretty(&compile_commands)?;
    let _ = writeln!(&output_file_handle, "{serialized}");
    Ok(())
}
