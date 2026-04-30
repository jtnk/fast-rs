mod api;
mod cli;

fn main() -> anyhow::Result<()> {
    let _args = <cli::Args as clap::Parser>::parse();
    Ok(())
}
