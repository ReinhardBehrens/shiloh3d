//! Shiloh3D CLI — build, package, import, and automation (pure Rust).

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use shiloh_assets::AssetPackage;
use shiloh_cli::{PackOptions, pack_desktop};
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
    /// One-click pack: stage Windows · macOS · Ubuntu binaries under dist/desktop/
    ///
    /// Builds the host OS fully; other OS folders get stubs locally.
    /// GitHub Actions `pack-desktop.yml` fills all three (cargo-dist–style matrix).
    Pack {
        /// Workspace root (Cargo.toml)
        #[arg(long, default_value = ".")]
        workspace: PathBuf,
        /// Output directory
        #[arg(short, long, default_value = "dist/desktop")]
        out: PathBuf,
        /// Attempt every triple on this machine (needs linkers / SDKs)
        #[arg(long, default_value_t = false)]
        all_targets: bool,
        /// Debug profile instead of release
        #[arg(long, default_value_t = false)]
        debug: bool,
        /// Extra binary crate names (default: shiloh-demo, shiloh-editor)
        #[arg(long)]
        bin: Vec<String>,
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
            println!("pack: Windows · macOS · Ubuntu → `shiloh-cli pack` / Build → Pack Desktop");
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
        Commands::Pack {
            workspace,
            out,
            all_targets,
            debug,
            bin,
        } => {
            let bins = if bin.is_empty() {
                shiloh_cli::DEFAULT_BINS
                    .iter()
                    .map(|s| (*s).to_string())
                    .collect()
            } else {
                bin
            };
            let opts = PackOptions {
                workspace,
                out_dir: out.clone(),
                bins,
                host_only: !all_targets,
                release: !debug,
            };
            let dir = pack_desktop(&opts)?;
            println!("Desktop pack ready → {}", dir.display());
            println!("Folders: windows-x86_64 · macos-x86_64 · ubuntu-x86_64");
            println!("See {}/README.md", dir.display());
        }
    }
    Ok(())
}
