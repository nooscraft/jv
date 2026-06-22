//! Core data models for the jv Java dependency resolver.
//!
//! This module defines the fundamental types used throughout the resolver:
//! coordinates, versions, dependencies, scopes, and artifacts.

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::fmt;
use std::str::FromStr;

/// Maven coordinate in the standard `groupId:artifactId:version` form.
/// Also known as GAV coordinates.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MavenCoordinate {
    pub group_id: String,
    pub artifact_id: String,
    pub version: Version,
}

impl MavenCoordinate {
    pub fn new(
        group_id: impl Into<String>,
        artifact_id: impl Into<String>,
        version: Version,
    ) -> Self {
        Self {
            group_id: group_id.into(),
            artifact_id: artifact_id.into(),
            version,
        }
    }

    /// Returns the standard Maven path fragment for this coordinate (without version).
    /// e.g. "com/google/guava"
    pub fn path(&self) -> String {
        format!("{}/{}", self.group_id.replace('.', "/"), self.artifact_id)
    }

    /// Returns the full Maven repository path for a specific file.
    pub fn repository_path(&self, extension: &str, classifier: Option<&str>) -> String {
        let base = format!(
            "{}/{}/{}/{}",
            self.path(),
            self.version,
            self.artifact_id,
            self.version
        );
        match classifier {
            Some(c) if !c.is_empty() => format!("{}-{}{}", base, c, extension),
            _ => format!("{}{}", base, extension),
        }
    }
}

impl fmt::Display for MavenCoordinate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}:{}", self.group_id, self.artifact_id, self.version)
    }
}

/// Maven dependency scope. Determines which classpaths a dependency participates in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    #[default]
    Compile,
    Runtime,
    Test,
    Provided,
    System,
    /// Special scope used in <dependencyManagement> to import dependencyManagement from another POM.
    Import,
}

impl Scope {
    /// Returns true if this scope contributes to the compile classpath.
    pub fn is_compile(&self) -> bool {
        matches!(self, Scope::Compile | Scope::Provided | Scope::System)
    }

    /// Returns true if this scope contributes to the runtime classpath.
    pub fn is_runtime(&self) -> bool {
        matches!(self, Scope::Compile | Scope::Runtime)
    }

    /// Returns true if this scope should be included in the final resolved set for a normal build.
    pub fn is_transitive(&self) -> bool {
        !matches!(self, Scope::Test | Scope::Provided | Scope::System)
    }
}

impl FromStr for Scope {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "compile" | "" => Ok(Scope::Compile),
            "runtime" => Ok(Scope::Runtime),
            "test" => Ok(Scope::Test),
            "provided" => Ok(Scope::Provided),
            "system" => Ok(Scope::System),
            "import" => Ok(Scope::Import),
            other => Err(format!("unknown scope: {other}")),
        }
    }
}

impl fmt::Display for Scope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Scope::Compile => write!(f, "compile"),
            Scope::Runtime => write!(f, "runtime"),
            Scope::Test => write!(f, "test"),
            Scope::Provided => write!(f, "provided"),
            Scope::System => write!(f, "system"),
            Scope::Import => write!(f, "import"),
        }
    }
}

/// Represents an exclusion declared on a dependency.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Exclusion {
    pub group_id: String,
    pub artifact_id: String,
}

impl Exclusion {
    pub fn matches(&self, group: &str, artifact: &str) -> bool {
        (self.group_id == "*" || self.group_id == group)
            && (self.artifact_id == "*" || self.artifact_id == artifact)
    }
}

/// A single dependency declaration as it appears in a POM or build file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dependency {
    pub coordinate: MavenCoordinate,
    pub scope: Scope,
    pub optional: bool,
    pub exclusions: Vec<Exclusion>,
    /// Classifier (e.g., "sources", "javadoc", or custom)
    pub classifier: Option<String>,
    /// Type / packaging (jar, pom, war, etc.)
    pub r#type: Option<String>,
}

impl Dependency {
    pub fn new(group_id: &str, artifact_id: &str, version: &str) -> Self {
        Self {
            coordinate: MavenCoordinate::new(
                group_id,
                artifact_id,
                Version::parse(version).unwrap_or_else(|_| Version::new(version)),
            ),
            scope: Scope::default(),
            optional: false,
            exclusions: vec![],
            classifier: None,
            r#type: None,
        }
    }

    pub fn with_scope(mut self, scope: Scope) -> Self {
        self.scope = scope;
        self
    }
}

/// An artifact that can be downloaded from a Maven repository.
/// Typically a JAR, but can also be POM, sources JAR, etc.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Artifact {
    pub coordinate: MavenCoordinate,
    pub classifier: Option<String>,
    pub extension: String, // "jar", "pom", "war", etc.
    /// SHA-1 checksum if known from metadata
    pub sha1: Option<String>,
}

impl Artifact {
    pub fn jar(coordinate: MavenCoordinate) -> Self {
        Self {
            coordinate,
            classifier: None,
            extension: "jar".to_string(),
            sha1: None,
        }
    }

