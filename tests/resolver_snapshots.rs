//! Snapshot tests for the transitive resolver.
//!
//! These tests use `insta` to catch regressions in resolution behavior,
//! lockfile output, and conflict/exclusion handling.

use jv::models::{MavenCoordinate, Version};
use jv::parser::Pom;
use jv::resolver::{resolve_transitive, ResolveOptions};
use std::path::PathBuf;
use tempfile::NamedTempFile;

fn write_temp_pom(xml: &str) -> PathBuf {
    // Use a non-deleting temp file so the path remains valid after this function returns
    let tmp = tempfile::Builder::new()
        .prefix("jv-test-pom-")
        .suffix(".xml")
        .tempfile_in(std::env::temp_dir())
        .unwrap();
    let path = tmp.path().to_path_buf();
    std::fs::write(&path, xml).unwrap();
    // Leak the file handle — the OS will clean it up eventually
    std::mem::forget(tmp);
    path
}

#[tokio::test]
async fn resolves_simple_two_dependency_pom() {
    let pom = r#"
        <project>
            <groupId>com.example</groupId>
            <artifactId>test</artifactId>
            <version>1.0</version>
            <dependencies>
                <dependency>
                    <groupId>org.apache.commons</groupId>
                    <artifactId>commons-lang3</artifactId>
                    <version>3.14.0</version>
                </dependency>
            </dependencies>
        </project>
    "#;

    let path = write_temp_pom(pom);
    let resolution = resolve_transitive(&path, ResolveOptions::default())
        .await
        .expect("resolution should succeed");

    // We snapshot the final list of resolved GAVs (sorted) for stability
    let mut coords: Vec<_> = resolution
        .dependencies
        .iter()
        .map(|d| d.coordinate.to_string())
        .collect();
    coords.sort();

    insta::assert_json_snapshot!("simple_two_deps", coords);
}

#[tokio::test]
async fn handles_basic_exclusions() {
    // A tiny synthetic case where we exclude a transitive dep
    let pom = r#"
        <project>
            <groupId>com.example</groupId>
            <artifactId>test-excl</artifactId>
            <version>1.0</version>
            <dependencies>
                <dependency>
                    <groupId>com.google.guava</groupId>
                    <artifactId>guava</artifactId>
                    <version>33.2.1-jre</version>
                    <exclusions>
                        <exclusion>
                            <groupId>com.google.code.findbugs</groupId>
                            <artifactId>jsr305</artifactId>
                        </exclusion>
                    </exclusions>
                </dependency>
            </dependencies>
        </project>
    "#;

    let path = write_temp_pom(pom);
    let resolution = resolve_transitive(&path, ResolveOptions::default())
        .await
        .expect("resolution should succeed");

    let has_jsr305 = resolution
        .dependencies
        .iter()
        .any(|d| d.coordinate.artifact_id == "jsr305");

    assert!(!has_jsr305, "jsr305 should have been excluded");
}
