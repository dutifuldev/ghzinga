use std::{
    fs,
    path::{Path, PathBuf},
};

#[test]
fn domain_layer_stays_pure() {
    let forbidden = [
        "crate::app",
        "crate::github",
        "crate::input",
        "crate::render",
        "crate::terminal",
        "crossterm",
        "ratatui",
        "reqwest",
        "tokio",
        "std::fs",
        "std::process",
    ];

    assert_no_forbidden_text("src/domain", &forbidden);
}

#[test]
fn github_adapter_does_not_depend_on_tui_layers() {
    let forbidden = [
        "crate::app",
        "crate::input",
        "crate::render",
        "crate::terminal",
        "crossterm",
        "ratatui",
    ];

    assert_no_forbidden_text("src/github", &forbidden);
}

#[test]
fn github_data_layer_does_not_shell_out_to_gh_view_or_api() {
    let source = fs::read_to_string("src/github/gh_cli.rs").expect("read GitHub adapter");

    assert_eq!(source.matches("Command::new(\"gh\")").count(), 1);
    assert!(source.contains(".args([\"auth\", \"token\"])"));

    for forbidden in [
        "gh pr view",
        "gh issue view",
        "gh api",
        ".args([\"pr\", \"view\"",
        ".args([\"issue\", \"view\"",
        ".args([\"api\"",
    ] {
        assert!(
            !source.contains(forbidden),
            "GitHub data adapter contains forbidden gh transport text: {forbidden}"
        );
    }
}

fn assert_no_forbidden_text(root: &str, forbidden: &[&str]) {
    for path in rust_files(Path::new(root)) {
        let source = fs::read_to_string(&path).expect("read source file");
        for text in forbidden {
            assert!(
                !source.contains(text),
                "{} contains forbidden dependency text `{}`",
                path.display(),
                text
            );
        }
    }
}

fn rust_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_rust_files(root, &mut files);
    files.sort();
    files
}

fn collect_rust_files(path: &Path, files: &mut Vec<PathBuf>) {
    if path.is_file() {
        if path.extension().is_some_and(|extension| extension == "rs") {
            files.push(path.to_path_buf());
        }
        return;
    }

    for entry in fs::read_dir(path).expect("read source directory") {
        let entry = entry.expect("read source directory entry");
        collect_rust_files(&entry.path(), files);
    }
}
