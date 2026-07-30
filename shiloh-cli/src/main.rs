//! Shiloh3D CLI — build, package, import, and automation (pure Rust).

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use shiloh_assets::AssetPackage;
use shiloh_core::{logging, profile};
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
    /// Cook a project into a runnable package directory (assets + manifest)
    Cook {
        /// Project root containing `shiloh.project.json`
        #[arg(short, long, default_value = "shiloh_project")]
        project: PathBuf,
        /// Output directory for the cooked package
        #[arg(short, long, default_value = "dist/package")]
        out: PathBuf,
    },
}

fn main() -> anyhow::Result<()> {
    logging::init();
    profile::install_crash_hook(Some(PathBuf::from("crashes")));
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
            println!("phases: 1–2 complete · 3 packaging/hot-reload/anim · 4 partition/visual IR");
        }
        Commands::Package { name, out } => {
            let pkg = AssetPackage::new(name);
            let text = serde_json::to_string_pretty(&pkg)?;
            std::fs::write(&out, text)?;
            println!("Wrote {}", out.display());
        }
        Commands::Cook { project, out } => {
            let _ = Project::load(&project).or_else(|_| Project::create(&project, "Cooked"))?;
            let pkg = AssetPackage::cook_project(&project, &out, "ShilohPackage")?;
            println!(
                "Cooked {} assets → {} ({})",
                pkg.assets.len(),
                out.display(),
                pkg.name
            );
        }
    }
    Ok(())
}
