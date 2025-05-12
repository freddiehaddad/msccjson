# MSCCJSON

This is a project designed to generate a `compile_commands.json` database from
the `msbuild` output file (`msbuild.log`) generated during the compilation
process.

It works by reading the log file and collecting all the relevant compilation
commands from the MSVC compiler (`cl.exe`) and converting them into entries in
the following format:

```json
{
    "file": "main.cpp",
    "directory": "C:\\projects\\example",
    "arguments": [
        "cl.exe", "/EHsc", "/Zi", "/D", "DEBUG",
        "C:\\projects\\example\\main.cpp"
    ]
}
```

## Usage

See the help (`msccjson.exe --help`) for how to use the program.

```console
> msccjson.exe --help
Utility to generate a compile_commands.json file from msbuild.log.

Usage: msccjson.exe [OPTIONS] --input-file <INPUT_FILE>

Options:
  -i, --input-file <INPUT_FILE>              Path to msbuild.log
  -o, --output-file <OUTPUT_FILE>            Output JSON file [default: compile_commands.json]
  -d, --source-directory <SOURCE_DIRECTORY>  Path to source code [default: c:\projects\example]
  -e, --source-extension <SOURCE_EXTENSION>  File extension for cpp files [default: cpp]
  -c, --compiler-executable <EXE>            Name of compiler executable [default: cl.exe]
  -h, --help                                 Print help
  -V, --version                              Print version
```

## Missing Functionality

Some entries may be missing the path to the `.cpp` file. In this case, an error
message is printed to `stderr` and the entry is skipped. However, the goal is
to implement proper handling for such entries.
