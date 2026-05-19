//! Very basic Gradle build file parser (declarative `dependencies {}` only).
//!
//! This is intentionally minimal for Phase 1. It handles the most common
//! patterns people use in real `build.gradle` / `build.gradle.kts` files:
//!
//! - implementation 'g:a:v'
//! - api "g:a:v"
//! - testImplementation 'g:a:v'
//!
//! Full Groovy/Kotlin DSL execution is out of scope (and would defeat the
//! performance goal). For complex builds we recommend using Gradle's own
//! `dependencies` task output or generating a lockfile.

use crate::models::{Dependency, MavenCoordinate, Scope, Version};
use regex::Regex;
use std::str::FromStr;

/// Extremely lightweight Gradle dependency extractor.
pub fn parse_build_gradle(content: &str) -> Vec<Dependency> {
    let mut deps = Vec::new();

    // Match common dependency declarations
    // Examples:
    //   implementation 'com.google.guava:guava:33.2.1-jre'
    //   api "org.apache.commons:commons-lang3:3.14.0"
    //   testImplementation("junit:junit:4.13.2")
    let re = Regex::new(
        r#"(?x)
        (implementation|api|compile|runtimeOnly|testImplementation|testRuntimeOnly)
        \s*[\(\s]?
        ['"]([^:'"]+):([^:'"]+):([^:'"]+)['"]
        [\)\s]?
    "#,
    )
    .expect("regex is valid");

    for cap in re.captures_iter(content) {
        let config = &cap[1];
        let group = &cap[2];
        let artifact = &cap[3];
        let version = &cap[4];

        let scope = match config {
            "testImplementation" | "testRuntimeOnly" => Scope::Test,
            _ => Scope::Compile,
        };

        if let Ok(ver) = Version::parse(version) {
            let mut dep = Dependency::new(group, artifact, "");
            dep.coordinate.version = ver;
            dep.scope = scope;
            deps.push(dep);
        }
    }

    deps
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_common_gradle_dependency_notation() {
        let build = r#"
            dependencies {
                implementation 'com.google.guava:guava:33.2.1-jre'
                api "org.apache.commons:commons-lang3:3.14.0"
                testImplementation('junit:junit:4.13.2')
            }
        "#;

        let deps = parse_build_gradle(build);
        assert_eq!(deps.len(), 3);

        assert_eq!(deps[0].coordinate.group_id, "com.google.guava");
        assert_eq!(deps[0].scope, Scope::Compile);

        assert_eq!(deps[2].coordinate.artifact_id, "junit");
        assert_eq!(deps[2].scope, Scope::Test);
    }
}
