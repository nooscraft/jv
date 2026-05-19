
Feasibility assessment for a Rust-based Java dependency resolver:

## High feasibility — with important caveats

### Why it's feasible

1. Proven pattern: `uv` shows Rust can deliver 10–100x speedups for dependency resolution. The same approach applies to Java.
2. Technical advantages:
   - Fast dependency resolution algorithms
   - Parallel downloads and processing
   - Efficient caching
   - Lower memory usage
   - Native performance

3. Java ecosystem needs:
   - Maven/Gradle can be slow on large projects
   - Build times are a common pain point
   - Large dependency graphs benefit from fast resolution

### Major challenges

1. Ecosystem complexity:
   - Maven POMs with inheritance, profiles, and plugins
   - Gradle’s DSL and build scripts
   - Multiple repository formats (Maven Central, Ivy, custom)
   - Transitive dependency resolution with conflict resolution

2. Build system integration:
   - Maven and Gradle are full build systems, not just dependency resolvers
   - You’d need to either:
     - Replace them entirely (large scope)
     - Integrate as a resolver plugin (more feasible)
     - Create a new build tool (like Bazel, but Rust-based)

3. Compatibility requirements:
   - Must understand Maven coordinates (groupId:artifactId:version)
   - Support Maven repositories (HTTP/HTTPS)
   - Handle version ranges, exclusions, optional dependencies
   - Support multi-module projects
   - Understand Java classpath semantics

4. Adoption barriers:
   - Enterprise Java teams are conservative
   - Existing tooling integration (IDEs, CI/CD)
   - Migration path from existing builds

### Recommended approach

1. Start as a dependency resolver (not a full build system):
   - Focus on fast dependency resolution
   - Generate lock files (like `Cargo.lock` or `uv.lock`)
   - Can be used by Gradle/Maven as a plugin initially

2. Phased rollout:
   - Phase 1: Standalone dependency resolver (like `uv pip compile`)
   - Phase 2: Gradle/Maven plugin integration
   - Phase 3: Full build system (if desired)

3. Key features to implement:
   - Maven repository protocol support
   - POM parsing and dependency extraction
   - Conflict resolution algorithms
   - Parallel artifact downloading
   - Smart caching (similar to uv's approach)
   - Lock file generation

### Market opportunity

- Large Java codebases with slow builds
- CI/CD pipelines where speed matters
- Teams already using Rust (polyglot environments)
- New projects looking for modern tooling

### Similar projects to study

- `coursier` (Scala-based, fast Maven resolver) — shows demand
- `sbt` (Scala build tool) — demonstrates Rust could work
- `bazel` (build system) — shows appetite for fast build tools

### Bottom line

Feasibility: 8/10 — technically very feasible  
Market fit: 7/10 — demand exists, but adoption may be gradual  
Complexity: 9/10 — Java ecosystem is complex, but manageable

Recommendation: Start with a dependency resolver that can integrate with existing tools, similar to how `uv` started as a `pip` replacement before expanding. This reduces risk and provides a clearer migration path.

Should I outline a technical architecture or implementation plan?