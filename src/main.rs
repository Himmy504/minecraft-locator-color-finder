use std::{
    io::{self, IsTerminal},
    path::{Path, PathBuf},
    process::ExitCode,
    time::Duration,
};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use indicatif::{ProgressBar, ProgressStyle};
use locator_color_finder::{
    color::{Rgb, rendered_preimages},
    dataset::scan_dataset,
    download::{DATASET_ID, ensure_default_dataset},
    model::NameStatus,
    output::write_matches_csv,
    verify::Verifier,
};

#[derive(Debug, Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Download and integrity-check the default 65.9-million-profile dataset.
    Download {
        /// Redownload even if a valid cached copy already exists.
        #[arg(long)]
        force: bool,
    },
    /// Scan a dataset for UUIDs whose default rendered Locator Bar color matches.
    Scan {
        /// Final rendered RGB color to match.
        #[arg(long, default_value = "#39E6E5")]
        color: Rgb,
        /// A local uuid,username CSV or CSV.zst file. Omit to use the default snapshot.
        #[arg(long)]
        dataset: Option<PathBuf>,
        /// Destination CSV file.
        #[arg(long, default_value = "matches.csv")]
        output: PathBuf,
        /// Keep snapshot usernames and make no requests to Mojang.
        #[arg(long)]
        no_verify_names: bool,
    },
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error:#}");
            ExitCode::from(1)
        }
    }
}

fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Command::Download { force } => {
            let progress = download_progress();
            let path = ensure_default_dataset(force, progress.as_ref())?;
            println!("Dataset ready: {}", path.display());
        }
        Command::Scan {
            color,
            dataset,
            output,
            no_verify_names,
        } => run_scan(color, dataset, &output, !no_verify_names)?,
    }
    Ok(())
}

fn run_scan(
    target: Rgb,
    requested_dataset: Option<PathBuf>,
    output: &Path,
    verify_names: bool,
) -> Result<()> {
    let matcher_progress = spinner("Computing exact rendered-color preimages");
    let target_preimages = rendered_preimages(target);
    if let Some(progress) = matcher_progress {
        progress.finish_with_message(format!(
            "{} internal RGB values render as {target}",
            target_preimages.len()
        ));
    }

    let (dataset_path, dataset_id) = match requested_dataset {
        Some(path) => {
            let id = custom_dataset_id(&path);
            (path, id)
        }
        None => {
            let progress = download_progress();
            let path = ensure_default_dataset(false, progress.as_ref())?;
            (path, DATASET_ID.to_owned())
        }
    };
    if !dataset_path.is_file() {
        anyhow::bail!(
            "dataset does not exist or is not a file: {}",
            dataset_path.display()
        );
    }

    let scan_progress = spinner("Opening profile dataset");
    let mut result = scan_dataset(
        &dataset_path,
        &target_preimages,
        target,
        scan_progress.as_ref(),
    )
    .with_context(|| format!("failed while scanning {}", dataset_path.display()))?;

    if verify_names {
        let progress = finite_progress(result.matches.len() as u64, "Verifying Mojang usernames");
        Verifier::new()?.verify_matches(&mut result.matches, progress.as_ref())?;
    }

    write_matches_csv(output, &result.matches, &dataset_id)?;

    let unchanged = count_status(&result.matches, NameStatus::Unchanged);
    let renamed = count_status(&result.matches, NameStatus::Renamed);
    let not_found = count_status(&result.matches, NameStatus::NotFound);
    let failed = count_status(&result.matches, NameStatus::VerificationFailed);
    println!();
    println!("Scan complete");
    println!("  Dataset: {dataset_id} ({})", dataset_path.display());
    println!("  Profiles scanned: {}", result.rows_scanned);
    println!("  Rendered target: {target}");
    println!("  Internal preimages: {}", target_preimages.len());
    println!("  Unique matches: {}", result.matches.len());
    println!(
        "  Duplicate matching rows ignored: {}",
        result.duplicate_matches
    );
    if verify_names {
        println!(
            "  Name verification: {unchanged} unchanged, {renamed} renamed, {not_found} not found, {failed} failed"
        );
    } else {
        println!("  Name verification: not requested");
    }
    println!("  Output: {}", output.display());
    println!();
    println!(
        "Coverage warning: these are all matches in the selected dataset, not every Minecraft account. Mojang does not expose a complete account-enumeration API."
    );

    for record in result
        .matches
        .iter()
        .filter(|record| record.name_status == NameStatus::VerificationFailed)
    {
        eprintln!(
            "warning: username verification failed for {}: {}",
            record.uuid,
            record
                .verification_error
                .as_deref()
                .unwrap_or("unknown error")
        );
    }
    Ok(())
}

fn count_status(records: &[locator_color_finder::model::MatchRecord], status: NameStatus) -> usize {
    records
        .iter()
        .filter(|record| record.name_status == status)
        .count()
}

fn custom_dataset_id(path: &Path) -> String {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("unnamed");
    format!("custom:{name}")
}

fn progress_enabled() -> bool {
    io::stderr().is_terminal()
}

fn spinner(message: &str) -> Option<ProgressBar> {
    if !progress_enabled() {
        return None;
    }
    let progress = ProgressBar::new_spinner();
    progress.set_style(
        ProgressStyle::with_template("{spinner:.cyan} {msg}")
            .expect("static progress template must be valid"),
    );
    progress.enable_steady_tick(Duration::from_millis(100));
    progress.set_message(message.to_owned());
    Some(progress)
}

fn download_progress() -> Option<ProgressBar> {
    if !progress_enabled() {
        return None;
    }
    let progress = ProgressBar::new(0);
    progress.set_style(
        ProgressStyle::with_template(
            "{spinner:.cyan} {msg} [{bar:36.cyan/blue}] {bytes}/{total_bytes} ({eta})",
        )
        .expect("static progress template must be valid")
        .progress_chars("=>-"),
    );
    Some(progress)
}

fn finite_progress(length: u64, message: &str) -> Option<ProgressBar> {
    if !progress_enabled() {
        return None;
    }
    let progress = ProgressBar::new(length);
    progress.set_style(
        ProgressStyle::with_template("{spinner:.cyan} {msg} [{bar:36.cyan/blue}] {pos}/{len}")
            .expect("static progress template must be valid")
            .progress_chars("=>-"),
    );
    progress.set_message(message.to_owned());
    Some(progress)
}
