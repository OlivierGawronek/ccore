mod ast;
mod checker;
mod codegen;
mod diagnostic;
mod lexer;
mod parser;
mod token;
mod toolchain;

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{self, Command};

const USAGE: &str = "\
usage:
    ccore build <file.ccr> [-o <output>]   compile to an executable
    ccore run <file.ccr>                   compile and run
    ccore emit-c <file.ccr>                print the generated C code
    ccore tokens <file.ccr>                print the token stream";

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let (command, path) = match args.as_slice() {
        [command, path, ..] => (command.as_str(), path.as_str()),
        _ => fail(USAGE),
    };

    match command {
        "build" => {
            let output = match &args[2..] {
                [] => default_output(path),
                [flag, out] if flag == "-o" => PathBuf::from(out),
                _ => fail(USAGE),
            };
            let c_code = compile_to_c(path);
            build_executable(&c_code, &output);
        }
        "run" => {
            let c_code = compile_to_c(path);
            let exe = env::temp_dir().join(format!(
                "ccore-run-{}{}",
                process::id(),
                env::consts::EXE_SUFFIX
            ));
            build_executable(&c_code, &exe);

            let status = Command::new(&exe).status();
            let _ = fs::remove_file(&exe);
            match status {
                Ok(status) => process::exit(status.code().unwrap_or(1)),
                Err(e) => fail(&format!("error: cannot run {}: {e}", exe.display())),
            }
        }
        "emit-c" => print!("{}", compile_to_c(path)),
        "tokens" => {
            let src = read_source(path);
            let tokens = or_report(lexer::Lexer::new(&src).tokenize(), path, &src);
            for t in tokens {
                println!("{:>4}..{:<4} {:?}", t.span.start, t.span.end, t.kind);
            }
        }
        _ => fail(USAGE),
    }
}

fn fail(message: &str) -> ! {
    eprintln!("{}", message.trim_end());
    process::exit(1);
}

fn read_source(path: &str) -> String {
    fs::read_to_string(path).unwrap_or_else(|e| fail(&format!("error: cannot read {path}: {e}")))
}

fn or_report<T>(result: Result<T, diagnostic::Diagnostic>, path: &str, src: &str) -> T {
    result.unwrap_or_else(|d| fail(&d.render(path, src)))
}

fn compile_to_c(path: &str) -> String {
    let src = read_source(path);
    let tokens = or_report(lexer::Lexer::new(&src).tokenize(), path, &src);
    let mut program = or_report(parser::parse(tokens), path, &src);
    or_report(checker::check(&mut program), path, &src);

    let source_name = Path::new(path)
        .file_name()
        .map_or(path.into(), |n| n.to_string_lossy());
    codegen::generate(&program, &src, &source_name)
}

fn build_executable(c_code: &str, output: &Path) {
    let Some(cc) = toolchain::find_c_compiler() else {
        fail(
            "error: no C compiler found\n\
             ccore generates C and needs clang or gcc on your PATH (or set CCORE_CC)",
        );
    };
    if let Err(stderr) = toolchain::compile_c(&cc, c_code, output) {
        // Generated C should always compile, so this is either a broken toolchain
        // (e.g. a missing linker) or a bug in ccore, never a problem in the user's code.
        fail(&format!(
            "error: `{cc}` failed to build the generated C code\n\
             note: check that `{cc}` can compile and link a plain C program; \
             if it can, this is a bug in ccore\n\n{stderr}"
        ));
    }
}

fn default_output(path: &str) -> PathBuf {
    let stem = Path::new(path)
        .file_stem()
        .map_or("out".into(), |s| s.to_string_lossy());
    PathBuf::from(format!("{stem}{}", env::consts::EXE_SUFFIX))
}