    pub fn pom(coordinate: MavenCoordinate) -> Self {
        Self {
            coordinate,
            classifier: None,
            extension: "pom".to_string(),
            sha1: None,
        }
    }
}

/// Parsed and comparable Maven version.
///
/// Maven versions follow a specific ordering that is *not* semantic versioning.
/// Examples: 1.0-alpha < 1.0-beta < 1.0-rc1 < 1.0 < 1.0.1 < 1.1
///
/// This implementation provides a faithful ordering based on Apache Maven's
/// ComparableVersion algorithm (simplified but sufficient for most cases).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Version {
    /// Original string representation (preserved for serialization and display)
    pub raw: String,
    /// Pre-parsed comparable representation (list of tokens)
    #[serde(skip)]
    comparable: Vec<VersionToken>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum VersionToken {
    Number(u64),
    String(String),
    /// Special qualifier ordering: alpha < beta < milestone < rc < snapshot < "" < sp
    Qualifier(Qualifier),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Qualifier {
    Alpha,
    Beta,
    Milestone,
    Rc,
    Snapshot,
    Release, // the empty qualifier, treated as highest before service packs
    Sp,
}

impl Version {
    /// Create a new Version from a raw string, performing best-effort parsing.
    pub fn new(raw: impl Into<String>) -> Self {
        let raw = raw.into();
        let comparable = Self::parse_comparable(&raw);
        Self { raw, comparable }
    }

    /// Parse a version string, returning error on completely invalid input.
    pub fn parse(raw: &str) -> Result<Self, String> {
        if raw.trim().is_empty() {
            return Err("version string cannot be empty".to_string());
        }
        Ok(Self::new(raw))
    }

    fn parse_comparable(raw: &str) -> Vec<VersionToken> {
        // Simplified but effective Maven version tokenizer.
        // Splits on '.' and '-' and interprets numbers vs strings.
        let mut tokens = Vec::new();
        let mut current = String::new();
        let mut chars = raw.chars().peekable();

        while let Some(&ch) = chars.peek() {
            match ch {
                '.' | '-' | '_' => {
                    if !current.is_empty() {
                        tokens.push(Self::classify_token(&current));
                        current.clear();
                    }
                    chars.next();
                }
                _ => {
                    current.push(ch);
                    chars.next();
                }
            }
        }
        if !current.is_empty() {
            tokens.push(Self::classify_token(&current));
        }

        // Normalize: treat missing parts gracefully
        if tokens.is_empty() {
            tokens.push(VersionToken::Number(0));
        }

        tokens
    }

    fn classify_token(s: &str) -> VersionToken {
        if let Ok(n) = s.parse::<u64>() {
            return VersionToken::Number(n);
        }

        let lower = s.to_lowercase();
        match lower.as_str() {
            "alpha" | "a" => VersionToken::Qualifier(Qualifier::Alpha),
            "beta" | "b" => VersionToken::Qualifier(Qualifier::Beta),
            "milestone" | "m" => VersionToken::Qualifier(Qualifier::Milestone),
            "rc" | "cr" => VersionToken::Qualifier(Qualifier::Rc),
            "snapshot" | "snap" => VersionToken::Qualifier(Qualifier::Snapshot),
            "sp" | "servicepack" => VersionToken::Qualifier(Qualifier::Sp),
            "final" | "ga" | "release" => VersionToken::Qualifier(Qualifier::Release),
            _ => {
                // Try to extract leading number, e.g. "1alpha" or just keep as string
                if let Some(first_digit) = s.find(|c: char| c.is_ascii_digit()) {
                    if first_digit > 0 {
                        // Split numeric prefix
                        let num_part: String = s[..first_digit].to_string();
                        if let Ok(_n) = num_part.parse::<u64>() {
                            // For simplicity we just treat the whole as string for now
                        }
                    }
                }
                VersionToken::String(lower)
            }
        }
    }
}

impl FromStr for Version {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.raw)
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        // Compare token by token, padding the shorter with zeros / release
        let a = &self.comparable;
        let b = &other.comparable;
        let len = a.len().max(b.len());

        for i in 0..len {
            let ta = a.get(i).cloned().unwrap_or(VersionToken::Number(0));
            let tb = b.get(i).cloned().unwrap_or(VersionToken::Number(0));

            let ord = match (ta, tb) {
                (VersionToken::Number(na), VersionToken::Number(nb)) => na.cmp(&nb),
                (VersionToken::Qualifier(qa), VersionToken::Qualifier(qb)) => qa.cmp(&qb),
                // Qualifier < Number/String for practical Maven ordering
                (VersionToken::Qualifier(_), VersionToken::Number(_)) => Ordering::Less,
                (VersionToken::Number(_), VersionToken::Qualifier(_)) => Ordering::Greater,
                (VersionToken::String(sa), VersionToken::String(sb)) => sa.cmp(&sb),
                (VersionToken::String(_), VersionToken::Number(n)) => {
                    if n == 0 {
                        Ordering::Greater
                    } else {
                        Ordering::Less
                    }
                }
                (VersionToken::Number(n), VersionToken::String(_)) => {
                    if n == 0 {
                        Ordering::Less
                    } else {
                        Ordering::Greater
                    }
                }
                (VersionToken::Qualifier(_), VersionToken::String(_)) => Ordering::Less,
                (VersionToken::String(_), VersionToken::Qualifier(_)) => Ordering::Greater,
            };

            if ord != Ordering::Equal {
                return ord;
            }
        }
        Ordering::Equal
    }
}

