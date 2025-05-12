use anyhow::{Context, Result};
use clap::Parser;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::io::{BufRead, BufReader, Write};
use std::{fs, fs::File};
use std::{path::Path, path::PathBuf};

#[derive(Deserialize, Serialize)]
struct CompileCommand {
    file: String,
    directory: String,
    arguments: Vec<String>,
}

#[derive(Parser)]
#[command(
    version,
    about = "Utility to generate a compile_commands.json file from msbuild.log."
)]
struct Cli {
    /// Path to msbuild.log
    #[arg(short('i'), long)]
    input_file: PathBuf,

    /// Output JSON file
    #[arg(short('o'), long, default_value = "compile_commands.json")]
    output_file: PathBuf,

    /// Path to source code
    #[arg(short('d'), long)]
    source_directory: PathBuf,

    /// File extension for cpp files
    #[arg(short('e'), long, default_value = "cpp")]
    source_extension: PathBuf,

    /// Name of compiler executable
    #[arg(short('c'), long, name = "EXE", default_value = "cl.exe")]
    compiler_executable: String,
}

/// Returns all lines from `handle` that contain the substring `pattern`.
fn filter_compile_commands(
    handle: BufReader<File>,
    filter: String,
) -> Vec<String> {
    handle
        .lines()
        .map_while(Result::ok)
        .filter_map(|line| {
            if line.to_lowercase().contains(&filter) {
                Some(line.replace("\"", ""))
            } else {
                None
            }
        })
        .collect()
}

/// Converts a vector of compile commands into a CompileCommand.
fn generate_entries(
    compile_commands: Vec<String>,
    directory_tree: HashMap<String, String>,
) -> Result<Vec<CompileCommand>> {
    let mut entries = Vec::new();
    for compile_command in &compile_commands {
        let arguments: Vec<_> = compile_command
            .split_whitespace()
            .map(String::from)
            .collect();

        // We expect a proper path (can be relative) as the last line in the
        // cl.exe compile command.
        //
        // Example:
        //   S:\Azure\Storage\XStore\src\base\PlatformConfig\lib\vdsutils.cpp
        let target_cpp_file =
            Path::new(arguments.iter().last().ok_or_else(|| {
                anyhow::anyhow!("Unexpected input: {:?}", arguments)
            })?);

        // The file field of the compile_commands.json entry
        // vdsutils.cpp
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

        // The directory field of the compile_commands.json entry
        //
        // Example:
        //   S:\Azure\Storage\XStore\src\base\PlatformConfig\lib\
        let directory = match target_cpp_file.parent() {
            Some(parent) => parent.display().to_string(),
            None => {
                eprintln!("{}: parent unknown", target_cpp_file.display());
                String::new()
            }
        };

        // Check the directory tree if path is not part of the compile command
        let directory = match directory.is_empty() {
            false => directory,
            true => match directory_tree.get(&file_name) {
                Some(dir) => {
                    // An empty value indicates duplicate files names; skip
                    if dir.is_empty() {
                        eprintln!(
                            "{}: no entry found in tree",
                            target_cpp_file.display()
                        );
                        continue;
                    } else {
                        dir.clone()
                    }
                }
                None => {
                    eprintln!(
                        "{}: duplicate entries found in tree",
                        target_cpp_file.display()
                    );
                    continue;
                }
            },
        };

        assert!(!directory.is_empty());

        // Construct and add the entry
        entries.push(CompileCommand {
            file: file_name,
            directory,
            arguments,
        });
    }
    Ok(entries)
}

/// Explores the entire directory tree starting from `dir` adding any files with
/// `extension` to the `tree` as the key and the parent path of the file as the
/// value.  This lookup table is used for adding the directory entry to the
/// compile_commands.json file where its not specified on the command line.
///
/// Because files with matching names can exist in multiple directories, these
/// cases result in the value entry in the tree being set to the empty string
/// since we cannot know which path is the correct one.
fn find_files(
    dir: &Path,
    extension: &Path,
    tree: &mut HashMap<String, String>,
) -> Result<()> {
    // Iterate over each file in dir
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        // Further explore any directory
        if path.is_dir() {
            find_files(&path, extension, tree)?;
            continue;
        }

        // Test if non-directory entry is a file with a matchin extension
        if let Some(ext) = path.extension() {
            if ext.len() != 3 || ext.to_ascii_lowercase() != extension {
                continue;
            }

            let file_name =
                String::from(path.file_name().unwrap().to_string_lossy());
            let parent = String::from(path.parent().unwrap().to_string_lossy());

            // Add KV pair (file/path) to the hash table; clear on collision
            match tree.entry(file_name) {
                Entry::Vacant(e) => {
                    e.insert(parent);
                }
                Entry::Occupied(mut e) => {
                    e.get_mut().clear();
                }
            };
        }
    }
    Ok(())
}

fn main() -> Result<()> {
    // Parse command line arguments
    let cli = Cli::parse();

    // Get the command line arguments
    let input_file = cli.input_file;
    let output_file = cli.output_file;
    let compiler_executable = cli.compiler_executable;
    let source_directory = cli.source_directory;
    let source_extension = cli.source_extension;

    // File reader
    let input_file_handle = File::open(&input_file).with_context(|| {
        format!("Failed to open {}", input_file.to_string_lossy())
    })?;

    let input_file_handle = BufReader::new(input_file_handle);

    // File writer
    let output_file_handle = File::options()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&output_file)
        .with_context(|| {
            format!("Failed to open {}", output_file.to_string_lossy())
        })?;

    // Build directory tree
    anyhow::ensure!(
        source_directory.is_dir(),
        format!(
            "Provided path is not a directory: {}",
            source_directory.display()
        )
    );

    // Generate a map of files and their directories
    let mut source_tree: HashMap<String, String> = HashMap::new();
    find_files(&source_directory, &source_extension, &mut source_tree)?;

    // Collect all the compile commands from the input file
    let compile_commands: Vec<_> =
        filter_compile_commands(input_file_handle, compiler_executable);

    println!("Found {} compile commands", compile_commands.len());

    // Tokenize the compile commands
    let compile_commands = generate_entries(compile_commands, source_tree)?;

    // Generate the compile_commands.json file
    let serialized = serde_json::to_string_pretty(&compile_commands)?;
    let _ = writeln!(&output_file_handle, "{serialized}");
    Ok(())
}
