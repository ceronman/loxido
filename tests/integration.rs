use std::{env, fs, process::Command};
use std::path::PathBuf;

use regex::Regex;

extern crate test_generator;


use test_generator::test_resources;

fn loxido_command() -> Command {
    // Create full path to binary
    let mut path = env::current_exe().unwrap().parent().unwrap().parent().unwrap().to_owned();
    path.push(env!("CARGO_PKG_NAME"));
    path.set_extension(env::consts::EXE_EXTENSION);
    Command::new(path.into_os_string())
}

fn parse_comments(path: &PathBuf) -> (Vec<String>, Vec<String>) {
    let output_re = Regex::new(r"// expect: ?(.*)").unwrap();
    let error_re = Regex::new(r"// (Error.*)").unwrap();
    let error_line_re = Regex::new(r"// \[(?:c )?line (\d+)\] (Error.*)").unwrap();
    let runtime_error_re = Regex::new(r"// expect runtime error: (.+)").unwrap();

    let mut expected_out = vec![];
    let mut expected_err = vec![];

    println!("{}", path.display());
    let content = fs::read_to_string(path).unwrap();
    for (i, line) in content.lines().enumerate() {
        if let Some(m) = output_re.captures(line) {
            let s = m.get(1).unwrap().as_str().to_owned();
            expected_out.push(s);
        }
        if let Some(m) = error_line_re.captures(line) {
            let line = m.get(1).unwrap().as_str();
            let msg = m.get(2).unwrap().as_str();
            let s = format!("[line {}] {}", line, msg);
            expected_err.push(s);
        }
        if let Some(m) = error_re.captures(line) {
            let msg = m.get(1).unwrap().as_str();
            let s = format!("[line {}] {}", i + 1, msg);
            expected_err.push(s);
        }
        if let Some(m) = runtime_error_re.captures(line) {
            let msg = m.get(1).unwrap().as_str().to_owned();
            let s = format!("[line {}]", i + 1);
            expected_err.push(msg.to_owned());
            expected_err.push(s);
        }
    }
    (expected_out, expected_err)
}

fn run_file_test(filename: &str) {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push(filename);
    let (expected_out, expected_err) = parse_comments(&path);

    let output = loxido_command().arg(path).output().unwrap();
    let out = String::from_utf8(output.stdout).unwrap();
    let err = String::from_utf8(output.stderr).unwrap();

    for (expected, actual) in expected_out.iter().zip(out.lines()) {
        assert_eq!(expected, actual);
    }

    for (expected, actual) in expected_err.iter().zip(err.lines()) {
        assert_eq!(expected, actual);
    }
}

#[test_resources("tests/resources/assignment/*.lox")]
fn test_helloworld(resource: &str) {
    run_file_test(resource);
}