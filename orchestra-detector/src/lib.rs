//! Stack detection for `orchestra-detector`.
//!
//! `detect_stack(path)` inspects indicator files in a codebase root and returns
//! the primary language, framework, and project category. Checks are ordered by
//! specificity: language-specific manifest files take priority over generic ones.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::fs;

use orchestra_core::types::ProjectType;
use thiserror::Error;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Confidence level of a detected stack.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Confidence {
    /// Definitive indicator file with content match.
    High,
    /// Indicator file present but no framework match.
    Medium,
}

/// A detected technology stack for a codebase.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetectedStack {
    /// Primary programming language (e.g. `"Rust"`, `"TypeScript"`).
    pub primary_language: String,
    /// Framework or runtime, if identified (e.g. `"Next.js"`, `"Laravel"`).
    pub framework: Option<String>,
    /// Inferred Orchestra project category.
    pub project_type: ProjectType,
    /// Detection confidence.
    pub confidence: Confidence,
}

/// Errors from stack detection.
#[derive(Debug, Error)]
pub enum DetectError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("failed to parse {path}: {message}")]
    ParseError { path: PathBuf, message: String },

    #[error("could not determine stack for '{path}' — no known indicator file found")]
    UnknownStack { path: PathBuf },
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Detect the technology stack of the codebase at `path`.
///
/// Checks indicator files in priority order. Returns `DetectError::UnknownStack`
/// if no known stack can be inferred.
pub fn detect_stack(path: &Path) -> Result<DetectedStack, DetectError> {
    // Priority: specific manifests first, generic (package.json, requirements) last.
    if let Some(s) = detect_php(path)? { return Ok(s); }
    if let Some(s) = detect_dart_flutter(path)? { return Ok(s); }
    if let Some(s) = detect_rust_crate(path)? { return Ok(s); }
    if let Some(s) = detect_go(path)? { return Ok(s); }
    if let Some(s) = detect_elixir(path)? { return Ok(s); }
    if let Some(s) = detect_jvm(path)? { return Ok(s); }
    if let Some(s) = detect_dotnet(path)? { return Ok(s); }
    if let Some(s) = detect_ruby(path)? { return Ok(s); }
    if let Some(s) = detect_swift(path)? { return Ok(s); }
    if let Some(s) = detect_javascript(path)? { return Ok(s); }
    if let Some(s) = detect_python(path)? { return Ok(s); }

    Err(DetectError::UnknownStack { path: path.to_path_buf() })
}

// ---------------------------------------------------------------------------
// Language detectors
// ---------------------------------------------------------------------------

fn detect_php(path: &Path) -> Result<Option<DetectedStack>, DetectError> {
    let file = path.join("composer.json");
    if !file.exists() { return Ok(None); }
    let content = fs::read_to_string(&file)?;
    let lower = content.to_lowercase();

    let framework = if lower.contains("laravel/framework") {
        Some("Laravel")
    } else if lower.contains("symfony/symfony") || lower.contains("symfony/framework-bundle") {
        Some("Symfony")
    } else if lower.contains("roots/sage") || lower.contains("johnpbloch/wordpress") {
        Some("WordPress")
    } else if lower.contains("slim/slim") {
        Some("Slim")
    } else if lower.contains("cakephp/cakephp") {
        Some("CakePHP")
    } else if lower.contains("codeigniter4/framework") {
        Some("CodeIgniter")
    } else {
        None
    };

    Ok(Some(DetectedStack {
        primary_language: "PHP".to_string(),
        framework: framework.map(str::to_string),
        project_type: ProjectType::Backend,
        confidence: if framework.is_some() { Confidence::High } else { Confidence::Medium },
    }))
}

fn detect_dart_flutter(path: &Path) -> Result<Option<DetectedStack>, DetectError> {
    let file = path.join("pubspec.yaml");
    if !file.exists() { return Ok(None); }
    let content = fs::read_to_string(&file)?;

    let is_flutter = content.contains("flutter:") && content.contains("sdk: flutter");
    Ok(Some(DetectedStack {
        primary_language: "Dart".to_string(),
        framework: if is_flutter { Some("Flutter".to_string()) } else { None },
        project_type: if is_flutter { ProjectType::Mobile } else { ProjectType::Backend },
        confidence: if is_flutter { Confidence::High } else { Confidence::Medium },
    }))
}

