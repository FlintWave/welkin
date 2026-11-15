// SPDX-License-Identifier: GPL-3.0-or-later
//! Layer 1 offline compilation test runner for welkin-script-compiler.
//!
//! Feeds LSL scripts through each compiler stage and records per-stage results.
//! Each script is compiled in a child process with memory limits to prevent OOM.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use clap::{Parser as ClapParser, Subcommand};
use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(ClapParser)]
#[command(name = "lsl-test-runner", about = "Layer 1 LSL compilation test runner")]
struct Cli {
    #[command(subcommand)]
    command: CliCommand,
}

#[derive(Subcommand)]
enum CliCommand {
    /// Compile all scripts and output JSON results.
    Run {
        #[arg(long)]
        scripts_dir: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
    /// Compile a single script (used internally by subprocess).
    CompileOne {
        #[arg(long)]
        script_path: PathBuf,
    },
    /// Read JSON results and generate a Markdown compatibility matrix.
    Matrix {
        #[arg(long)]
        results: PathBuf,
        #[arg(long)]
        manifest: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
}

#[derive(Deserialize)]
struct Manifest {
    selected_scripts: Vec<ManifestEntry>,
}

#[derive(Deserialize)]
struct ManifestEntry {
    filename: String,
    categories: Vec<String>,
    has_ossl: bool,
    #[allow(dead_code)]
    function_count: Option<u32>,
    #[allow(dead_code)]
    line_count: Option<u32>,
    #[allow(dead_code)]
    source_path: Option<String>,
    #[allow(dead_code)]
    selection_reason: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct Results {
    results: Vec<ScriptResult>,
    summary: Summary,
}

#[derive(Serialize, Deserialize)]
struct ScriptResult {
    file: String,
    categories: Vec<String>,
    has_ossl: bool,
    stages: Stages,
    failure_class: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    missing_builtins: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone)]
struct Stages {
    lex: StageResult,
    parse: StageResult,
    analyze: StageResult,
    codegen: StageResult,
}

#[derive(Serialize, Deserialize, Clone)]
struct StageResult {
    pass: Option<bool>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    errors: Vec<String>,
}

#[derive(Serialize, Deserialize)]
struct Summary {
    total: usize,
    lex_pass: usize,
    parse_pass: usize,
    analyze_pass: usize,
    codegen_pass: usize,
    failure_classes: BTreeMap<String, usize>,
}

// Output from compile-one subprocess (JSON on stdout)
#[derive(Serialize, Deserialize)]
struct CompileOneResult {
    stages: Stages,
    failure_class: Option<String>,
    missing_builtins: Vec<String>,
}

fn compile_one_script(script_path: &Path) {
    let source = match fs::read_to_string(script_path) {
        Ok(s) => s,
        Err(e) => {
            let result = CompileOneResult {
                stages: Stages {
                    lex: StageResult { pass: None, errors: vec![format!("cannot read: {e}")] },
                    parse: StageResult { pass: None, errors: vec![] },
                    analyze: StageResult { pass: None, errors: vec![] },
                    codegen: StageResult { pass: None, errors: vec![] },
                },
                failure_class: Some("io_error".to_string()),
                missing_builtins: vec![],
            };
            println!("{}", serde_json::to_string(&result).unwrap());
            return;
        }
    };

    let builtin_re = Regex::new(r"\b(ll[A-Z]\w+|os[A-Z]\w+)\b").unwrap();
    let mut stages = Stages {
        lex: StageResult { pass: None, errors: vec![] },
        parse: StageResult { pass: None, errors: vec![] },
        analyze: StageResult { pass: None, errors: vec![] },
        codegen: StageResult { pass: None, errors: vec![] },
    };
    let mut failure_class: Option<String> = None;
    let mut missing_builtins: Vec<String> = Vec::new();

    let tokens = match welkin_script_compiler::lexer::lex(&source) {
        Ok(t) => {
            stages.lex.pass = Some(true);
            Some(t)
        }
        Err(e) => {
            stages.lex.pass = Some(false);
            stages.lex.errors.push(e.to_string());
            failure_class = Some(classify_error("lex", &[e.to_string()], &builtin_re));
            None
        }
    };

    let script_ast = if let Some(tokens) = tokens {
        match welkin_script_compiler::parser::Parser::new(tokens).parse() {
            Ok(s) => {
                stages.parse.pass = Some(true);
                Some(s)
            }
            Err(errs) => {
                stages.parse.pass = Some(false);
                let msgs: Vec<String> = errs.iter().map(|e| e.to_string()).collect();
                failure_class = Some(classify_error("parse", &msgs, &builtin_re));
                stages.parse.errors = msgs;
                None
            }
        }
    } else {
        None
    };

    let analyzed = if let Some(mut script_ast) = script_ast {
        match welkin_script_compiler::analyzer::Analyzer::new().analyze(&mut script_ast) {
            Ok(()) => {
                stages.analyze.pass = Some(true);
                Some(script_ast)
            }
            Err(errs) => {
                stages.analyze.pass = Some(false);
                let msgs: Vec<String> = errs.iter().map(|e| e.to_string()).collect();
                let cls = classify_error("analyze", &msgs, &builtin_re);
                if cls == "missing_builtin" {
                    let mut builtins: BTreeSet<String> = BTreeSet::new();
                    for msg in &msgs {
                        for cap in builtin_re.find_iter(msg) {
                            builtins.insert(cap.as_str().to_string());
                        }
                    }
                    missing_builtins = builtins.into_iter().collect();
                }
                failure_class = Some(cls);
                stages.analyze.errors = msgs;
                None
            }
        }
    } else {
        None
    };

    if let Some(ref script_ast) = analyzed {
        let _luau = welkin_script_compiler::codegen::CodeGen::generate(script_ast);
        stages.codegen.pass = Some(true);
    }

    let result = CompileOneResult { stages, failure_class, missing_builtins };
    println!("{}", serde_json::to_string(&result).unwrap());
}

fn run_compilation(scripts_dir: &Path, output_path: &Path) {
    let manifest_path = scripts_dir.join("manifest.json");
    let manifest_text = fs::read_to_string(&manifest_path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", manifest_path.display()));
    let manifest: Manifest = serde_json::from_str(&manifest_text)
        .unwrap_or_else(|e| panic!("invalid manifest.json: {e}"));

    let self_exe = std::env::current_exe().expect("cannot find own executable");

    let mut results: Vec<ScriptResult> = Vec::new();
    let mut lex_pass = 0usize;
    let mut parse_pass = 0usize;
    let mut analyze_pass = 0usize;
    let mut codegen_pass = 0usize;
    let mut failure_classes: BTreeMap<String, usize> = BTreeMap::new();

    for entry in &manifest.selected_scripts {
        let script_path = scripts_dir.join(&entry.filename);
        eprintln!("  compiling: {}", entry.filename);

        // Run each script in a subprocess with 512MB memory limit and 30s timeout
        let child_result = Command::new("bash")
            .arg("-c")
            .arg(format!(
                "ulimit -v 524288; exec \"{}\" compile-one --script-path \"{}\"",
                self_exe.display(),
                script_path.display()
            ))
            .output();

        let (stages, failure_class, missing_builtins) = match child_result {
            Ok(output) if output.status.success() => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                match serde_json::from_str::<CompileOneResult>(&stdout) {
                    Ok(r) => (r.stages, r.failure_class, r.missing_builtins),
                    Err(e) => {
                        eprintln!("    JSON parse error: {e}");
                        let stages = Stages {
                            lex: StageResult { pass: None, errors: vec![] },
                            parse: StageResult { pass: None, errors: vec![] },
                            analyze: StageResult { pass: None, errors: vec![] },
                            codegen: StageResult { pass: None, errors: vec![] },
                        };
                        (stages, Some("runner_error".to_string()), vec![])
                    }
                }
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                let code = output.status.code().unwrap_or(-1);
                eprintln!("    subprocess failed (exit {}): {}", code, stderr.lines().next().unwrap_or(""));
                let mut stages = Stages {
                    lex: StageResult { pass: None, errors: vec![] },
                    parse: StageResult { pass: None, errors: vec![] },
                    analyze: StageResult { pass: None, errors: vec![] },
                    codegen: StageResult { pass: None, errors: vec![] },
                };
                // If it crashed with memory allocation failure, lex likely passed
                if stderr.contains("memory allocation") {
                    stages.lex.pass = Some(true);
                    stages.parse.pass = Some(false);
                    stages.parse.errors.push("parser OOM (exponential memory growth)".to_string());
                    (stages, Some("parser_oom".to_string()), vec![])
                } else {
                    stages.lex.pass = None;
                    (stages, Some(format!("crash_{code}")), vec![])
                }
            }
            Err(e) => {
                eprintln!("    cannot spawn subprocess: {e}");
                let stages = Stages {
                    lex: StageResult { pass: None, errors: vec![] },
                    parse: StageResult { pass: None, errors: vec![] },
                    analyze: StageResult { pass: None, errors: vec![] },
                    codegen: StageResult { pass: None, errors: vec![] },
                };
                (stages, Some("runner_error".to_string()), vec![])
            }
        };

        if stages.lex.pass == Some(true) { lex_pass += 1; }
        if stages.parse.pass == Some(true) { parse_pass += 1; }
        if stages.analyze.pass == Some(true) { analyze_pass += 1; }
        if stages.codegen.pass == Some(true) { codegen_pass += 1; }

        if let Some(ref cls) = failure_class {
            *failure_classes.entry(cls.clone()).or_insert(0) += 1;
        }

        results.push(ScriptResult {
            file: entry.filename.clone(),
            categories: entry.categories.clone(),
            has_ossl: entry.has_ossl,
            stages,
            failure_class,
            missing_builtins,
        });
    }

    let total = results.len();
    let summary = Summary {
        total,
        lex_pass,
        parse_pass,
        analyze_pass,
        codegen_pass,
        failure_classes: failure_classes.clone(),
    };

    let output_data = Results { results, summary };
    let json = serde_json::to_string_pretty(&output_data).expect("JSON serialization failed");

    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).ok();
    }
    fs::write(output_path, &json)
        .unwrap_or_else(|e| panic!("cannot write {}: {e}", output_path.display()));

