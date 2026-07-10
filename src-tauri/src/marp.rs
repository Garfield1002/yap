//! Narrow bridge to Marp CLI. Plugins may export a deck through this command,
//! but they never receive a generic shell/spawn primitive.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use tempfile::{Builder, NamedTempFile};

const FORMATS: &[&str] = &["html", "pdf", "pptx"];

fn output_format(path: &Path) -> Result<&str, String> {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "Marp output must end in .html, .pdf, or .pptx".to_owned())?;
    FORMATS
        .iter()
        .copied()
        .find(|format| extension.eq_ignore_ascii_case(format))
        .ok_or_else(|| "Marp output must end in .html, .pdf, or .pptx".to_owned())
}

fn source_file(markdown: &str, document_dir: &str) -> Result<NamedTempFile, String> {
    let requested = PathBuf::from(document_dir);
    let mut source = if !document_dir.is_empty() && requested.is_dir() {
        Builder::new()
            .prefix(".bulletmd-marp-")
            .suffix(".md")
            .tempfile_in(&requested)
    } else {
        Builder::new().prefix("bulletmd-marp-").suffix(".md").tempfile()
    }
    .map_err(|e| format!("creating temporary Marp source: {e}"))?;
    source
        .write_all(markdown.as_bytes())
        .map_err(|e| format!("writing temporary Marp source: {e}"))?;
    source
        .flush()
        .map_err(|e| format!("flushing temporary Marp source: {e}"))?;
    Ok(source)
}

fn concise_output(bytes: &[u8]) -> String {
    const LIMIT: usize = 8 * 1024;
    let start = bytes.len().saturating_sub(LIMIT);
    String::from_utf8_lossy(&bytes[start..]).trim().to_owned()
}

fn run_command(command: &mut Command) -> Result<Output, String> {
    let result = command.output().map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            "Marp CLI was not found. Install `@marp-team/marp-cli` and ensure `marp` is on PATH."
                .to_owned()
        } else {
            format!("starting Marp CLI: {error}")
        }
    })?;
    if result.status.success() {
        return Ok(result);
    }

    let detail = concise_output(if result.stderr.is_empty() {
        &result.stdout
    } else {
        &result.stderr
    });
    let code = result
        .status
        .code()
        .map_or_else(|| "signal".to_owned(), |c| c.to_string());
    if detail.is_empty() {
        Err(format!("Marp CLI failed ({code})"))
    } else {
        Err(format!("Marp CLI failed ({code}): {detail}"))
    }
}

fn marp_command(source: &Path, document_dir: &str, allow_local_files: bool) -> Command {
    let mut command = Command::new("marp");
    command.arg(source);
    if allow_local_files {
        command.arg("--allow-local-files");
    }
    if !document_dir.is_empty() && Path::new(document_dir).is_dir() {
        command.current_dir(document_dir);
    }
    command
}

fn run_export(
    markdown: String,
    output: String,
    document_dir: String,
    allow_local_files: bool,
) -> Result<(), String> {
    let output_path = PathBuf::from(&output);
    output_format(&output_path)?;
    if let Some(parent) = output_path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
    {
        if !parent.is_dir() {
            return Err(format!(
                "output directory does not exist: {}",
                parent.display()
            ));
        }
    }

    let source = source_file(&markdown, &document_dir)?;
    let mut command = marp_command(source.path(), &document_dir, allow_local_files);
    command.arg("--output").arg(&output_path);
    run_command(&mut command).map(|_| ())
}

/// Export the current in-memory buffer, so the result never races autosave.
/// The temporary source lives beside a saved document to preserve relative
/// asset and theme resolution. It is deleted as soon as Marp exits. Marp can
/// take several seconds for browser-backed formats, so it runs off the UI
/// thread.
#[tauri::command]
pub async fn marp_export(
    markdown: String,
    output: String,
    document_dir: String,
    allow_local_files: bool,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        run_export(markdown, output, document_dir, allow_local_files)
    })
    .await
    .map_err(|error| format!("waiting for Marp CLI: {error}"))?
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn accepts_only_supported_output_extensions() {
        assert_eq!(output_format(Path::new("deck.html")).unwrap(), "html");
        assert_eq!(output_format(Path::new("deck.PDF")).unwrap(), "pdf");
        assert_eq!(output_format(Path::new("deck.pptx")).unwrap(), "pptx");
        assert!(output_format(Path::new("deck.md")).is_err());
        assert!(output_format(Path::new("deck")).is_err());
    }

    #[test]
    fn temporary_source_uses_document_directory_and_is_removed() {
        let dir = tempfile::tempdir().unwrap();
        let path;
        {
            let source = source_file("# Slide\n", dir.path().to_str().unwrap()).unwrap();
            path = source.path().to_owned();
            assert_eq!(source.path().parent(), Some(dir.path()));
            assert_eq!(fs::read_to_string(source.path()).unwrap(), "# Slide\n");
        }
        assert!(!path.exists());
    }
}
