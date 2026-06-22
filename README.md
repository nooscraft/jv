# jv

A fast, Rust-based dependency resolver for Java and the JVM ecosystem.

`jv` brings the speed, global caching model, and developer experience of tools like [uv](https://github.com/astral-sh/uv) to the Java world — while remaining compatible with existing Maven and Gradle projects.

## Highlights

- **Fast dependency resolution** written in Rust
- **Global, filesystem-backed cache** — human-readable layout, shared across all your projects
- **Artifact downloading** with progress reporting
- **Deterministic TOML lockfiles** (`jv.lock`)
- Works with **existing projects** — no migration required
- Transitive resolution with conflict handling
- Support for Maven Central and custom repositories

## Motivation

Maven and Gradle are incredibly powerful, but their dependency resolution can be painfully slow on large or complex projects. `jv` applies the same performance principles that made uv revolutionary for Python to the Java ecosystem — without forcing you to abandon your existing build tools.

## Installation

jv is currently installed from source:

```bash
git clone https://github.com/nooscraft/jv
cd jv
cargo install --path .
```

Standalone binaries and easier installation methods are planned.

## Quick Start

Resolve dependencies for a project:

```bash
# Resolve the current directory
jv resolve .

# Or point at a specific file
jv resolve pom.xml
jv resolve build.gradle
```

This will analyze your build file, resolve transitive dependencies, and generate a `jv.lock` file.

## Current Capabilities

jv can already be used productively on real projects:

- Resolve Maven POMs (including parent inheritance and basic BOM support)
- Basic parsing of Gradle `dependencies {}` blocks
- Transitive dependency resolution with conflict handling
- Parallel downloads + aggressive local caching
- Generate deterministic `jv.lock` files

We are actively hardening the resolver on large, real-world Spring Boot and enterprise Java projects.

## Performance (Early Data)

`jv` is built around three performance pillars: parallel downloads, a global content-addressable cache, and an efficient resolver.

Early structured benchmarks on real multi-module Spring Boot projects show:

- Strong cache effectiveness on repeated runs (often 20–40% of POMs served from the global cache).
- Steady improvement in the number of resolved artifacts as BOM and profile handling gets deeper.
- On the primary test project, recent warm-cache runs resolve ~49 artifacts in ~60–70 seconds (with 40+ cache hits). No-cache runs are slower but still functional.

We are deliberately collecting real, reproducible data before making specific speed claims. Full methodology and ongoing results are available in [BENCHMARKS.md](./BENCHMARKS.md).

## Documentation

Documentation is still being written. For now, the best reference is:

```bash
jv --help
jv resolve --help
```

## Development Status

`jv` is under active development. Current focus areas include:

- Robust resolution on large, real-world Spring Boot and enterprise projects
- Deeper BOM and property handling
- Better tooling around lockfiles (`jv tree`, verification, etc.)
- Performance measurement and optimization

See the [GitHub Issues](https://github.com/nooscraft/jv/issues) for the latest roadmap.

## Contributing

Contributions, bug reports, and real-world project feedback are very welcome.

If you're testing jv on a large or complex Java codebase, please share your experience — it helps tremendously.

## Acknowledgments

jv is heavily inspired by [uv](https://github.com/astral-sh/uv) and the broader Rust tooling ecosystem.

Special thanks to the [Coursier](https://github.com/coursier/coursier) team for proving that fast, high-quality Java dependency resolution is both possible and extremely valuable.

## License

To be decided (likely Apache-2.0 / MIT dual license).

## Contributing

We welcome contributions, bug reports, and real-world usage feedback.

If you're testing jv on a large or complex Java project, please open an issue — real project feedback is extremely valuable.

## Acknowledgments

jv is heavily inspired by [uv](https://github.com/astral-sh/uv) and the Rust tooling ecosystem.

## License

To be decided (likely Apache-2.0 / MIT dual license).