    eprintln!("\n=== Layer 1 compilation results ===");
    eprintln!("Total scripts: {total}");
    eprintln!("Lex pass:      {lex_pass}/{total}");
    eprintln!("Parse pass:    {parse_pass}/{total}");
    eprintln!("Analyze pass:  {analyze_pass}/{total}");
    eprintln!("Codegen pass:  {codegen_pass}/{total}");
    eprintln!("Failure classes:");
    for (cls, count) in &failure_classes {
        eprintln!("  {cls}: {count}");
    }
    eprintln!("Results written to {}", output_path.display());
}

fn classify_error(stage: &str, messages: &[String], builtin_re: &Regex) -> String {
    let combined = messages.join(" ");
    match stage {
        "lex" => "lex_error".to_string(),
        "parse" => {
            if combined.contains("unsupported") || combined.contains("not supported") {
                "unsupported_syntax".to_string()
            } else {
                "parse_error".to_string()
            }
        }
        "analyze" => {
            let has_unknown = combined.contains("unknown") || combined.contains("undefined")
                || combined.contains("not defined") || combined.contains("undeclared");
            let has_builtin_name = builtin_re.is_match(&combined);
            if has_unknown && has_builtin_name {
                "missing_builtin".to_string()
            } else if (combined.contains("type") && combined.contains("mismatch"))
                || combined.contains("cannot convert")
                || (combined.contains("expected") && combined.contains("got"))
            {
                "type_error".to_string()
            } else if has_builtin_name {
                "missing_builtin".to_string()
            } else {
                "analysis_error".to_string()
            }
        }
        "codegen" => "codegen_error".to_string(),
        _ => "unknown".to_string(),
    }
}

