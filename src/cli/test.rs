use colored::*;
use std::path::Path;

use regent::tester::{FixtureManager, ModuleTester, TestConfig, TestType, TestReporter};
use regent::tester::reporter::ReportFormat;

pub struct TestCommand;

impl TestCommand {
    pub fn execute(
        path: &Path,
        pattern: Option<&str>,
        report: Option<&Path>,
        detail: bool,
    ) -> anyhow::Result<()> {
        let effective_pattern = pattern.unwrap_or("spec/{aliases,classes,defines,functions,hosts,integration,plans,tasks,type_aliases,types,unit}/**/*_spec.rb");
        println!("{} Running tests with pattern: {}", "⚙".cyan(), effective_pattern);

        // Check if spec directory exists
        if !path.join("spec").exists() {
            return Err(anyhow::anyhow!("No spec directory found at {:?}", path));
        }

        let module_path = path
            .canonicalize()
            .unwrap_or_else(|_| path.to_path_buf());

        // Auto-prep fixtures if .fixtures.yml is present and `spec/fixtures/modules/` is bare.
        prep_fixtures_if_needed(&module_path);

        let config = TestConfig::new(module_path, TestType::Unit)
            .with_pattern(Some(effective_pattern.to_string()));
        let tester = ModuleTester::new(config);
        let results = tester.run_tests()?;

        if results.success() {
            println!("{} Tests passed", "✓".green().bold());
        } else {
            println!("{} Tests failed", "✗".red().bold());
        }
        println!(
            "  Passed: {}, Failed: {}, Skipped: {}",
            results.passed, results.failed, results.skipped
        );
        if results.total > 0 && results.skipped == results.total {
            println!("{}", "Artichoke runner skipped all specs (RSpec execution pending).".yellow());
        }

        if detail {
            println!("Test cases:");
            for test_case in &results.test_cases {
                let status = match test_case.status {
                    regent::tester::TestStatus::Passed => "PASS".green().bold(),
                    regent::tester::TestStatus::Failed => "FAIL".red().bold(),
                    regent::tester::TestStatus::Skipped => "SKIP".yellow().bold(),
                    regent::tester::TestStatus::Pending => "PEND".yellow().bold(),
                };
                println!("  {} {}", status, test_case.name);
                if let Some(message) = &test_case.message {
                    println!("    {}", message);
                }
            }
        }

        if let Some(report_path) = report {
            let format = report_format_from_path(report_path);
            TestReporter::write_report(&results, report_path, format)?;
            println!(
                "{} Report written to {}",
                "✓".green().bold(),
                report_path.display()
            );
        }

        if !results.success() {
            if !results.stdout.trim().is_empty() {
                println!("{}", results.stdout.trim_end());
            }
            if !results.stderr.trim().is_empty() {
                println!("{}", results.stderr.trim_end());
            }
            return Err(anyhow::anyhow!("Tests failed"));
        }

        Ok(())
    }
}

fn prep_fixtures_if_needed(module_path: &Path) {
    let fixtures_yml = module_path.join(".fixtures.yml");
    if !fixtures_yml.exists() {
        return;
    }
    let fixtures_dir = module_path.join("spec").join("fixtures").join("modules");
    let mut manager = FixtureManager::new(module_path, &fixtures_dir);
    if let Err(err) = manager.parse_fixtures_yml(&fixtures_yml) {
        eprintln!("warning: parsing .fixtures.yml failed: {err}");
        return;
    }
    match manager.setup_fixtures() {
        Ok(0) => {}
        Ok(n) => println!("{} Prepared {} fixture module(s)", "⚙".cyan(), n),
        Err(err) => eprintln!("warning: fixture setup failed: {err}"),
    }
}

fn report_format_from_path(path: &Path) -> ReportFormat {
    match path.extension().and_then(|ext| ext.to_str()).unwrap_or("") {
        "json" => ReportFormat::Json,
        "html" | "htm" => ReportFormat::Html,
        _ => ReportFormat::Text,
    }
}
