//! Tests for the `cargo generate-lockfile` command.

#![allow(deprecated)]

use cargo_test_support::registry::{Package, RegistryBuilder};
use cargo_test_support::{basic_manifest, paths, project, ProjectBuilder};
use std::fs;

#[cargo_test]
fn adding_and_removing_packages() {
    let p = project()
        .file("src/main.rs", "fn main() {}")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.0.1"))
        .file("bar/src/lib.rs", "")
        .build();

    p.cargo("generate-lockfile").run();

    let lock1 = p.read_lockfile();

    // add a dep
    p.change_file(
        "Cargo.toml",
        r#"
            [package]
            name = "foo"
            authors = []
            version = "0.0.1"

            [dependencies.bar]
            path = "bar"
        "#,
    );
    p.cargo("generate-lockfile").run();
    let lock2 = p.read_lockfile();
    assert_ne!(lock1, lock2);

    // change the dep
    p.change_file("bar/Cargo.toml", &basic_manifest("bar", "0.0.2"));
    p.cargo("generate-lockfile").run();
    let lock3 = p.read_lockfile();
    assert_ne!(lock1, lock3);
    assert_ne!(lock2, lock3);

    // remove the dep
    println!("lock4");
    p.change_file(
        "Cargo.toml",
        r#"
            [package]
            name = "foo"
            authors = []
            version = "0.0.1"
        "#,
    );
    p.cargo("generate-lockfile").run();
    let lock4 = p.read_lockfile();
    assert_eq!(lock1, lock4);
}

#[cargo_test]
fn no_index_update_sparse() {
    let _registry = RegistryBuilder::new().http_index().build();
    no_index_update();
}

#[cargo_test]
fn no_index_update_git() {
    no_index_update();
}

fn no_index_update() {
    Package::new("serde", "1.0.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"
                authors = []
                version = "0.0.1"

                [dependencies]
                serde = "1.0"
            "#,
        )
        .file("src/main.rs", "fn main() {}")
        .build();

    p.cargo("generate-lockfile")
        .with_stderr(
            "\
[UPDATING] `[..]` index
[LOCKING] 2 packages to latest compatible versions
",
        )
        .run();

    p.cargo("generate-lockfile -Zno-index-update")
        .masquerade_as_nightly_cargo(&["no-index-update"])
        .with_stdout("")
        .with_stderr(
            "\
[LOCKING] 2 packages to latest compatible versions
",
        )
        .run();
}

#[cargo_test]
fn preserve_metadata() {
    let p = project()
        .file("src/main.rs", "fn main() {}")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.0.1"))
        .file("bar/src/lib.rs", "")
        .build();

    p.cargo("generate-lockfile").run();

    let metadata = r#"
[metadata]
bar = "baz"
foo = "bar"
"#;
    let lock = p.read_lockfile();
    let data = lock + metadata;
    p.change_file("Cargo.lock", &data);

    // Build and make sure the metadata is still there
    p.cargo("build").run();
    let lock = p.read_lockfile();
    assert!(lock.contains(metadata.trim()), "{}", lock);

    // Update and make sure the metadata is still there
    p.cargo("update").run();
    let lock = p.read_lockfile();
    assert!(lock.contains(metadata.trim()), "{}", lock);
}

#[cargo_test]
fn preserve_line_endings_issue_2076() {
    let p = project()
        .file("src/main.rs", "fn main() {}")
        .file("bar/Cargo.toml", &basic_manifest("bar", "0.0.1"))
        .file("bar/src/lib.rs", "")
        .build();

    let lockfile = p.root().join("Cargo.lock");
    p.cargo("generate-lockfile").run();
    assert!(lockfile.is_file());
    p.cargo("generate-lockfile").run();

    let lock0 = p.read_lockfile();

    assert!(lock0.starts_with("# This file is automatically @generated by Cargo.\n# It is not intended for manual editing.\n"));

    let lock1 = lock0.replace("\n", "\r\n");
    p.change_file("Cargo.lock", &lock1);

    p.cargo("generate-lockfile").run();

    let lock2 = p.read_lockfile();

    assert!(lock2.starts_with("# This file is automatically @generated by Cargo.\r\n# It is not intended for manual editing.\r\n"));
    assert_eq!(lock1, lock2);
}

#[cargo_test]
fn cargo_update_generate_lockfile() {
    let p = project().file("src/main.rs", "fn main() {}").build();

    let lockfile = p.root().join("Cargo.lock");
    assert!(!lockfile.is_file());
    p.cargo("update").with_stderr("").run();
    assert!(lockfile.is_file());

    fs::remove_file(p.root().join("Cargo.lock")).unwrap();

    assert!(!lockfile.is_file());
    p.cargo("update").with_stderr("").run();
    assert!(lockfile.is_file());
}

#[cargo_test]
fn duplicate_entries_in_lockfile() {
    let _a = ProjectBuilder::new(paths::root().join("a"))
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "a"
            authors = []
            version = "0.0.1"

            [dependencies]
            common = {path="common"}
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    let common_toml = &basic_manifest("common", "0.0.1");

    let _common_in_a = ProjectBuilder::new(paths::root().join("a/common"))
        .file("Cargo.toml", common_toml)
        .file("src/lib.rs", "")
        .build();

    let b = ProjectBuilder::new(paths::root().join("b"))
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "b"
            authors = []
            version = "0.0.1"

            [dependencies]
            common = {path="common"}
            a = {path="../a"}
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    let _common_in_b = ProjectBuilder::new(paths::root().join("b/common"))
        .file("Cargo.toml", common_toml)
        .file("src/lib.rs", "")
        .build();

    // should fail due to a duplicate package `common` in the lock file
    b.cargo("build")
        .with_status(101)
        .with_stderr_contains(
            "[..]package collision in the lockfile: packages common [..] and \
             common [..] are different, but only one can be written to \
             lockfile unambiguously",
        )
        .run();
}

#[cargo_test]
fn generate_lockfile_holds_lock_and_offline() {
    Package::new("syn", "1.0.0").publish();

    let p = project()
        .file(
            "Cargo.toml",
            r#"
                [package]
                name = "foo"

                [dependencies]
                syn = "1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .build();

    p.cargo("generate-lockfile")
        .with_stderr(
            "\
[UPDATING] `[..]` index
[LOCKING] 2 packages to latest compatible versions
",
        )
        .run();

    p.cargo("generate-lockfile --offline")
        .with_stderr_contains(
            "\
[LOCKING] 2 packages to latest compatible versions
",
        )
        .run();
}
