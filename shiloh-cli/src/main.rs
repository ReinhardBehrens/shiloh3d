//! Shiloh3D CLI — build, package, import, and automation (pure Rust).

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use shiloh_assets::AssetPackage;
use shiloh_core::logging;
use shiloh_editor::Project;

#[derive(Parser, Debug)]
#[command(name = "shiloh-cli", version, about = "Shiloh3D tooling")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Create a new Shiloh project
    New {
        name: String,
        #[arg(short, long, default_value = ".")]
        path: PathBuf,
    },
    /// Print engine / workspace info
    Info,
    /// Write an empty asset package manifest
    Package {
        name: String,
        #[arg(short, long, default_value = "package.json")]
        out: PathBuf,
    },
}

fn main() -> anyhow::Result<()> {
    logging::init();
    let cli = Cli::parse();
    match cli.command {
        Commands::New { name, path } => {
            let root = path.join(&name);
            let project = Project::create(&root, &name)?;
            println!(
                "Created project '{}' at {}",
                project.manifest.name,
                root.display()
            );
        }
        Commands::Info => {
            println!("Shiloh3D {}", env!("CARGO_PKG_VERSION"));
            println!("edition: Rust workspace (pure-Rust default backends)");
        }
        Commands::Package { name, out } => {
            let pkg = AssetPackage::new(name);
            let text = serde_json::to_string_pretty(&pkg)?;
            std::fs::write(&out, text)?;
            println!("Wrote {}", out.display());
        }
    }
    Ok(())
}