fn detect_rust_crate(path: &Path) -> Result<Option<DetectedStack>, DetectError> {
    let file = path.join("Cargo.toml");
    if !file.exists() { return Ok(None); }
    let content = fs::read_to_string(&file)?;
    let lower = content.to_lowercase();

    let framework = if lower.contains("actix-web") || lower.contains("actix_web") {
        Some("Actix Web")
    } else if lower.contains("\"axum\"") || lower.contains("axum =") {
        Some("Axum")
    } else if lower.contains("tauri") {
        Some("Tauri")
    } else if lower.contains("leptos") {
        Some("Leptos")
    } else if lower.contains("rocket") {
        Some("Rocket")
    } else if lower.contains("warp") {
        Some("Warp")
    } else {
        None
    };

    let project_type = match framework {
        Some("Tauri") => ProjectType::Frontend,
        Some("Leptos") => ProjectType::Frontend,
        _ => ProjectType::Backend,
    };

    Ok(Some(DetectedStack {
        primary_language: "Rust".to_string(),
        framework: framework.map(str::to_string),
        project_type,
        confidence: if framework.is_some() { Confidence::High } else { Confidence::Medium },
    }))
}

fn detect_go(path: &Path) -> Result<Option<DetectedStack>, DetectError> {
    let file = path.join("go.mod");
    if !file.exists() { return Ok(None); }
    let content = fs::read_to_string(&file)?;
    let lower = content.to_lowercase();

    let framework = if lower.contains("gin-gonic/gin") {
        Some("Gin")
    } else if lower.contains("labstack/echo") {
        Some("Echo")
    } else if lower.contains("gofiber/fiber") {
        Some("Fiber")
    } else if lower.contains("beego/beego") || lower.contains("astaxie/beego") {
        Some("Beego")
    } else if lower.contains("go-chi/chi") {
        Some("Chi")
    } else {
        None
    };

    Ok(Some(DetectedStack {
        primary_language: "Go".to_string(),
        framework: framework.map(str::to_string),
        project_type: ProjectType::Backend,
        confidence: if framework.is_some() { Confidence::High } else { Confidence::Medium },
    }))
}

fn detect_elixir(path: &Path) -> Result<Option<DetectedStack>, DetectError> {
    let file = path.join("mix.exs");
    if !file.exists() { return Ok(None); }
    let content = fs::read_to_string(&file)?;
    let lower = content.to_lowercase();

    let framework = if lower.contains(":phoenix") || lower.contains("\"phoenix\"") {
        Some("Phoenix")
    } else {
        None
    };

    Ok(Some(DetectedStack {
        primary_language: "Elixir".to_string(),
        framework: framework.map(str::to_string),
        project_type: ProjectType::Backend,
        confidence: if framework.is_some() { Confidence::High } else { Confidence::Medium },
    }))
}

fn detect_jvm(path: &Path) -> Result<Option<DetectedStack>, DetectError> {
    let gradle = path.join("build.gradle");
    let gradle_kts = path.join("build.gradle.kts");
    let pom = path.join("pom.xml");

    let (content, is_kotlin) = if gradle_kts.exists() {
        (fs::read_to_string(&gradle_kts)?, true)
    } else if gradle.exists() {
        (fs::read_to_string(&gradle)?, false)
    } else if pom.exists() {
        (fs::read_to_string(&pom)?, false)
    } else {
        return Ok(None);
    };

    let lower = content.to_lowercase();
    let framework = if lower.contains("spring-boot") || lower.contains("springframework") {
        Some("Spring Boot")
    } else if lower.contains("quarkus") {
        Some("Quarkus")
    } else if lower.contains("micronaut") {
        Some("Micronaut")
    } else {
        None
    };

    let language = if is_kotlin { "Kotlin" } else { "Java" };
    Ok(Some(DetectedStack {
        primary_language: language.to_string(),
        framework: framework.map(str::to_string),
        project_type: ProjectType::Backend,
        confidence: if framework.is_some() { Confidence::High } else { Confidence::Medium },
    }))
}

