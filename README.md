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
  -d, --source-directory <SOURCE_DIRECTORY>  Path to source code
  -e, --source-extension <SOURCE_EXTENSION>  File extension for cpp files [default: cpp]
  -c, --compiler-executable <EXE>            Name of compiler executable [default: cl.exe]
  -h, --help                                 Print help
  -V, --version                              Print version
```

## Known Issues

Some files in the `msbuild.log` output do not contain the full path. To address
this, the entire directory tree `source-directory` is recursively explored
searching for all files with an extension matching `source-extension`. They are
added to an internal KV store to use as a lookup table when generating the final
entry for the `compile_commands.json` file. However, it's possible that
multiple files with the same name can exist in different directories. In this
case, it is unknown which directory is correct. Thus the entry is explicitly
left out in the generated output.
