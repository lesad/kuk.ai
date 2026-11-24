//! `kuk schema`: the tool describing itself, built from the clap definitions
//! and from real payloads so it cannot drift from what the binary does.

use std::path::PathBuf;

use serde_json::{Value, json};

use crate::cli::{self, EXIT_CODES};
use crate::report::{ImageInfo, Mismatch, Report, SCHEMA_VERSION};

pub fn build() -> Value {
    let root = cli::command();
    let mut commands = vec![describe(&root)];
    commands.extend(root.get_subcommands().map(describe));

    let exit_codes: Vec<Value> = EXIT_CODES
        .iter()
        .map(|(code, meaning)| json!({ "code": code, "meaning": meaning }))
        .collect();

    json!({
        "schema_version": SCHEMA_VERSION,
        "name": root.get_name(),
        "version": env!("CARGO_PKG_VERSION"),
        "commands": commands,
        "exit_codes": exit_codes,
        "output": {
            "formats": ["text", "json", "toon"],
            "report_example": example_report(),
            "dimension_mismatch_example": example_mismatch(),
        },
    })
}

fn describe(cmd: &clap::Command) -> Value {
    let args: Vec<Value> = cmd
        .get_arguments()
        .filter(|arg| !matches!(arg.get_id().as_str(), "help" | "version"))
        .map(|arg| {
            let values: Vec<String> = arg
                .get_possible_values()
                .iter()
                .filter(|v| !v.is_hide_set())
                .map(|v| v.get_name().to_string())
                .collect();
            let defaults: Vec<String> = arg
                .get_default_values()
                .iter()
                .map(|v| v.to_string_lossy().into_owned())
                .collect();
            json!({
                "name": arg.get_id().as_str(),
                "long": arg.get_long(),
                "short": arg.get_short(),
                "positional": arg.is_positional(),
                "required": arg.is_required_set(),
                "takes_value": arg.get_action().takes_values(),
                "default": defaults.first(),
                "values": values,
                "help": arg.get_help().map(ToString::to_string),
            })
        })
        .collect();

    json!({
        "name": cmd.get_name(),
        "about": cmd.get_about().map(ToString::to_string),
        "args": args,
    })
}

fn example_info(path: &str, width: u32, height: u32) -> ImageInfo {
    ImageInfo {
        path: PathBuf::from(path),
        width,
        height,
    }
}

fn example_report() -> Report {
    Report {
        schema_version: SCHEMA_VERSION,
        a: example_info("design.png", 1600, 1200),
        b: example_info("impl.png", 1600, 1200),
        dims_match: true,
        score: 0.9958,
        threshold: 0.99,
        passed: true,
        diff_path: Some(PathBuf::from("diff.png")),
    }
}

fn example_mismatch() -> Mismatch {
    Mismatch::new(
        PathBuf::from("design.png"),
        PathBuf::from("impl.png"),
        (1600, 1200),
        (1600, 1240),
    )
}
