// Regent CLI - Rust + Artichoke Ruby powered Puppet module development kit

mod cli;
mod artichoke_runtime;

use clap::{Parser, Subcommand};
use colored::*;
use std::path::PathBuf;

use cli::{NewCommand, GenerateCommand, ValidateCommand, BuildCommand, TestCommand, BootstrapCommand};

#[derive(Parser)]
#[command(
    name = "regent",
    version = env!("CARGO_PKG_VERSION"),
    about = "Regent - Rust + Artichoke Ruby powered Puppet Development Kit",
    long_about = "Regent is an alternative development kit for Puppet modules with native Rust performance and full Ruby gem compatibility through Artichoke Ruby."
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    #[arg(global = true, short, long, help = "Enable verbose output")]
    verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a new Puppet module
    New {
        #[arg(help = "Name of the module to create")]
        name: String,

        #[arg(long, help = "Author name")]
        author: Option<String>,

        #[arg(long, help = "License type", default_value = "Apache-2.0")]
        license: String,

        #[arg(long, help = "Module description")]
        description: Option<String>,
    },

    /// Generate component (class, task, plan, etc)
    #[command(subcommand)]
    Generate(GenerateSubcommands),

    /// Validate module syntax and structure
    Validate {
        #[arg(help = "Path to module to validate", default_value = ".")]
        path: PathBuf,
    },

    /// Build module package
    Build {
        #[arg(help = "Path to module", default_value = ".")]
        path: PathBuf,

        #[arg(long, help = "Output directory")]
        output: Option<PathBuf>,
    },

    /// Run tests
    Test {
        #[arg(help = "Path to module", default_value = ".")]
        path: PathBuf,

        #[arg(short, long, help = "Test pattern")]
        pattern: Option<String>,

        #[arg(long, help = "Write test report to path")]
        report: Option<PathBuf>,

        #[arg(long, help = "Show detailed test case output")]
        detail: bool,
    },

    /// Install required gems and runtime dependencies for Regent
    Bootstrap {
        #[arg(help = "Path to module", default_value = ".")]
        path: PathBuf,

        #[arg(long, help = "Overwrite an existing Gemfile with a Regent-managed one")]
        force: bool,
    },

    /// Show version
    Version,
}

#[derive(Subcommand)]
enum GenerateSubcommands {
    /// Generate a new class
    Class {
        #[arg(help = "Class name")]
        name: String,

        #[arg(long, help = "Module path", default_value = ".")]
        module_path: PathBuf,
    },

    /// Generate a new task
    Task {
        #[arg(help = "Task name")]
        name: String,

        #[arg(long, help = "Module path", default_value = ".")]
        module_path: PathBuf,

        #[arg(long, help = "Task type", default_value = "ruby")]
        task_type: String,
    },

    /// Generate a new plan
    Plan {
        #[arg(help = "Plan name")]
        name: String,

        #[arg(long, help = "Module path", default_value = ".")]
        module_path: PathBuf,
    },
}

fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .init();

    let cli = Cli::parse();

    if cli.verbose {
        log::set_max_level(log::LevelFilter::Debug);
    }

    match cli.command {
        Commands::New {
            name,
            author,
            license,
            description,
        } => {
            println!(
                "{}",
                format!("Creating new Puppet module: {}", name).cyan().bold()
            );
            NewCommand::execute(&name, author.as_deref(), &license, description.as_deref())?;
        }

        Commands::Generate(subcmd) => match subcmd {
            GenerateSubcommands::Class { name, module_path } => {
                println!("{}", format!("Generating class: {}", name).cyan());
                GenerateCommand::class(&name, &module_path)?;
            }
            GenerateSubcommands::Task {
                name,
                module_path,
                task_type,
            } => {
                println!("{}", format!("Generating task: {}", name).cyan());
                GenerateCommand::task(&name, &module_path, &task_type)?;
            }
            GenerateSubcommands::Plan { name, module_path } => {
                println!("{}", format!("Generating plan: {}", name).cyan());
                GenerateCommand::plan(&name, &module_path)?;
            }
        },

        Commands::Validate { path } => {
            println!(
                "{}",
                format!("Validating module at: {:?}", path).cyan()
            );
            ValidateCommand::execute(&path)?;
        }

        Commands::Build { path, output } => {
            println!("{}", format!("Building module at: {:?}", path).cyan());
            BuildCommand::execute(&path, output.as_deref())?;
        }

        Commands::Test {
            path,
            pattern,
            report,
            detail,
        } => {
            println!("{}", format!("Running tests at: {:?}", path).cyan());
            TestCommand::execute(&path, pattern.as_deref(), report.as_deref(), detail)?;
        }

        Commands::Bootstrap { path, force } => {
            BootstrapCommand::execute(&path, force)?;
        }

        Commands::Version => {
            println!("Regent {}", env!("CARGO_PKG_VERSION"));
            println!("Built with Rust + Artichoke Ruby");
        }
    }

    Ok(())
}
