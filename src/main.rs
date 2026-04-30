mod api;
mod cli;
mod measure;

fn main() -> anyhow::Result<()> {
    let _args = <cli::Args as clap::Parser>::parse();
    Ok(())
}