/// Version range specification as used in Maven POMs.
///
/// Supported forms (initial implementation):
/// - "1.0"                 -> exact version
/// - "[1.0,2.0]"           -> inclusive range
/// - "[1.0,2.0)"           -> half-open
/// - "(,2.0]" , "[1.0,)"   -> open ended
/// - "1.+"                 -> prefix (Gradle style, treated as [1.0,2.0) for major)
///
/// Full Maven range support will be expanded iteratively.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersionRange {
    pub raw: String,
    /// Lower bound (inclusive if true)
    pub lower: Option<(Version, bool)>,
    /// Upper bound (inclusive if true)
    pub upper: Option<(Version, bool)>,
    /// Exact version (when no range syntax used)
    pub exact: Option<Version>,
}

impl VersionRange {
    pub fn parse(spec: &str) -> Result<Self, String> {
        let spec = spec.trim();
        if spec.is_empty() {
            return Err("empty version range".to_string());
        }

        // Exact version (no special chars)
        if !spec.contains(|c: char| "()[],+".contains(c)) {
            let v = Version::parse(spec)?;
            return Ok(Self {
                raw: spec.to_string(),
                lower: None,
                upper: None,
                exact: Some(v),
            });
        }

        // Handle prefix style "1.+"
        if let Some(prefix) = spec.strip_suffix(".+") {
            // Treat as [prefix, next major)
            let lower_v = Version::parse(prefix)?;
            // Very naive "next major" - real impl needs more care
            let upper_v = Version::parse(&format!(
                "{}.99999",
                prefix.split('.').next().unwrap_or("0")
            ))?;
            return Ok(Self {
                raw: spec.to_string(),
                lower: Some((lower_v, true)),
                upper: Some((upper_v, false)),
                exact: None,
            });
        }

        // Range form
        let is_open_lower = spec.starts_with('(');
        let is_open_upper = spec.ends_with(')');
        let is_closed_lower = spec.starts_with('[');
        let is_closed_upper = spec.ends_with(']');

        if !(is_open_lower || is_closed_lower) || !(is_open_upper || is_closed_upper) {
            return Err(format!("invalid version range syntax: {spec}"));
        }

        let inner = &spec[1..spec.len() - 1];
        let parts: Vec<&str> = inner.split(',').collect();

        let (lower, upper) = match parts.as_slice() {
            [lo, hi] => {
                let l = if lo.trim().is_empty() {
                    None
                } else {
                    Some((Version::parse(lo.trim())?, is_closed_lower))
                };
                let u = if hi.trim().is_empty() {
                    None
                } else {
                    Some((Version::parse(hi.trim())?, is_closed_upper))
                };
                (l, u)
            }
            [single] if !single.trim().is_empty() => {
                let v = Version::parse(single.trim())?;
                (Some((v.clone(), true)), Some((v, true)))
            }
            _ => return Err(format!("malformed range: {spec}")),
        };

        Ok(Self {
            raw: spec.to_string(),
            lower,
            upper,
            exact: None,
        })
    }

    /// Returns true if the given version satisfies this range.
    pub fn contains(&self, version: &Version) -> bool {
        if let Some(ex) = &self.exact {
            return ex == version;
        }

        let lower_ok = match &self.lower {
            Some((lo, inclusive)) => {
                if *inclusive {
                    version >= lo
                } else {
                    version > lo
                }
            }
            None => true,
        };

        let upper_ok = match &self.upper {
            Some((hi, inclusive)) => {
                if *inclusive {
                    version <= hi
                } else {
                    version < hi
                }
            }
            None => true,
        };

        lower_ok && upper_ok
    }

    /// Returns a human friendly description.
    pub fn description(&self) -> String {
        self.raw.clone()
    }
}

impl FromStr for VersionRange {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl fmt::Display for VersionRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.raw)
    }
}

/// A resolved dependency entry that appears in the lock file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedDependency {
    pub coordinate: MavenCoordinate,
    pub scope: Scope,
    pub optional: bool,
    /// The direct parent dependency that pulled this in (for conflict debugging)
    pub depended_by: Option<MavenCoordinate>,
    /// All artifacts associated (usually the main jar + pom)
    pub artifacts: Vec<Artifact>,
}

impl ResolvedDependency {
    pub fn new(coordinate: MavenCoordinate, scope: Scope) -> Self {
        Self {
            coordinate,
            scope,
            optional: false,
            depended_by: None,
            artifacts: vec![],
        }
    }
}
