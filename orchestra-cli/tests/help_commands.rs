use assert_cmd::Command;

fn orchestra_command() -> Command {
    assert_cmd::cargo::cargo_bin_cmd!("orchestra")
}

#[test]
fn help_lists_all_available_commands() {
    let output = orchestra_command()
        .arg("--help")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let text = String::from_utf8(output).expect("utf8 help output");

    for expected in [
        "init",
        "project list",
        "project add",
        "sync",
        "status",
        "diff",
        "daemon start",
        "daemon stop",
        "daemon status",
        "daemon install",
        "daemon uninstall",
        "daemon logs",
    ] {
        assert!(
            text.contains(expected),
            "root help should include '{expected}', got:\n{text}"
        );
    }
}
