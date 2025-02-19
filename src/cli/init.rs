use crate::{config::get_default_author, consts};
use clap::Parser;
use miette::IntoDiagnostic;
use minijinja::{context, Environment};
use rattler_conda_types::Platform;
use std::io::{Error, ErrorKind, Write};
use std::path::Path;
use std::{fs, path::PathBuf};

/// Creates a new project
#[derive(Parser, Debug)]
pub struct Args {
    /// Where to place the project (defaults to current path)
    #[arg(default_value = ".")]
    pub path: PathBuf,

    /// Channels to use in the project.
    #[arg(short, long = "channel", id = "channel")]
    pub channels: Option<Vec<String>>,

    /// Platforms that the project supports.
    #[arg(short, long = "platform", id = "platform")]
    pub platforms: Vec<String>,
}

/// The default channels to use for a new project.
const DEFAULT_CHANNELS: &[&str] = &["conda-forge"];

/// The pixi.toml template
///
/// This uses a template just to simplify the flexibility of emitting it.
const PROJECT_TEMPLATE: &str = r#"[project]
name = "{{ name }}"
version = "{{ version }}"
description = "Add a short description here"
{%- if author %}
authors = ["{{ author[0] }} <{{ author[1] }}>"]
{%- endif %}
channels = [{%- if channels %}"{{ channels|join("\", \"") }}"{%- endif %}]
platforms = ["{{ platforms|join("\", \"") }}"]

[tasks]

[dependencies]

"#;

const GITIGNORE_TEMPLATE: &str = r#"# pixi environments
.pixi

"#;

const GITATTRIBUTES_TEMPLATE: &str = r#"# GitHub syntax highlighting
pixi.lock linguist-language=YAML

"#;

pub async fn execute(args: Args) -> miette::Result<()> {
    let env = Environment::new();
    let dir = get_dir(args.path).into_diagnostic()?;
    let manifest_path = dir.join(consts::PROJECT_MANIFEST);
    let gitignore_path = dir.join(".gitignore");
    let gitattributes_path = dir.join(".gitattributes");

    // Check if the project file doesn't already exist. We don't want to overwrite it.
    if fs::metadata(&manifest_path).map_or(false, |x| x.is_file()) {
        miette::bail!("{} already exists", consts::PROJECT_MANIFEST);
    }

    // Fail silently if it already exists or cannot be created.
    fs::create_dir_all(&dir).ok();

    // Write pixi.toml
    let name = dir
        .file_name()
        .ok_or_else(|| {
            miette::miette!(
                "Cannot get file or directory name from the path: {}",
                dir.to_string_lossy()
            )
        })?
        .to_string_lossy();
    let version = "0.1.0";
    let author = get_default_author();
    let channels = if let Some(channels) = args.channels {
        channels
    } else {
        DEFAULT_CHANNELS
            .iter()
            .copied()
            .map(ToOwned::to_owned)
            .collect()
    };

    let platforms = if args.platforms.is_empty() {
        vec![Platform::current().to_string()]
    } else {
        args.platforms
    };

    let rv = env
        .render_named_str(
            consts::PROJECT_MANIFEST,
            PROJECT_TEMPLATE,
            context! {
                name,
                version,
                author,
                channels,
                platforms
            },
        )
        .unwrap();
    fs::write(&manifest_path, rv).into_diagnostic()?;

    // create a .gitignore if one is missing
    create_or_append_file(&gitignore_path, GITIGNORE_TEMPLATE)?;

    // create a .gitattributes if one is missing
    create_or_append_file(&gitattributes_path, GITATTRIBUTES_TEMPLATE)?;

    // Emit success
    eprintln!(
        "{}Initialized project in {}",
        console::style(console::Emoji("✔ ", "")).green(),
        dir.display()
    );

    Ok(())
}

// Checks if string is in file.
// If search string is multiline it will check if any of those lines is in the file.
fn string_in_file(path: &Path, search: &str) -> bool {
    let content = fs::read_to_string(path).unwrap_or_default();
    search.lines().any(|line| content.contains(line))
}

// When the specific template is not in the file or the file does not exist.
// Make the file and append the template to the file.
fn create_or_append_file(path: &Path, template: &str) -> miette::Result<()> {
    if !path.is_file() || !string_in_file(path, template) {
        fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(path)
            .into_diagnostic()?
            .write_all(template.as_bytes())
            .into_diagnostic()?;
    }
    Ok(())
}

fn get_dir(path: PathBuf) -> Result<PathBuf, Error> {
    if path.components().count() == 1 {
        Ok(std::env::current_dir().unwrap_or_default().join(path))
    } else {
        path.canonicalize().map_err(|e| match e.kind() {
            ErrorKind::NotFound => Error::new(
                ErrorKind::NotFound,
                format!(
                    "Cannot find '{}' please make sure the folder is reachable",
                    path.to_string_lossy()
                ),
            ),
            _ => Error::new(
                ErrorKind::InvalidInput,
                "Cannot canonicalize the given path",
            ),
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::cli::init::get_dir;
    use std::path::PathBuf;

    #[test]
    fn test_get_name() {
        assert_eq!(
            get_dir(PathBuf::from(".")).unwrap(),
            std::env::current_dir().unwrap()
        );
        assert_eq!(
            get_dir(PathBuf::from("test_folder")).unwrap(),
            std::env::current_dir().unwrap().join("test_folder")
        );
        assert_eq!(
            get_dir(std::env::current_dir().unwrap()).unwrap(),
            std::env::current_dir().unwrap().canonicalize().unwrap()
        );
    }

    #[test]
    fn test_get_name_panic() {
        match get_dir(PathBuf::from("invalid/path")) {
            Ok(_) => panic!("Expected error, but got OK"),
            Err(e) => assert_eq!(e.kind(), std::io::ErrorKind::NotFound),
        }
    }
}
