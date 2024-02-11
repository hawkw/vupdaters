use camino::Utf8PathBuf;
use clap::CommandFactory;
use miette::{Context, IntoDiagnostic};
use std::fs;

use std::{fmt::Write as _, io::Write};

fn main() -> miette::Result<()> {
    let workspace_root = cargo_metadata::MetadataCommand::new()
        .exec()
        .into_diagnostic()
        .context("failed to execute cargo metadata")?
        .workspace_root;

    let docs_path = workspace_root.join("docs");
    let md_path = docs_path.join("mdbook");
    let out_path = docs_path.join("rendered");

    let dialctl_cmd = vupdaters::dialctl::Args::command();
    render_command(dialctl_cmd, &md_path, &out_path)
        .with_context(|| format!("failed to render `dialctl` docs to {md_path}"))?;

    let vupdated_cmd = vupdaters::daemon::Args::command();
    render_command(vupdated_cmd, &md_path, &out_path)
        .with_context(|| format!("failed to render `vupdated` docs to {md_path}"))?;

    for file in fs::read_dir(&md_path)
        .into_diagnostic()
        .with_context(|| format!("failed to read {md_path}"))?
    {
        let file = file
            .into_diagnostic()
            .with_context(|| format!("failed to read {md_path}"))?;
        let file_name = file.file_name();
        let file_name = file_name.to_string_lossy();
        let path = file.path();
        if file_name.ends_with(".md") {
            if file_name == "dialctl.md" || file_name == "vupdated.md" {
                continue;
            }
            let out_file = out_path.join(file_name.as_ref());
            println!("cargo:rerun-if-changed={}", path.display());
            fs::copy(&path, &out_file)
                .into_diagnostic()
                .with_context(|| format!("failed to copy {} to {out_file}", path.display()))?;
        }
    }

    Ok(())
}

fn render_command(
    cmd: clap::Command,
    md_path: &Utf8PathBuf,
    out_path: &Utf8PathBuf,
) -> miette::Result<()> {
    fs::create_dir_all(out_path)
        .into_diagnostic()
        .with_context(|| format!("failed to create {out_path}"))?;

    let name = cmd.get_name().to_string();
    let cmd_in = md_path.join(&name).with_extension("md");
    println!("cargo:rerun-if-changed={cmd_in}");
    let mut main_docs = fs::read_to_string(&cmd_in)
        .into_diagnostic()
        .with_context(|| format!("failed to read {cmd_in}"))?;
    let mut help_template = "\
        {about}\n\n\
        ```\n\
        {usage}\n\
        ```\n\n\
    "
    .to_owned();

    if cmd.has_subcommands() {
        help_template.push_str("### subcommands\n\n```\n{subcommands}\n```\n");
    }

    let mut cmd = cmd.help_template(help_template).max_term_width(80);

    let cmd_docs = cmd.render_long_help();
    write!(&mut main_docs, "\n\n## command-line usage\n\n{cmd_docs}\n").into_diagnostic()?;

    if cmd.get_positionals().count() > 0 {
        main_docs.push_str("\n### arguments\n\n");
        for arg in cmd.get_positionals() {
            write_arg(&mut main_docs, arg).context("failed to format positional arg")?;
        }
    }

    if cmd.get_opts().count() > 0 {
        main_docs.push_str("\n### options\n\n");
        for arg in cmd.get_opts() {
            write_arg(&mut main_docs, arg).context("failed to format option")?;
        }
    }

    let cmd_out = out_path.join(&name).with_extension("md");
    fs::File::create(&cmd_out)
        .into_diagnostic()
        .with_context(|| format!("failed to create {cmd_out}"))?
        .write_all(main_docs.as_bytes())
        .into_diagnostic()
        .with_context(|| format!("failed to write to {cmd_out}"))?;

    let submd_path = md_path.join(&name);
    let subout_path = out_path.join(&name);

    for subcmd in cmd.get_subcommands() {
        if subcmd.get_name() == "help" {
            continue;
        }
        render_command(subcmd.clone(), &submd_path, &subout_path)
            .with_context(|| format!("failed to render docs for {md_path}"))?;
    }
    Ok(())
}

fn write_arg(rendered: &mut String, arg: &clap::Arg) -> miette::Result<()> {
    rendered.push_str(" - ");
    if let Some(short) = arg.get_short() {
        write!(
            rendered,
            "`-{short}`{}",
            if arg.get_long().is_some() { ", " } else { "" }
        )
        .into_diagnostic()?;
    };
    if let Some(long) = arg.get_long() {
        write!(rendered, "`--{long}`").into_diagnostic()?;
    }

    let num_vals = arg.get_num_args().unwrap_or_else(|| 1.into());

    let mut val_names = match arg.get_value_names() {
        Some(names) => names.iter().map(|s| s.to_string()).collect(),
        None => vec![arg.get_id().as_str().to_ascii_uppercase()],
    };
    if val_names.len() == 1 {
        let min = num_vals.min_values().max(1);
        let val_name = val_names.pop().unwrap();
        val_names = vec![val_name; min];
    }
    if !val_names.is_empty() {
        rendered.push_str(" `");
        for (n, val_name) in val_names.iter().enumerate() {
            if n > 0 {
                rendered.push(' ');
            }
            if arg.is_positional() && (num_vals.min_values() == 0 || !arg.is_required_set()) {
                write!(rendered, "[{val_name}]").into_diagnostic()?;
            } else {
                write!(rendered, "<{val_name}>").into_diagnostic()?;
            };
        }

        let mut extra_values = false;
        extra_values |= val_names.len() < num_vals.max_values();
        if arg.is_positional() && matches!(*arg.get_action(), clap::ArgAction::Append) {
            extra_values = true;
        }
        if extra_values {
            rendered.push_str("...");
        }
        rendered.push('`');
    }

    let help = arg.get_long_help().or_else(|| arg.get_help());
    if let Some(help) = help {
        let help = help.to_string();
        writeln!(rendered, ": {}", help.replace('\n', " ")).into_diagnostic()?;
    } else {
        rendered.push('\n');
    }

    let defaults = arg.get_default_values();
    if !defaults.is_empty() {
        write!(rendered, "    - **default:** ").into_diagnostic()?;
        for val in defaults {
            write!(rendered, "`{}` ", val.to_string_lossy()).into_diagnostic()?;
        }
        rendered.push('\n');
    }
    let possible = arg.get_possible_values();
    if !possible.is_empty() {
        writeln!(rendered, "    - **possible values:** ").into_diagnostic()?;
        for val in possible {
            write!(rendered, "      - `{}` ", val.get_name()).into_diagnostic()?;
            if let Some(help) = val.get_help() {
                let help = help.to_string();
                writeln!(rendered, ": {}", help.replace('\n', " ")).into_diagnostic()?;
            } else {
                rendered.push('\n');
            }
        }
        rendered.push('\n');
    }

    if let Some(env) = arg.get_env() {
        writeln!(rendered, "    - **env:** `{}`", env.to_string_lossy()).into_diagnostic()?;
    }

    Ok(())
}
