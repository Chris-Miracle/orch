use std::fs;

use orchestra_detector::scan_agent_files;
use tempfile::TempDir;

#[test]
fn scans_known_agent_files_and_dirs() {
    let workspace = TempDir::new().expect("workspace");
    let root = workspace.path();

    fs::create_dir_all(root.join(".cursor/rules")).expect("mkdir cursor");
    fs::create_dir_all(root.join(".claude/agents")).expect("mkdir claude agents");
    fs::create_dir_all(root.join(".github")).expect("mkdir github");

    fs::write(root.join("CLAUDE.md"), "x").expect("write CLAUDE");
    fs::write(root.join("AGENTS.md"), "x").expect("write AGENTS");
    fs::write(root.join(".github/copilot-instructions.md"), "x").expect("write copilot");
    fs::write(root.join(".aider.conf"), "x").expect("write aider");

    let hits = scan_agent_files(root).expect("scan");
    let paths: Vec<String> = hits
        .iter()
        .map(|h| {
            h.path
                .strip_prefix(root)
                .unwrap_or(h.path.as_path())
                .display()
                .to_string()
        })
        .collect();

    assert!(paths.iter().any(|p| p == "CLAUDE.md"));
    assert!(paths.iter().any(|p| p == "AGENTS.md"));
    assert!(paths.iter().any(|p| p == ".github/copilot-instructions.md"));
    assert!(paths.iter().any(|p| p == ".cursor"));
    assert!(paths.iter().any(|p| p == ".claude/agents"));
    assert!(paths.iter().any(|p| p == ".aider.conf"));
}
