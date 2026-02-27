//! Parameterised stack detection tests for `orchestra-detector`.
//!
//! Each `#[case]` gets an isolated `TempDir` — no shared state.

use orchestra_core::types::ProjectType;
use orchestra_detector::{detect_stack, Confidence};
use rstest::rstest;
use std::fs;
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// Helper
// ---------------------------------------------------------------------------

fn make_dir() -> TempDir {
    TempDir::new().expect("tempdir")
}

fn write(dir: &TempDir, filename: &str, content: &str) {
    fs::write(dir.path().join(filename), content).expect("write fixture");
}

// ---------------------------------------------------------------------------
// PHP
// ---------------------------------------------------------------------------

#[rstest]
#[case("laravel/framework", "PHP", "Laravel", ProjectType::Backend)]
#[case("symfony/framework-bundle", "PHP", "Symfony", ProjectType::Backend)]
#[case("roots/sage", "PHP", "WordPress", ProjectType::Backend)]
#[case("slim/slim", "PHP", "Slim", ProjectType::Backend)]
#[case("cakephp/cakephp", "PHP", "CakePHP", ProjectType::Backend)]
#[case("codeigniter4/framework", "PHP", "CodeIgniter", ProjectType::Backend)]
fn php_detection(
    #[case] dep: &str,
    #[case] lang: &str,
    #[case] fw: &str,
    #[case] pt: ProjectType,
) {
    let dir = make_dir();
    write(&dir, "composer.json", &format!(r#"{{"require": {{"{dep}": "^1.0"}}}}"#));
    let s = detect_stack(dir.path()).expect("detect");
    assert_eq!(s.primary_language, lang);
    assert_eq!(s.framework.as_deref(), Some(fw));
    assert_eq!(s.project_type, pt);
    assert_eq!(s.confidence, Confidence::High);
}

#[test]
fn php_no_framework() {
    let dir = make_dir();
    write(&dir, "composer.json", r#"{"require": {"monolog/monolog": "^3.0"}}"#);
    let s = detect_stack(dir.path()).expect("detect");
    assert_eq!(s.primary_language, "PHP");
    assert!(s.framework.is_none());
    assert_eq!(s.confidence, Confidence::Medium);
}

// ---------------------------------------------------------------------------
// Dart / Flutter
// ---------------------------------------------------------------------------

#[test]
fn flutter_detection() {
    let dir = make_dir();
    write(&dir, "pubspec.yaml", "name: myapp\ndependencies:\n  flutter:\n    sdk: flutter\n");
    let s = detect_stack(dir.path()).expect("detect");
    assert_eq!(s.primary_language, "Dart");
    assert_eq!(s.framework.as_deref(), Some("Flutter"));
    assert_eq!(s.project_type, ProjectType::Mobile);
    assert_eq!(s.confidence, Confidence::High);
}

#[test]
fn dart_no_flutter() {
    let dir = make_dir();
    write(&dir, "pubspec.yaml", "name: cli_tool\nenvironment:\n  sdk: '>=3.0.0 <4.0.0'\n");
    let s = detect_stack(dir.path()).expect("detect");
    assert_eq!(s.primary_language, "Dart");
    assert!(s.framework.is_none());
}

// ---------------------------------------------------------------------------
// Rust
// ---------------------------------------------------------------------------

#[rstest]
#[case("actix-web", "Rust", "Actix Web", ProjectType::Backend)]
#[case("axum", "Rust", "Axum", ProjectType::Backend)]
#[case("tauri", "Rust", "Tauri", ProjectType::Frontend)]
#[case("leptos", "Rust", "Leptos", ProjectType::Frontend)]
#[case("rocket", "Rust", "Rocket", ProjectType::Backend)]
fn rust_detection(
    #[case] dep: &str,
    #[case] lang: &str,
    #[case] fw: &str,
    #[case] pt: ProjectType,
) {
    let dir = make_dir();
    write(
        &dir,
        "Cargo.toml",
        &format!("[package]\nname = \"app\"\nversion = \"0.1.0\"\n\n[dependencies]\n{dep} = \"1\"\n"),
    );
    let s = detect_stack(dir.path()).expect("detect");
    assert_eq!(s.primary_language, lang);
    assert_eq!(s.framework.as_deref(), Some(fw));
    assert_eq!(s.project_type, pt);
}

#[test]
fn rust_no_framework() {
    let dir = make_dir();
    write(&dir, "Cargo.toml", "[package]\nname = \"app\"\nversion = \"0.1.0\"\n");
    let s = detect_stack(dir.path()).expect("detect");
    assert_eq!(s.primary_language, "Rust");
    assert!(s.framework.is_none());
    assert_eq!(s.confidence, Confidence::Medium);
}

// ---------------------------------------------------------------------------
// Go
// ---------------------------------------------------------------------------

#[rstest]
#[case("github.com/gin-gonic/gin", "Go", "Gin")]
#[case("github.com/labstack/echo/v4", "Go", "Echo")]
#[case("github.com/gofiber/fiber/v2", "Go", "Fiber")]
fn go_detection(#[case] dep: &str, #[case] lang: &str, #[case] fw: &str) {
    let dir = make_dir();
    write(
        &dir,
        "go.mod",
        &format!("module myapp\n\ngo 1.21\n\nrequire {dep} v1.0.0\n"),
    );
    let s = detect_stack(dir.path()).expect("detect");
    assert_eq!(s.primary_language, lang);
    assert_eq!(s.framework.as_deref(), Some(fw));
    assert_eq!(s.project_type, ProjectType::Backend);
}

// ---------------------------------------------------------------------------
// Elixir / Phoenix
// ---------------------------------------------------------------------------

#[test]
fn phoenix_detection() {
    let dir = make_dir();
    write(&dir, "mix.exs", "defmodule MyApp do\n  defp deps do\n    [{:phoenix, \"~> 1.7\"}]\n  end\nend\n");
    let s = detect_stack(dir.path()).expect("detect");
    assert_eq!(s.primary_language, "Elixir");
    assert_eq!(s.framework.as_deref(), Some("Phoenix"));
}

// ---------------------------------------------------------------------------
// JVM (Java / Kotlin)
// ---------------------------------------------------------------------------

#[test]
fn spring_boot_gradle() {
    let dir = make_dir();
    write(
        &dir,
        "build.gradle",
        "plugins { id 'org.springframework.boot' version '3.1.0' }\n",
    );
    let s = detect_stack(dir.path()).expect("detect");
    assert_eq!(s.primary_language, "Java");
    assert_eq!(s.framework.as_deref(), Some("Spring Boot"));
}

#[test]
fn spring_boot_maven() {
    let dir = make_dir();
    write(
        &dir,
        "pom.xml",
        "<project><parent><artifactId>spring-boot-starter-parent</artifactId></parent></project>",
    );
    let s = detect_stack(dir.path()).expect("detect");
    assert_eq!(s.primary_language, "Java");
    assert_eq!(s.framework.as_deref(), Some("Spring Boot"));
}

#[test]
fn kotlin_gradle_kts() {
    let dir = make_dir();
    write(
        &dir,
        "build.gradle.kts",
        "plugins { id(\"org.springframework.boot\") version \"3.1.0\" }\n",
    );
    let s = detect_stack(dir.path()).expect("detect");
    assert_eq!(s.primary_language, "Kotlin");
}

// ---------------------------------------------------------------------------
// Ruby
// ---------------------------------------------------------------------------

#[rstest]
#[case("gem \"rails\", \"~> 7.1\"", "Rails")]
#[case("gem 'sinatra'", "Sinatra")]
#[case("gem 'hanami', '~> 2.0'", "Hanami")]
fn ruby_detection(#[case] gemfile_line: &str, #[case] fw: &str) {
    let dir = make_dir();
    write(&dir, "Gemfile", &format!("source 'https://rubygems.org'\n{gemfile_line}\n"));
    let s = detect_stack(dir.path()).expect("detect");
    assert_eq!(s.primary_language, "Ruby");
    assert_eq!(s.framework.as_deref(), Some(fw));
}

// ---------------------------------------------------------------------------
// JavaScript / TypeScript
// ---------------------------------------------------------------------------

fn pkg_json(deps: &[(&str, &str)]) -> String {
    let entries: Vec<String> = deps.iter().map(|(k, v)| format!("\"{k}\": \"{v}\"")).collect();
    format!("{{\"dependencies\": {{{}}}}}", entries.join(", "))
}

#[rstest]
#[case("next", "^14.0.0", "JavaScript", "Next.js", ProjectType::Frontend)]
#[case("nuxt", "^3.0.0", "JavaScript", "Nuxt", ProjectType::Frontend)]
#[case("react", "^18.0.0", "JavaScript", "React", ProjectType::Frontend)]
#[case("vue", "^3.0.0", "JavaScript", "Vue", ProjectType::Frontend)]
#[case("@angular/core", "^17.0.0", "JavaScript", "Angular", ProjectType::Frontend)]
#[case("svelte", "^4.0.0", "JavaScript", "Svelte", ProjectType::Frontend)]
#[case("@sveltejs/kit", "^2.0.0", "JavaScript", "SvelteKit", ProjectType::Frontend)]
#[case("astro", "^4.0.0", "JavaScript", "Astro", ProjectType::Frontend)]
#[case("gatsby", "^5.0.0", "JavaScript", "Gatsby", ProjectType::Frontend)]
#[case("express", "^4.0.0", "JavaScript", "Express", ProjectType::Backend)]
#[case("fastify", "^4.0.0", "JavaScript", "Fastify", ProjectType::Backend)]
#[case("@nestjs/core", "^10.0.0", "JavaScript", "NestJS", ProjectType::Backend)]
#[case("koa", "^2.0.0", "JavaScript", "Koa", ProjectType::Backend)]
fn js_framework_detection(
    #[case] dep: &str,
    #[case] ver: &str,
    #[case] lang: &str,
    #[case] fw: &str,
    #[case] pt: ProjectType,
) {
    let dir = make_dir();
    write(&dir, "package.json", &pkg_json(&[(dep, ver)]));
    let s = detect_stack(dir.path()).expect("detect");
    assert_eq!(s.primary_language, lang);
    assert_eq!(s.framework.as_deref(), Some(fw));
    assert_eq!(s.project_type, pt);
    assert_eq!(s.confidence, Confidence::High);
}

#[test]
fn typescript_detected_via_tsconfig() {
    let dir = make_dir();
    write(&dir, "package.json", &pkg_json(&[("next", "^14.0.0")]));
    write(&dir, "tsconfig.json", r#"{"compilerOptions": {"target": "es2022"}}"#);
    let s = detect_stack(dir.path()).expect("detect");
    assert_eq!(s.primary_language, "TypeScript");
    assert_eq!(s.framework.as_deref(), Some("Next.js"));
}

#[test]
fn remix_detection() {
    let dir = make_dir();
    write(&dir, "package.json", &pkg_json(&[("@remix-run/react", "^2.0.0")]));
    let s = detect_stack(dir.path()).expect("detect");
    assert_eq!(s.framework.as_deref(), Some("Remix"));
}

// ---------------------------------------------------------------------------
// Python
// ---------------------------------------------------------------------------

#[rstest]
#[case("fastapi\nuvicorn\n", "FastAPI", ProjectType::Backend)]
#[case("Django>=4.2\n", "Django", ProjectType::Backend)]
#[case("Flask>=3.0\n", "Flask", ProjectType::Backend)]
#[case("starlette\nhttpx\n", "Starlette", ProjectType::Backend)]
#[case("torch\nnumpy\n", "PyTorch", ProjectType::Ml)]
#[case("tensorflow>=2.0\n", "TensorFlow", ProjectType::Ml)]
#[case("scikit-learn\npandas\n", "scikit-learn", ProjectType::Ml)]
#[case("streamlit\n", "Streamlit", ProjectType::Ml)]
#[case("gradio\n", "Gradio", ProjectType::Ml)]
fn python_detection(#[case] req: &str, #[case] fw: &str, #[case] pt: ProjectType) {
    let dir = make_dir();
    write(&dir, "requirements.txt", req);
    let s = detect_stack(dir.path()).expect("detect");
    assert_eq!(s.primary_language, "Python");
    assert_eq!(s.framework.as_deref(), Some(fw));
    assert_eq!(s.project_type, pt);
}

#[test]
fn python_pyproject_toml_detection() {
    let dir = make_dir();
    write(
        &dir,
        "pyproject.toml",
        "[tool.poetry.dependencies]\nfastapi = \"^0.110\"\nuvicorn = \"*\"\n",
    );
    let s = detect_stack(dir.path()).expect("detect");
    assert_eq!(s.primary_language, "Python");
    assert_eq!(s.framework.as_deref(), Some("FastAPI"));
}

// ---------------------------------------------------------------------------
// Unknown stack
// ---------------------------------------------------------------------------

#[test]
fn empty_dir_returns_unknown_stack_error() {
    let dir = make_dir();
    let err = detect_stack(dir.path()).unwrap_err();
    assert!(
        err.to_string().contains("could not determine stack"),
        "got: {err}"
    );
}
