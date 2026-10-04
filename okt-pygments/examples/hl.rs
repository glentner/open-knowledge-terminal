//! Highlight files the way the reference does and print the result as JSON.
//!
//! ```text
//! cargo run --release -p okt-pygments --example hl -- FILE [--path LOGICAL_PATH] [--lang ALIAS]
//! cargo run --release -p okt-pygments --example hl -- --batch < list
//! ```
//!
//! Single file: prints the JSON array of lines, or `{"unsupported": "<LexerClass>"}`.
//! The file is decoded like the reference does (`bytes.decode('utf-8', 'replace')`).
//!
//! `--batch`: reads lines of `FILE\tPATH\tLANG` (PATH/LANG may be empty) from stdin and prints
//! one JSON value per line, so a test driver can check many files with one process.
//! `--time`: also print the highlighting time to stderr.
//! `--which`: in batch mode, print the selected lexer class (JSON string or null) instead of
//! highlighting; FILE may then name a file that does not exist (its text is then empty).
//! `--bench N`: highlight the file N more times after the first call and report the timing
//! of the first (cold: data load + regex compilation) and the warm calls.

use std::io::{BufRead, Write};
use std::time::Instant;

use okt_pygments::{Highlight, highlight_lines, select_lexer};

fn run(file: &str, path: Option<&str>, lang: Option<&str>, time: bool) -> String {
    let bytes = match std::fs::read(file) {
        Ok(b) => b,
        Err(e) => return serde_json::json!({ "error": e.to_string() }).to_string(),
    };
    let text = String::from_utf8_lossy(&bytes);
    let t0 = Instant::now();
    let res = highlight_lines(&text, Some(path.unwrap_or(file)), lang);
    if time {
        eprintln!("{file}: {:.2} ms", t0.elapsed().as_secs_f64() * 1e3);
    }
    match res {
        Highlight::Lines(lines) => serde_json::to_string(&lines).unwrap(),
        Highlight::Unsupported { lexer } => serde_json::json!({ "unsupported": lexer }).to_string(),
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut file = None;
    let mut path = None;
    let mut lang = None;
    let mut batch = false;
    let mut time = false;
    let mut bench = 0usize;
    let mut which = false;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--lang" => lang = it.next().cloned(),
            "--path" => path = it.next().cloned(),
            "--batch" => batch = true,
            "--time" => time = true,
            "--which" => which = true,
            "--bench" => bench = it.next().and_then(|n| n.parse().ok()).unwrap_or(10),
            _ => file = Some(a.clone()),
        }
    }
    if batch {
        let stdin = std::io::stdin();
        let mut out = std::io::stdout().lock();
        for line in stdin.lock().lines() {
            let line = line.unwrap();
            let mut f = line.split('\t');
            let file = f.next().unwrap_or("");
            let path = f.next().filter(|s| !s.is_empty());
            let lang = f.next().filter(|s| !s.is_empty());
            if which {
                let text = std::fs::read(file)
                    .map(|b| String::from_utf8_lossy(&b).into_owned())
                    .unwrap_or_default();
                let sel = select_lexer(&text, Some(path.unwrap_or(file)), lang);
                writeln!(out, "{}", serde_json::to_string(&sel).unwrap()).unwrap();
                continue;
            }
            writeln!(out, "{}", run(file, path, lang, time)).unwrap();
            out.flush().unwrap();
        }
        return;
    }
    let Some(file) = file else {
        eprintln!("usage: hl FILE [--path LOGICAL_PATH] [--lang ALIAS] [--time] | hl --batch");
        std::process::exit(2);
    };
    if bench > 0 {
        let text = String::from_utf8_lossy(&std::fs::read(&file).expect("read")).into_owned();
        let p = Some(path.as_deref().unwrap_or(&file));
        let t0 = Instant::now();
        let first = highlight_lines(&text, p, lang.as_deref());
        let cold = t0.elapsed().as_secs_f64() * 1e3;
        let mut warm: Vec<f64> = (0..bench)
            .map(|_| {
                let t = Instant::now();
                let r = highlight_lines(&text, p, lang.as_deref());
                assert_eq!(r, first);
                t.elapsed().as_secs_f64() * 1e3
            })
            .collect();
        warm.sort_by(|a, b| a.partial_cmp(b).unwrap());
        println!(
            "{file}: {} bytes, cold {cold:.1} ms, warm min {:.1} ms / median {:.1} ms ({bench} runs)",
            text.len(),
            warm[0],
            warm[warm.len() / 2]
        );
        return;
    }
    println!("{}", run(&file, path.as_deref(), lang.as_deref(), time));
}