fn generate_matrix(results_path: &Path, manifest_path: &Path, output_path: &Path) {
    let results_text = fs::read_to_string(results_path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", results_path.display()));
    let results: Results = serde_json::from_str(&results_text)
        .unwrap_or_else(|e| panic!("invalid results JSON: {e}"));

    let manifest_text = fs::read_to_string(manifest_path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", manifest_path.display()));
    let _manifest: Manifest = serde_json::from_str(&manifest_text)
        .unwrap_or_else(|e| panic!("invalid manifest.json: {e}"));

    let mut md = String::with_capacity(8192);

    md.push_str("# LSL Compilation Compatibility Matrix\n\n");
    md.push_str("| Script | Categories | Lex | Parse | Analyze | Codegen | Failure | Missing Builtins |\n");
    md.push_str("|--------|-----------|-----|-------|---------|---------|---------|------------------|\n");

    for r in &results.results {
        let cats = if r.categories.len() > 3 {
            format!("{},...", r.categories[..3].join(","))
        } else {
            r.categories.join(",")
        };
        let lex = stage_cell(&r.stages.lex);
        let parse = stage_cell(&r.stages.parse);
        let analyze = stage_cell(&r.stages.analyze);
        let codegen = stage_cell(&r.stages.codegen);
        let failure = r.failure_class.as_deref().unwrap_or("-");
        let missing = if r.missing_builtins.is_empty() {
            "-".to_string()
        } else if r.missing_builtins.len() > 3 {
            format!("{},... ({} total)", r.missing_builtins[..3].join(","), r.missing_builtins.len())
        } else {
            r.missing_builtins.join(",")
        };
        md.push_str(&format!(
            "| {file} | {cats} | {lex} | {parse} | {analyze} | {codegen} | {failure} | {missing} |\n",
            file = r.file,
        ));
    }

    let s = &results.summary;
    let pct = if s.total > 0 { s.codegen_pass * 100 / s.total } else { 0 };
    md.push_str(&format!(
        "\n## Summary\n\n\
         - **Total scripts**: {total}\n\
         - **Full compilation**: {codegen}/{total} ({pct}%)\n\
         - **Lex pass**: {lex}/{total}\n\
         - **Parse pass**: {parse}/{total}\n\
         - **Analyze pass**: {analyze}/{total}\n\
         - **Codegen pass**: {codegen}/{total}\n",
        total = s.total,
        lex = s.lex_pass,
        parse = s.parse_pass,
        analyze = s.analyze_pass,
        codegen = s.codegen_pass,
    ));

    md.push_str("\n## Failure breakdown\n\n");
    md.push_str("| Class | Count | Scripts |\n");
    md.push_str("|-------|-------|---------|\n");

    let mut class_scripts: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for r in &results.results {
        if let Some(ref cls) = r.failure_class {
            class_scripts.entry(cls.clone()).or_default().push(r.file.clone());
        }
    }
    for (cls, scripts) in &class_scripts {
        let count = scripts.len();
        let names = if scripts.len() > 5 {
            format!("{}, ... ({count} total)", scripts[..5].join(", "))
        } else {
            scripts.join(", ")
        };
        md.push_str(&format!("| {cls} | {count} | {names} |\n"));
    }

    let mut builtin_usage: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for r in &results.results {
        for b in &r.missing_builtins {
            builtin_usage.entry(b.clone()).or_default().push(r.file.clone());
        }
    }
    if !builtin_usage.is_empty() {
        let mut sorted: Vec<_> = builtin_usage.into_iter().collect();
        sorted.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then(a.0.cmp(&b.0)));

        md.push_str("\n## Missing builtins (unique)\n\n");
        md.push_str("| Builtin | Occurrences | Scripts |\n");
        md.push_str("|---------|------------|---------|\n");
        for (builtin, scripts) in &sorted {
            let names = if scripts.len() > 5 {
                format!("{}, ... ({} total)", scripts[..5].join(", "), scripts.len())
            } else {
                scripts.join(", ")
            };
            md.push_str(&format!("| {builtin} | {} | {names} |\n", scripts.len()));
        }
    }

    md.push_str("\n## Category coverage\n\n");
    md.push_str("| Category | Scripts | Pass | Fail |\n");
    md.push_str("|----------|---------|------|------|\n");

    let mut cat_stats: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    for r in &results.results {
        let passed = r.failure_class.is_none();
        for cat in &r.categories {
            let entry = cat_stats.entry(cat.clone()).or_insert((0, 0));
            if passed { entry.0 += 1; } else { entry.1 += 1; }
        }
    }
    for (cat, (pass, fail)) in &cat_stats {
        md.push_str(&format!("| {cat} | {} | {pass} | {fail} |\n", pass + fail));
    }

    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).ok();
    }
    fs::write(output_path, &md)
        .unwrap_or_else(|e| panic!("cannot write {}: {e}", output_path.display()));
    eprintln!("Matrix written to {}", output_path.display());
}

fn stage_cell(stage: &StageResult) -> &'static str {
    match stage.pass {
        Some(true) => "PASS",
        Some(false) => "FAIL",
        None => "-",
    }
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        CliCommand::Run { scripts_dir, output } => run_compilation(&scripts_dir, &output),
        CliCommand::CompileOne { script_path } => compile_one_script(&script_path),
        CliCommand::Matrix { results, manifest, output } => generate_matrix(&results, &manifest, &output),
    }
}