fn detect_dotnet(path: &Path) -> Result<Option<DetectedStack>, DetectError> {
    // Look for any *.csproj or *.sln file
    let found = fs::read_dir(path)?
        .filter_map(|e| e.ok())
        .any(|e| {
            let name = e.file_name();
            let s = name.to_string_lossy();
            s.ends_with(".csproj") || s.ends_with(".sln") || s.ends_with(".fsproj")
        });
    if !found { return Ok(None); }

    // Read all .csproj files to detect framework
    let csproj = fs::read_dir(path)?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().ends_with(".csproj"))
        .filter_map(|e| fs::read_to_string(e.path()).ok())
        .collect::<Vec<_>>()
        .join("\n");

    let lower = csproj.to_lowercase();
    let framework = if lower.contains("microsoft.aspnetcore") {
        Some("ASP.NET Core")
    } else if lower.contains("maui") {
        Some("MAUI")
    } else if lower.contains("blazor") || lower.contains("microsoft.aspnetcore.components") {
        Some("Blazor")
    } else {
        None
    };

    let project_type = match framework {
        Some("MAUI") => ProjectType::Mobile,
        Some("Blazor") => ProjectType::Frontend,
        _ => ProjectType::Backend,
    };

    Ok(Some(DetectedStack {
        primary_language: "C#".to_string(),
        framework: framework.map(str::to_string),
        project_type,
        confidence: if framework.is_some() { Confidence::High } else { Confidence::Medium },
    }))
}

fn detect_ruby(path: &Path) -> Result<Option<DetectedStack>, DetectError> {
    let file = path.join("Gemfile");
    if !file.exists() { return Ok(None); }
    let content = fs::read_to_string(&file)?;
    let lower = content.to_lowercase();

    let framework = if lower.contains("\"rails\"") || lower.contains("'rails'") {
        Some("Rails")
    } else if lower.contains("sinatra") {
        Some("Sinatra")
    } else if lower.contains("hanami") {
        Some("Hanami")
    } else {
        None
    };

    Ok(Some(DetectedStack {
        primary_language: "Ruby".to_string(),
        framework: framework.map(str::to_string),
        project_type: ProjectType::Backend,
        confidence: if framework.is_some() { Confidence::High } else { Confidence::Medium },
    }))
}

fn detect_swift(path: &Path) -> Result<Option<DetectedStack>, DetectError> {
    let spm = path.join("Package.swift");
    let xcodeproj = fs::read_dir(path)
        .ok()
        .and_then(|mut d| d.find(|e| {
            e.as_ref().ok().map(|e| e.file_name().to_string_lossy().ends_with(".xcodeproj")).unwrap_or(false)
        }));

    if !spm.exists() && xcodeproj.is_none() { return Ok(None); }

    let content = if spm.exists() { fs::read_to_string(&spm)? } else { String::new() };
    let lower = content.to_lowercase();

    let framework = if lower.contains("swiftui") {
        Some("SwiftUI")
    } else if lower.contains("vapor") {
        Some("Vapor")
    } else {
        None
    };

    let project_type = match framework {
        Some("Vapor") => ProjectType::Backend,
        _ => ProjectType::Mobile,
    };

    Ok(Some(DetectedStack {
        primary_language: "Swift".to_string(),
        framework: framework.map(str::to_string),
        project_type,
        confidence: Confidence::Medium,
    }))
}

