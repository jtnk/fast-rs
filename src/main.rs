use anyhow::Result;
use clap::Parser;
use fastrs::{api, cli, measure, output};

#[tokio::main]
async fn main() -> Result<()> {
    let args = cli::Args::parse();
    let client = reqwest::Client::builder()
        .user_agent("fastrs/0.1")
        .build()?;

    if !args.json && !args.single_line {
        eprintln!("Connecting to fast.com...");
    }

    let token = api::fetch_token_default(&client).await?;
    let targets = api::fetch_targets_default(&client, &token, 5).await?;

    #[cfg(feature = "tui")]
    if args.tui {
        let report = fastrs::tui::run(
            &client,
            &targets,
            &measure::Options {
                no_upload: args.no_upload,
            },
        )
        .await?;
        output::render_summary(&report);
        return Ok(());
    }

    let report = measure::run(
        &client,
        &targets,
        &measure::Options {
            no_upload: args.no_upload,
        },
    )
    .await?;

    if args.json {
        output::render_json(&report)?;
    } else if args.single_line {
        println!("{}", output::render_single_line(&report));
    } else {
        output::render_summary(&report);
    }
    Ok(())
}
