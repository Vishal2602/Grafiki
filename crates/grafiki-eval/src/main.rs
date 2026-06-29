//! `grafiki-eval` CLI — mirrors the `grafiki` CLI conventions (clap derive,
//! `--format json|md`).

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use grafiki_core::SearchMode;

use grafiki_eval::config::{EvalConfig, EvalResult, OutputFormat};
use grafiki_eval::dataset::{RedactionDataset, RetrievalDataset};
use grafiki_eval::report;
use grafiki_eval::runner::redaction::{run_redaction, RedactionReport};
use grafiki_eval::runner::retrieval::{run_retrieval, RetrievalReport};

#[derive(Parser)]
#[command(
    name = "grafiki-eval",
    version,
    about = "Grafiki evaluation harness — retrieval, redaction, and memory-QA metrics (H1)"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run an eval arm and emit results.json / report.md.
    Run(Box<RunArgs>),
    /// Print where to validate the IR metrics against the pytrec_eval oracle.
    ValidateMetrics,
}

#[derive(Parser)]
struct RunArgs {
    /// retrieval | redaction | all
    #[arg(long, default_value = "all")]
    arm: String,
    /// Dataset path (defaults to the bundled fixture for the chosen arm).
    #[arg(long)]
    dataset: Option<PathBuf>,
    /// keyword | semantic | hybrid | all (retrieval arm only)
    #[arg(long, default_value = "keyword")]
    mode: String,
    #[arg(long, default_value = "md")]
    format: String,
    /// Write results.json + report.md into this directory (else print to stdout).
    #[arg(long)]
    out: Option<PathBuf>,
    #[arg(long, default_value_t = 42)]
    seed: u64,
    #[arg(long, default_value_t = 2000)]
    bootstrap: usize,
    #[arg(long, default_value_t = 10_000)]
    permutation: usize,
    #[arg(long, default_value_t = 20)]
    limit: usize,
    /// Compare against this baseline.json.
    #[arg(long)]
    baseline: Option<PathBuf>,
    /// Exit non-zero if a regression (or any redaction leak) is detected.
    #[arg(long, default_value_t = false)]
    fail_on_regression: bool,
    /// Write a fresh baseline.json from this run instead of gating.
    #[arg(long)]
    write_baseline: Option<PathBuf>,
    #[arg(long, default_value_t = 0.05)]
    tolerance: f64,
}

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures")
}

fn parse_modes(raw: &str) -> EvalResult<Vec<SearchMode>> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "all" => Ok(vec![
            SearchMode::Keyword,
            SearchMode::Semantic,
            SearchMode::Hybrid,
        ]),
        other => Ok(vec![SearchMode::parse(other)?]),
    }
}

fn write_outputs(out: &Path, stem: &str, json: &serde_json::Value, md: &str) -> EvalResult<()> {
    std::fs::create_dir_all(out)?;
    std::fs::write(
        out.join(format!("{stem}.json")),
        serde_json::to_string_pretty(json)?,
    )?;
    std::fs::write(out.join(format!("{stem}.report.md")), md)?;
    Ok(())
}

fn run(args: &RunArgs) -> EvalResult<i32> {
    let cfg = EvalConfig {
        seed: args.seed,
        bootstrap: args.bootstrap,
        permutation: args.permutation,
        limit: args.limit,
        format: OutputFormat::parse(&args.format)?,
        out_dir: args.out.clone(),
        baseline: args.baseline.clone(),
        fail_on_regression: args.fail_on_regression,
    };

    let arm = args.arm.trim().to_ascii_lowercase();
    let do_retrieval = arm == "retrieval" || arm == "all";
    let do_redaction = arm == "redaction" || arm == "all";
    if !do_retrieval && !do_redaction {
        return Err(format!(
            "unknown --arm '{}' (expected retrieval|redaction|all)",
            args.arm
        )
        .into());
    }

    let mut retrieval_report: Option<RetrievalReport> = None;
    let mut redaction_report: Option<RedactionReport> = None;

    if do_retrieval {
        let dir = args
            .dataset
            .clone()
            .filter(|_| arm == "retrieval")
            .unwrap_or_else(|| fixtures_dir().join("retrieval/grafiki_dev_v1"));
        let dataset = RetrievalDataset::load(&dir)?;
        let modes = parse_modes(&args.mode)?;
        let rep = run_retrieval(&dataset, &modes, &cfg)?;
        let json = report::retrieval_json(&rep, &cfg);
        let md = report::retrieval_md(&rep, &cfg);
        if let Some(out) = &cfg.out_dir {
            write_outputs(out, "retrieval", &json, &md)?;
        } else if matches!(cfg.format, OutputFormat::Json) {
            println!("{}", serde_json::to_string_pretty(&json)?);
        } else {
            println!("{md}");
        }
        retrieval_report = Some(rep);
    }

    if do_redaction {
        let path = args
            .dataset
            .clone()
            .filter(|_| arm == "redaction")
            .unwrap_or_else(|| fixtures_dir().join("redaction/corpus_v1.jsonl"));
        let dataset = RedactionDataset::load(&path)?;
        let rep = run_redaction(&dataset)?;
        let json = report::redaction_json(&rep, &cfg);
        let md = report::redaction_md(&rep);
        if let Some(out) = &cfg.out_dir {
            write_outputs(out, "redaction", &json, &md)?;
        } else if matches!(cfg.format, OutputFormat::Json) {
            println!("{}", serde_json::to_string_pretty(&json)?);
        } else {
            println!("{md}");
        }
        redaction_report = Some(rep);
    }

    if let Some(path) = &args.write_baseline {
        let baseline = report::build_baseline(
            retrieval_report.as_ref(),
            redaction_report.as_ref(),
            args.tolerance,
        );
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        std::fs::write(path, serde_json::to_string_pretty(&baseline)?)?;
        eprintln!("wrote baseline → {}", path.display());
    }

    if cfg.fail_on_regression {
        let baseline_path = cfg
            .baseline
            .clone()
            .ok_or("--fail-on-regression requires --baseline")?;
        let baseline: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&baseline_path)?)?;
        let failures = report::check_regressions(
            &baseline,
            retrieval_report.as_ref(),
            redaction_report.as_ref(),
        );
        if !failures.is_empty() {
            eprintln!("\n❌ REGRESSION GATE FAILED ({} issue(s)):", failures.len());
            for f in &failures {
                eprintln!("  - {f}");
            }
            return Ok(1);
        }
        eprintln!(
            "\n✅ regression gate passed (vs {})",
            baseline_path.display()
        );
    }

    Ok(0)
}

fn real_main() -> EvalResult<i32> {
    let cli = Cli::parse();
    match &cli.command {
        Command::Run(args) => run(args),
        Command::ValidateMetrics => {
            println!(
                "IR metrics are validated against frozen pytrec_eval ground truth by:\n  \
                 cargo test -p grafiki-eval --test metrics_oracle\n\
                 (the oracle fixture lives in crates/grafiki-eval/fixtures/oracle/)."
            );
            Ok(0)
        }
    }
}

fn main() -> ExitCode {
    match real_main() {
        Ok(0) => ExitCode::SUCCESS,
        Ok(code) => ExitCode::from(code as u8),
        Err(e) => {
            eprintln!("grafiki-eval: {e}");
            ExitCode::FAILURE
        }
    }
}