fn detect_javascript(path: &Path) -> Result<Option<DetectedStack>, DetectError> {
    let file = path.join("package.json");
    if !file.exists() { return Ok(None); }
    let content = fs::read_to_string(&file)?;

    let json: serde_json::Value = serde_json::from_str(&content).map_err(|e| {
        DetectError::ParseError { path: file.clone(), message: e.to_string() }
    })?;

    let deps = collect_package_json_deps(&json);
    let is_typescript = path.join("tsconfig.json").exists() || deps.contains("typescript");
    let language = if is_typescript { "TypeScript" } else { "JavaScript" };

    // Framework detection — precedence from most specific to most generic
    let framework = if deps.contains("next") {
        Some(("Next.js", ProjectType::Frontend))
    } else if deps.contains("nuxt") || deps.contains("nuxt3") {
        Some(("Nuxt", ProjectType::Frontend))
    } else if deps.contains("@remix-run/react") || deps.contains("@remix-run/node") {
        Some(("Remix", ProjectType::Frontend))
    } else if deps.contains("astro") {
        Some(("Astro", ProjectType::Frontend))
    } else if deps.contains("gatsby") {
        Some(("Gatsby", ProjectType::Frontend))
    } else if deps.contains("@angular/core") {
        Some(("Angular", ProjectType::Frontend))
    } else if deps.contains("@sveltejs/kit") {
        Some(("SvelteKit", ProjectType::Frontend))
    } else if deps.contains("svelte") {
        Some(("Svelte", ProjectType::Frontend))
    } else if deps.contains("vue") {
        Some(("Vue", ProjectType::Frontend))
    } else if deps.contains("react") {
        Some(("React", ProjectType::Frontend))
    } else if deps.contains("@nestjs/core") {
        Some(("NestJS", ProjectType::Backend))
    } else if deps.contains("express") {
        Some(("Express", ProjectType::Backend))
    } else if deps.contains("fastify") {
        Some(("Fastify", ProjectType::Backend))
    } else if deps.contains("koa") {
        Some(("Koa", ProjectType::Backend))
    } else if deps.contains("hapi") || deps.contains("@hapi/hapi") {
        Some(("Hapi", ProjectType::Backend))
    } else {
        None
    };

    let (fw_name, project_type) = match framework {
        Some((name, ref pt)) => (Some(name.to_string()), pt.clone()),
        None => (None, ProjectType::Backend),
    };

    Ok(Some(DetectedStack {
        primary_language: language.to_string(),
        framework: fw_name,
        project_type,
        confidence: if framework.is_some() { Confidence::High } else { Confidence::Medium },
    }))
}

fn detect_python(path: &Path) -> Result<Option<DetectedStack>, DetectError> {
    let req_txt = path.join("requirements.txt");
    let pyproject = path.join("pyproject.toml");
    let setup_py = path.join("setup.py");
    let pipfile = path.join("Pipfile");

    let content = if req_txt.exists() {
        fs::read_to_string(&req_txt)?
    } else if pyproject.exists() {
        fs::read_to_string(&pyproject)?
    } else if pipfile.exists() {
        fs::read_to_string(&pipfile)?
    } else if setup_py.exists() {
        fs::read_to_string(&setup_py)?
    } else {
        return Ok(None);
    };

    let lower = content.to_lowercase();

    let (framework, project_type) = if lower.contains("fastapi") {
        (Some("FastAPI"), ProjectType::Backend)
    } else if lower.contains("django") {
        (Some("Django"), ProjectType::Backend)
    } else if lower.contains("flask") {
        (Some("Flask"), ProjectType::Backend)
    } else if lower.contains("starlette") {
        (Some("Starlette"), ProjectType::Backend)
    } else if lower.contains("litestar") || lower.contains("litestar-api") {
        (Some("Litestar"), ProjectType::Backend)
    } else if lower.contains("streamlit") {
        (Some("Streamlit"), ProjectType::Ml)
    } else if lower.contains("gradio") {
        (Some("Gradio"), ProjectType::Ml)
    } else if lower.contains("torch") || lower.contains("pytorch") {
        (Some("PyTorch"), ProjectType::Ml)
    } else if lower.contains("tensorflow") || lower.contains("keras") {
        (Some("TensorFlow"), ProjectType::Ml)
    } else if lower.contains("scikit-learn") || lower.contains("sklearn") {
        (Some("scikit-learn"), ProjectType::Ml)
    } else if lower.contains("pandas") || lower.contains("numpy") {
        (Some("Data Science"), ProjectType::Ml)
    } else {
        (None, ProjectType::Backend)
    };

    Ok(Some(DetectedStack {
        primary_language: "Python".to_string(),
        framework: framework.map(str::to_string),
        project_type,
        confidence: if framework.is_some() { Confidence::High } else { Confidence::Medium },
    }))
}

// ---------------------------------------------------------------------------
// Utilities
// ---------------------------------------------------------------------------

fn collect_package_json_deps(json: &serde_json::Value) -> HashSet<String> {
    let mut deps = HashSet::new();
    for key in &["dependencies", "devDependencies", "peerDependencies"] {
        if let Some(obj) = json.get(key).and_then(|v| v.as_object()) {
            for k in obj.keys() {
                deps.insert(k.clone());
            }
        }
    }
    deps
}
