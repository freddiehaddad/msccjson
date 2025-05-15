# MSCCJSON

This is a tool designed to generate a `compile_commands.json` database from the
`msbuild` output file (`msbuild.log`) generated during the compilation process.

It works by reading the log file and collecting all the relevant compilation
commands from the MSVC compiler (`cl.exe`) and converting them into entries in
the following format:

```json
{
    "file": "main.cpp",
    "directory": "C:\\projects\\example",
    "arguments": [
        "cl.exe", "/EHsc", "/Zi", "/D", "DEBUG", "C:\\projects\\example\\main.cpp"
    ]
}
```

## Usage

See the help (`msccjson.exe --help`) for how to use the program.

```console
$ msccjson.exe --help
Utility to generate a compile_commands.json file from msbuild.log.

Usage: msccjson.exe [OPTIONS] --input-file <INPUT_FILE> --source-directory <SOURCE_DIRECTORY>

Options:
  -i, --input-file <INPUT_FILE>              Path to msbuild.log
  -o, --output-file <OUTPUT_FILE>            Output JSON file [default: compile_commands.json]
  -d, --source-directory <SOURCE_DIRECTORY>  Path to source code
  -c, --compiler-executable <EXE>            Name of compiler executable [default: cl.exe]
  -h, --help                                 Print help
  -V, --version                              Print version
```

## Known Issues

Some commands in the `msbuild.log` output do not include the full path to the
source file. To address this, the entire directory tree `source-directory` is
recursively explored, and all files are added to an internal KV store that gets
used as a lookup table when generating `compile_commands.json` entries. However,
it's possible that multiple files with the same name can exist in different
directories. In this case, it is unknown which directory is correct. Thus, the
entry is explicitly left out in the generated output.

Consider the following scenario illustrating the problem of not knowing which
`widget.cpp` is being referenced:

```console
.
+-- bar
|   \-- widget.cpp
+-- foo
|   \-- widget.cpp
\-- main.cpp
```
