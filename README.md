# jv

A fast, Rust-based dependency resolver for Java and the JVM ecosystem.

**jv** is an attempt to bring the speed, caching model, and developer experience of tools like [uv](https://github.com/astral-sh/uv) to the Java world, while remaining compatible with existing Maven and Gradle projects.

> **Status**: Active development. Core resolution, caching, and lockfile generation are working and being hardened on real-world projects.

## Highlights

- **Fast dependency resolution** for Maven and Gradle projects, written in Rust
- **Global, content-addressable cache** — share dependencies across all your Java projects
- **Parallel artifact downloading** with progress reporting
- **Deterministic TOML lockfiles** (`jv.lock`)
- Works with **existing projects** — point it at any `pom.xml` or `build.gradle`
- Transitive dependency resolution with conflict handling
- Support for Maven Central and custom repositories

## Motivation

Maven and Gradle are powerful, but their dependency resolution can be painfully slow on large or complex projects. `jv` applies the lessons from high-performance tools like uv and Cargo to make the resolution step dramatically faster while remaining compatible with the existing Java ecosystem.

## Current Capabilities

jv can already be used on real projects:

- Resolve dependencies from `pom.xml` (including parent inheritance and basic BOM support)
- Basic support for declarative Gradle `dependencies {}` blocks
- Transitive resolution with nearest-wins conflict handling
- Parallel downloads + aggressive local caching
- Generate `jv.lock` (TOML) lockfiles

Performance work and deeper compatibility (especially with complex Spring Boot / multi-module projects) is ongoing.

## Installation

jv is currently built from source:

```bash
git clone https://github.com/nooscraft/jv
cd jv
cargo install --path .
```

Standalone installers and pre-built binaries are planned.

## Quick Start

Resolve dependencies for a project:

```bash
# From inside a project directory
jv resolve .

# Or point directly at a file
jv resolve pom.xml
jv resolve build.gradle
```

Generate (or update) a lockfile:

```bash
jv resolve .
```

View dependency tree (coming soon):

```bash
jv tree
```

## Documentation

Detailed documentation is still being written. For now, use `jv --help` or `jv resolve --help`.

## Development Status

jv is under active development. The focus right now is:

- Making resolution robust on large, real-world Spring Boot and enterprise Java projects
- Improving cache effectiveness and performance
- Adding better tooling around lockfiles (`jv tree`, verification, etc.)

See the [GitHub Issues](https://github.com/nooscraft/jv/issues) for the current roadmap.

## Contributing

We welcome contributions, bug reports, and real-world usage feedback.

If you're testing jv on a large or complex Java project, please open an issue — real project feedback is extremely valuable.

## Acknowledgments

jv is heavily inspired by [uv](https://github.com/astral-sh/uv) and the Rust tooling ecosystem.

## License

To be decided (likely Apache-2.0 / MIT dual license).
