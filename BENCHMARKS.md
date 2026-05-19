# jv Benchmarks

This document contains performance measurements comparing `jv` against traditional Java build tools (Maven and Gradle).

> **Note**: This document is currently a work in progress. Real-world benchmarks are being collected on large, multi-module projects (including Spring Boot applications).

## Goals

- Measure end-to-end dependency resolution time
- Measure behavior with cold cache vs warm/global cache
- Compare against `mvn dependency:resolve` / `mvn dependency:go-offline`
- Compare against equivalent Gradle commands
- Provide transparent methodology and raw data

## Test Projects

We are currently running benchmarks against:

- [micro-server-own](https://github.com/Blucezhang/micro-server-own) — A multi-module Spring Boot / Spring Cloud project
- Additional real-world Spring Boot and enterprise Java projects (to be added)

## Methodology

- All tests run on the same machine
- `jv` is tested using the release binary (`target/release/jv`)
- Maven and Gradle are tested with their standard dependency resolution commands
- Both cold cache and warm cache scenarios are measured where applicable
- Multiple runs are averaged to reduce noise

## Current Status

Benchmarks are actively being collected. Early internal results on real Spring Boot projects have been promising, particularly on repeated runs thanks to jv's global cache.

Detailed numbers and comparisons will be published here once we have a sufficient set of reproducible measurements.

## Contributing Benchmarks

If you have a large or interesting Java project and would like to contribute benchmark data, please open an issue.

## License

This document is part of the jv project and is licensed under the same terms as the project.
