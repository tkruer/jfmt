use std::env;
use std::fs;
use std::path::Path;

fn print_usage(program: &str) {
    eprintln!("Usage: {program} [--fix] <file1.java> [file2.java ...]");
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let program = args.get(0).map(String::as_str).unwrap_or("jfmtcli");

    // Parse flags and files (simple, no external deps)
    let mut fix = false;
    let mut files: Vec<String> = Vec::new();
    for arg in args.iter().skip(1) {
        if arg == "--fix" {
            fix = true;
        } else {
            files.push(arg.clone());
        }
    }

    if files.is_empty() {
        print_usage(program);
        std::process::exit(2);
    }

    let config = match libjfmt::load_config() {
        Ok(c) => c,
        Err(err) => {
            eprintln!("error loading config: {err}");
            std::process::exit(2);
        }
    };

    let mut total_issues = 0usize;

    for path in &files {
        if !path.ends_with(".java") {
            eprintln!("Skipping non-Java file: {path}");
            continue;
        }
        match lint_file(path, &config, fix) {
            Ok(count) => total_issues += count,
            Err(err) => {
                eprintln!("{path}: error: {err}");
                total_issues += 1; // count as failure
            }
        }
    }

    if total_issues > 0 {
        std::process::exit(1);
    }
}

fn lint_file(path: &str, config: &libjfmt::Config, fix: bool) -> Result<usize, String> {
    let display_path = Path::new(path).display();
    let src = fs::read_to_string(path).map_err(|e| format!("failed to read {display_path}: {e}"))?;

    if fix {
        let (fixed, issues_before) = libjfmt::fix_java_source(&src, config).map_err(|e| e.to_string())?;
        let mut issues_after = issues_before;
        if fixed != src {
            fs::write(path, &fixed).map_err(|e| format!("failed to write {display_path}: {e}"))?;
            eprintln!("applied fixes: {display_path}");
            // Re-lint the fixed content to show remaining issues only
            issues_after = libjfmt::lint_java_source(&fixed, config).map_err(|e| e.to_string())?;
        }
        for issue in &issues_after {
            println!(
                "{}:{}:{}: {}: {}",
                display_path,
                issue.line,
                issue.column,
                issue.rule_id,
                issue.message
            );
        }
        Ok(issues_after.len())
    } else {
        let issues = libjfmt::lint_java_source(&src, config).map_err(|e| e.to_string())?;
        for issue in &issues {
            println!(
                "{}:{}:{}: {}: {}",
                display_path,
                issue.line,
                issue.column,
                issue.rule_id,
                issue.message
            );
        }
        Ok(issues.len())
    }
}
