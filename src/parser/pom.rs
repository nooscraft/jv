//! Maven POM parser using quick-xml with serde.
//!
//! This parser focuses on the subset needed for dependency resolution. Full
//! fidelity to every Maven POM feature is intentionally deferred.

use crate::error::{JvError, Result};
use crate::models::{Dependency, Exclusion, MavenCoordinate, Scope, Version};
use quick_xml::de::from_str;
use serde::Deserialize;
use std::collections::HashMap;
use std::str::FromStr;

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Project {
    #[serde(rename = "groupId")]
    pub group_id: Option<String>,
    #[serde(rename = "artifactId")]
    pub artifact_id: Option<String>,
    pub version: Option<String>,
    pub packaging: Option<String>,
    pub parent: Option<Parent>,
    pub properties: Option<Properties>,
    pub dependencies: Option<Dependencies>,
    #[serde(rename = "dependencyManagement")]
    pub dependency_management: Option<DependencyManagement>,
    pub modules: Option<Modules>,
    pub name: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Parent {
    #[serde(rename = "groupId")]
    pub group_id: String,
    #[serde(rename = "artifactId")]
    pub artifact_id: String,
    pub version: String,
    pub relative_path: Option<String>,
}

/// Robust representation of <properties> that never fails the whole POM parse.
///
/// Maven <properties> blocks in the wild can contain:
/// - Simple string values (most common)
/// - Nested XML elements (rare but seen in some parent POMs)
/// - Empty elements
///
/// We deserialize everything into strings for interpolation purposes.
#[derive(Debug, Clone, Default)]
pub struct Properties {
    pub entries: HashMap<String, String>,
}

impl<'de> Deserialize<'de> for Properties {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum PropertyValue {
            Text(String),
            // Anything else (nested XML, empty element with attributes, etc.)
            // We treat as empty string for interpolation purposes.
            Other(serde::de::IgnoredAny),
        }

        let raw: HashMap<String, PropertyValue> = HashMap::deserialize(deserializer)?;

        let entries = raw
            .into_iter()
            .map(|(k, v)| {
                let value = match v {
                    PropertyValue::Text(s) => s,
                    PropertyValue::Other(_) => String::new(),
                };
                (k, value)
            })
            .collect();

        Ok(Properties { entries })
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Dependencies {
    #[serde(rename = "dependency", default)]
    pub entries: Vec<DependencyXml>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct DependencyXml {
    #[serde(rename = "groupId")]
    pub group_id: Option<String>,
    #[serde(rename = "artifactId")]
    pub artifact_id: Option<String>,
    pub version: Option<String>,
    pub scope: Option<String>,
    pub optional: Option<String>,
    pub classifier: Option<String>,
    #[serde(rename = "type")]
    pub r#type: Option<String>,
    pub exclusions: Option<ExclusionsXml>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ExclusionsXml {
    #[serde(rename = "exclusion", default)]
    pub entries: Vec<ExclusionXml>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ExclusionXml {
    #[serde(rename = "groupId")]
    pub group_id: Option<String>,
    #[serde(rename = "artifactId")]
    pub artifact_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct DependencyManagement {
    pub dependencies: Option<Dependencies>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Modules {
    #[serde(rename = "module", default)]
    pub entries: Vec<String>,
}

/// High-level representation of a parsed POM ready for the resolver.
#[derive(Debug, Clone)]
pub struct Pom {
    pub coordinate: MavenCoordinate,
    pub parent: Option<MavenCoordinate>,
    pub packaging: String,
    pub properties: HashMap<String, String>,
    pub dependencies: Vec<Dependency>,
    pub dependency_management: Vec<Dependency>,
    pub modules: Vec<String>,
}

impl Pom {
    /// Parse a raw POM XML string into our high-level model.
    pub fn parse(xml: &str) -> Result<Self> {
        let project: Project = from_str(xml).map_err(|e| JvError::PomParse {
            coord: "unknown".to_string(),
            reason: e.to_string(),
        })?;

        Self::from_project(project)
    }

    fn from_project(project: Project) -> Result<Self> {
        let group_id = project
            .group_id
            .or_else(|| project.parent.as_ref().map(|p| p.group_id.clone()))
            .ok_or_else(|| JvError::PomParse {
                coord: "unknown".to_string(),
                reason: "missing groupId (and no parent to inherit from)".to_string(),
            })?;

        let artifact_id = project.artifact_id.ok_or_else(|| JvError::PomParse {
            coord: "unknown".to_string(),
            reason: "missing artifactId".to_string(),
        })?;

        let version = project
            .version
            .or_else(|| project.parent.as_ref().map(|p| p.version.clone()))
            .ok_or_else(|| JvError::PomParse {
                coord: "unknown".to_string(),
                reason: "missing version (and no parent to inherit from)".to_string(),
            })?;

        let coordinate = MavenCoordinate::new(
            group_id,
            artifact_id,
            Version::parse(&version).unwrap_or_else(|_| Version::new(version)),
        );

        let parent = project.parent.map(|p| {
            MavenCoordinate::new(
                p.group_id,
                p.artifact_id,
                Version::parse(&p.version).unwrap_or_else(|_| Version::new(p.version)),
            )
        });

        let properties = project
            .properties
            .map(|p| p.entries)
            .unwrap_or_default();

        let dependencies = project
            .dependencies
            .map(|d| Self::convert_dependencies(d.entries, &properties))
            .unwrap_or_default();

        let dependency_management = project
            .dependency_management
            .and_then(|dm| dm.dependencies)
            .map(|d| Self::convert_dependencies(d.entries, &properties))
            .unwrap_or_default();

        let modules = project
            .modules
            .map(|m| m.entries)
            .unwrap_or_default();

        Ok(Self {
            coordinate,
            parent,
            packaging: project.packaging.unwrap_or_else(|| "jar".to_string()),
            properties,
            dependencies,
            dependency_management,
            modules,
        })
    }

    fn convert_dependencies(xml_deps: Vec<DependencyXml>, _props: &HashMap<String, String>) -> Vec<Dependency> {
        xml_deps
            .into_iter()
            .filter_map(|d| {
                let gid = d.group_id?;
                let aid = d.artifact_id?;
                // Version may be missing if it comes from dependencyManagement
                let ver = d.version.unwrap_or_else(|| "managed".to_string());

                let version = Version::parse(&ver).unwrap_or_else(|_| Version::new(ver));

                let scope = d
                    .scope
                    .as_deref()
                    .and_then(|s| Scope::from_str(s).ok())
                    .unwrap_or(Scope::Compile);

                let optional = d
                    .optional
                    .as_deref()
                    .map(|o| o.eq_ignore_ascii_case("true"))
                    .unwrap_or(false);

                let exclusions = d
                    .exclusions
                    .map(|exs| {
                        exs.entries
                            .into_iter()
                            .filter_map(|e| {
                                Some(Exclusion {
                                    group_id: e.group_id?,
                                    artifact_id: e.artifact_id?,
                                })
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                Some(Dependency {
                    coordinate: MavenCoordinate::new(gid, aid, version),
                    scope,
                    optional,
                    exclusions,
                    classifier: d.classifier,
                    r#type: d.r#type,
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIMPLE_POM: &str = r#"
        <project>
            <groupId>com.example</groupId>
            <artifactId>demo</artifactId>
            <version>1.0.0</version>
            <dependencies>
                <dependency>
                    <groupId>org.apache.commons</groupId>
                    <artifactId>commons-lang3</artifactId>
                    <version>3.14.0</version>
                </dependency>
            </dependencies>
        </project>
    "#;

    #[test]
    fn parses_simple_pom() {
        let pom = Pom::parse(SIMPLE_POM).expect("should parse");
        assert_eq!(pom.coordinate.group_id, "com.example");
        assert_eq!(pom.coordinate.artifact_id, "demo");
        assert_eq!(pom.dependencies.len(), 1);
        assert_eq!(pom.dependencies[0].coordinate.group_id, "org.apache.commons");
    }

    /// Regression test: some real Apache / Google parent POMs contain
    /// <properties> blocks that are not pure string maps. The parser must
    /// never blow up on them.
    #[tokio::test]
    async fn parses_real_world_poms_from_maven_central() {
        let client = crate::repository::RepositoryClient::new();

        let test_coords = [
            // These were known to trigger the old "map vs string" error
            ("org.apache.commons", "commons-parent", "64"),
            ("org.apache.commons", "commons-lang3", "3.14.0"),
            ("com.google.guava", "guava", "33.2.1-jre"),
        ];

        for (g, a, v) in test_coords {
            let coord = MavenCoordinate::new(g, a, Version::parse(v).unwrap());
            let xml = client
                .fetch_pom(&coord)
                .await
                .unwrap_or_else(|_| panic!("failed to fetch {g}:{a}:{v}"));

            let pom = Pom::parse(&xml);
            assert!(
                pom.is_ok(),
                "should parse real POM {g}:{a}:{v} without crashing on <properties> or parent"
            );
            let pom = pom.unwrap();
            assert_eq!(pom.coordinate.artifact_id, a);
        }
    }

    #[test]
    fn pubgrub_smoke_test_passes() {
        assert!(crate::resolver::pubgrub::smoke_test_pubgrub_compatibility());
    }
}
