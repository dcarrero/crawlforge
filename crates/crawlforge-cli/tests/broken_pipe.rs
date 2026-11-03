//! `crawlforge <command> | head` must end quietly, the way any Unix filter does.
//!
//! Before the fix, `println!` met the closed pipe as an `EPIPE` error and panicked: "failed
//! printing to stdout: Broken pipe", exit status 101. The test runs the real binary with its
//! stdout already closed, so the very first write meets a pipe without a reader.
#![cfg(unix)]

use std::os::unix::process::ExitStatusExt;
use std::process::{Command, Stdio};

/// Runs the binary with a stdout pipe whose read end is closed before the child writes.
fn run_with_closed_stdout(args: &[&str]) -> (std::process::ExitStatus, String) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_crawlforge"))
        .args(args)
        .env_remove("CRAWLFORGE_LANG")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn crawlforge");
    // Dropping our end of the pipe leaves it without a reader: the child's first write fails.
    drop(child.stdout.take());
    let out = child.wait_with_output().expect("wait for crawlforge");
    (out.status, String::from_utf8_lossy(&out.stderr).into_owned())
}

#[test]
fn a_closed_stdout_ends_the_command_quietly_instead_of_panicking() {
    // `rules --lang ja` is the reported case; `--detail` makes sure there is plenty to write.
    for args in [&["rules", "--lang", "ja"][..], &["rules", "--detail"][..]] {
        let (status, stderr) = run_with_closed_stdout(args);
        assert!(!stderr.contains("panicked"), "{args:?} panicked: {stderr}");
        assert!(!stderr.contains("Broken pipe"), "{args:?}: {stderr}");
        assert_ne!(status.code(), Some(101), "{args:?} exited as a panic: {stderr}");
        // Ended by the signal, like `cat` would be — not by an error of its own.
        assert_eq!(status.signal(), Some(libc::SIGPIPE), "{args:?}: {status:?} {stderr}");
    }
}

#[test]
fn an_open_stdout_still_gets_the_whole_output() {
    // The guard of the guard: the fix must not cut a normal run short.
    let out = Command::new(env!("CARGO_BIN_EXE_crawlforge"))
        .args(["rules", "--format", "json"])
        .output()
        .expect("run crawlforge");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).expect("complete JSON");
    assert!(json["count"].as_u64().is_some_and(|n| n > 0), "{json}");
}

