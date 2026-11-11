//! End-to-end tests: compile .ccr programs with the real binary and run them.
//! They need clang or gcc, so they're skipped with a note when neither is installed.

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};

fn have_c_compiler() -> bool {
    env::var("CCORE_CC").is_ok()
        || ["clang", "gcc", "cc"]
            .iter()
            .any(|cc| Command::new(cc).arg("--version").output().is_ok())
}

fn ccore(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ccore"))
        .args(args)
        .output()
        .expect("failed to run ccore")
}

fn run_file(path: &str) -> Output {
    ccore(&["run", path])
}

fn run_source(name: &str, src: &str) -> Output {
    let path: PathBuf =
        env::temp_dir().join(format!("ccore-test-{}-{name}.ccr", std::process::id()));
    fs::write(&path, src).unwrap();
    let out = run_file(path.to_str().unwrap());
    let _ = fs::remove_file(&path);
    out
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).replace("\r\n", "\n")
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

macro_rules! require_cc {
    () => {
        if !have_c_compiler() {
            eprintln!("skipping: no C compiler found");
            return;
        }
    };
}

#[test]
fn hello_world() {
    require_cc!();
    let out = run_file("examples/hello.ccr");
    assert_eq!(stdout(&out), "Hello, world!\n");
    assert_eq!(out.status.code(), Some(0));
}

#[test]
fn fibonacci() {
    require_cc!();
    let out = run_file("examples/fib.ccr");
    assert_eq!(stdout(&out), "0\n1\n1\n2\n3\n5\n8\n13\n21\n34\n88\n");
}

#[test]
fn exit_code_comes_from_main() {
    require_cc!();
    assert_eq!(run_file("examples/add.ccr").status.code(), Some(0));
    let out = run_source("exit", "fn main() -> i32 { return 42; }");
    assert_eq!(out.status.code(), Some(42));
}

#[test]
fn shadowing_and_scopes() {
    require_cc!();
    let out = run_source(
        "shadow",
        "fn main() {
            let x = 1;
            let x = x + 1;
            {
                let x = x * 10;
                println(x);
            }
            println(x);
        }",
    );
    assert_eq!(stdout(&out), "20\n2\n");
}

#[test]
fn integer_overflow_panics() {
    require_cc!();
    let out = run_source(
        "overflow",
        "fn main() {
            let mut n = 2147483600;
            while true {
                n = n + 10;
            }
        }",
    );
    assert_eq!(out.status.code(), Some(101));
    assert!(
        stderr(&out).contains(":4: integer overflow"),
        "{}",
        stderr(&out)
    );
}

#[test]
fn division_by_zero_panics() {
    require_cc!();
    let out = run_source(
        "divzero",
        "fn div(a: i64, b: i64) -> i64 { return a / b; }
        fn main() { println(div(10, 0)); }",
    );
    assert_eq!(out.status.code(), Some(101));
    assert!(stderr(&out).contains("division by zero"));
}

#[test]
fn compile_errors_are_reported() {
    // No C compiler needed: this fails before codegen.
    let out = run_source("error", "fn main() { let x = 1; x = 2; }");
    assert_eq!(out.status.code(), Some(1));
    let err = stderr(&out);
    assert!(
        err.contains("cannot assign twice to immutable variable `x`"),
        "{err}"
    );
    assert!(err.contains("^"), "{err}");
}
