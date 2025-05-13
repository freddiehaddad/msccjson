use anyhow::{Context, Result};
use clap::Parser;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;
use std::{fs, fs::File};

#[derive(Deserialize, Serialize)]
struct CompileCommand {
    file: PathBuf,
    directory: PathBuf,
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

    /// Name of compiler executable
    #[arg(short('c'), long, name = "EXE", default_value = "cl.exe")]
    compiler_executable: String,
}

fn main() -> Result<()> {
    // Parse command line arguments
    let cli = Cli::parse();

    // File reader
    let input_file_handle = File::open(&cli.input_file).with_context(|| {
        format!("Failed to open {}", cli.input_file.to_string_lossy())
    })?;

    let input_file_handle = BufReader::new(input_file_handle);

    // File writer
    let output_file_handle = File::options()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&cli.output_file)
        .with_context(|| {
            format!("Failed to open {}", cli.output_file.to_string_lossy())
        })?;

    // Build directory tree
    anyhow::ensure!(
        cli.source_directory.is_dir(),
        format!(
            "Provided path is not a directory: {}",
            cli.source_directory.display()
        )
    );

    println!("Generating the lookup tree (this will take some time) ...");

    let tree = thread::scope(|s| {
        let (entry_tx, entry_rx) = mpsc::channel::<PathBuf>();
        let (error_tx, error_rx) = mpsc::channel();

        // Log error messages
        s.spawn(move || {
            while let Ok(e) = error_rx.recv() {
                eprintln!("{e}");
            }
        });

        // Process discovered files
        let h = s.spawn(move || {
            // Generate a map of files and their directories
            let mut tree: HashMap<PathBuf, PathBuf> = HashMap::new();
            while let Ok(path) = entry_rx.recv() {
                // Test if entry is a file with an extension
                if path.extension().is_some() {
                    let file_name = PathBuf::from(path.file_name().unwrap());
                    let parent = PathBuf::from(path.parent().unwrap());

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
            tree
        });

        // Traverse the directory tree
        s.spawn(move || {
            let mut stack = vec![cli.source_directory];
            while let Some(path) = stack.pop() {
                let reader = match fs::read_dir(&path) {
                    Ok(r) => r,
                    Err(e) => {
                        let e = format!("read_dir error for {path:?}: {e}");
                        let _ = error_tx.send(e);
                        continue;
                    }
                };
                for entry in reader {
                    let entry = match entry {
                        Ok(de) => de,
                        Err(e) => {
                            let e =
                                format!("Failed to read from {path:?}: {e}",);
                            let _ = error_tx.send(e);
                            continue;
                        }
                    };

                    let path = entry.path();
                    if path.is_dir() {
                        stack.push(path);
                        continue;
                    }

                    if path.is_file() {
                        let _ = entry_tx.send(path);
                    }
                }
            }
        });

        h.join().unwrap()
    });

    println!("Finished");

    thread::scope(|s| {
        let (source_tx, source_rx) = mpsc::channel();
        let (preprocess_tx, preprocess_rx) = mpsc::channel();
        let (token_tx, token_rx) = mpsc::channel();
        let (compile_command_tx, compile_command_rx) = mpsc::channel();
        let (error_tx, error_rx) = mpsc::channel();

        // Log error messages
        s.spawn(move || {
            while let Ok(e) = error_rx.recv() {
                eprintln!("{e}");
            }
        });

        // Collect all the compile commands from the input file
        s.spawn(move || {
            println!("Scanning the msbuild log (this will take some time) ...");
            input_file_handle
                .lines()
                .map_while(Result::ok)
                .for_each(|line| {
                    if line.to_lowercase().contains(&cli.compiler_executable) {
                        let _ = source_tx.send(line);
                    }
                });
        });

        // Remove nested quotes (")
        s.spawn(move || {
            while let Ok(s) = source_rx.recv() {
                let s = s.replace("\"", "");
                let _ = preprocess_tx.send(s);
            }
        });

        // Tokenize
        s.spawn(move || {
            while let Ok(s) = preprocess_rx.recv() {
                let t: Vec<_> =
                    s.split_whitespace().map(String::from).collect();
                let _ = token_tx.send(t);
            }
        });

        // Verify the input
        s.spawn(move || {
            println!("Generating the compile commands ...");
            let error_tx = error_tx.clone();
            while let Ok(t) = token_rx.recv() {
                let path = match t.last() {
                    Some(path) => Path::new(path),
                    None => {
                        let e = String::from("Token vector is empty!");
                        let _ = error_tx.send(e);
                        continue;
                    }
                };

                let file_name = match path.file_name() {
                    Some(file_name) => PathBuf::from(file_name),
                    None => {
                        let e = format!(
                            "Expected file name as last token in {t:?}"
                        );
                        let _ = error_tx.send(e);
                        continue;
                    }
                };

                if path.extension().is_none() {
                    let e = format!("Expected file extension in path {path:?}");
                    let _ = error_tx.send(e);
                    continue;
                };

                assert!(!file_name.to_string_lossy().is_empty());

                // Safe to unwrap because parent will return at least ""
                let mut parent = path.parent().unwrap();
                if parent.to_string_lossy().is_empty() {
                    parent = match tree.get(&file_name) {
                        Some(dir) => dir,
                        None => {
                            let e = format!("Path not found for {file_name:?}");
                            let _ = error_tx.send(e);
                            continue;
                        }
                    };
                }

                let cc = CompileCommand {
                    file: file_name,
                    directory: PathBuf::from(parent),
                    arguments: t,
                };

                let _ = compile_command_tx.send(cc);
            }
        });

        // Generate the compile_commands.json file
        s.spawn(move || {
            println!("Generating the compile_commands.json database ...");
            let compile_commands: Vec<_> = compile_command_rx.iter().collect();
            let _ = serde_json::to_writer_pretty(
                output_file_handle,
                &compile_commands,
            );
        });
    });

    println!("Finished!");
    Ok(())
}
