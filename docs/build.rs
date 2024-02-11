use camino::Utf8PathBuf;
use clap::CommandFactory;
use miette::{Context, IntoDiagnostic};
use std::fs;

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
    use std::{fmt::Write as _, io::Write};

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

    if cmd.get_positionals().count() > 0 {
        help_template.push_str("\n### arguments\n\n```\n{positionals}\n```\n");
    }

    if cmd.get_opts().count() > 0 {
        help_template.push_str("\n### options\n\n```\n{options}\n```\n");
    }

    let mut cmd = cmd.help_template(help_template).max_term_width(80);

    let cmd_docs = cmd.render_long_help();
    write!(&mut main_docs, "\n\n## command-line usage\n\n{cmd_docs}\n").into_diagnostic()?;

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
