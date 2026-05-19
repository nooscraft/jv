# jv Benchmarks

This document contains performance measurements comparing `jv` against traditional Java build tools (Maven and Gradle).

> **Note**: This document is currently a work in progress. Real-world benchmarks are being collected on large, multi-module projects (including Spring Boot applications).

## Goals

- Measure end-to-end dependency resolution time
- Measure behavior with cold cache vs warm/global cache
- Compare against Maven and Gradle using fair, reproducible commands
- Provide transparent methodology and raw data

## Test Projects

Current projects under test:

- [micro-server-own](https://github.com/Blucezhang/micro-server-own) — Multi-module Spring Boot / Spring Cloud project (primary test case)
- Additional real-world Spring Boot and enterprise Java projects (planned)

## Benchmark Script

We use a structured benchmark runner located at:

```bash
./scripts/benchmark.sh <project-path> [mode]
```

Supported modes:
- `warm` (default) — Use existing caches
- `cold` — Clear relevant caches before running
- `no-cache` — Run jv with `--no-cache`

Example:
```bash
./scripts/benchmark.sh /path/to/project warm
```

The script automatically captures:
- Date and time
- Operating system and architecture
- Java version
- Maven version (if available)
- jv git commit
- Wall-clock timing for each tool

## Methodology

- All measurements are run on the same machine
- `jv` is tested using the release binary when available (`target/release/jv`)
- Maven comparison uses `mvn dependency:go-offline -B -q`
- Both cold-cache and warm-cache scenarios are supported
- Results are intended to be copied into this file or `results/`

## Current Status

Structured benchmarking framework is now in place (`scripts/benchmark.sh` + `results/` directory).

### First Structured Run (2026-05-19)

**Project**: `micro-server-own` (multi-module Spring Boot 1.5 project)

**Environment**:
- OS: macOS arm64 (Darwin)
- Java: 21.0.2
- Maven: Not available in PATH
- jv: commit `b801336` (release build)

**Mode**: `warm`

**Result**:
- jv resolved **3 artifacts** (processed 5 POMs)
- Time: **0.56 seconds**
- Many dependencies still unresolved due to `${...}` property placeholders (Lombok, internal modules, Thymeleaf extras, etc.)

**Notes**:
- This run highlights current limitations in property resolution on older Spring Boot setups.
- As resolver quality improves (especially BOM + property handling), we expect both the number of resolved artifacts and the value of these benchmarks to increase significantly.

Full raw output is saved in `results/micro-server-own_20260519_152335.txt`.

## Example Output

```
▶ Environment
Date:             2026-05-19 15:22:06
OS / Arch:        Darwin / arm64
Java:             21.0.2
Maven:            Maven not found
jv (git):         4ebec50

▶ Running jv benchmark
[INFO] jv resolve completed in 0.97s
```

## Contributing

If you would like to contribute benchmark data from your own projects, please open an issue with the output from the benchmark script.

## License

This document is part of the jv project and follows the same license.
