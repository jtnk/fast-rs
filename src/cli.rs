use clap::Parser;

/// Measure internet speed against fast.com.
#[derive(Parser, Debug, PartialEq, Eq)]
#[command(version, about)]
pub struct Args {
    /// Emit a single JSON object instead of human-friendly output.
    #[arg(long)]
    pub json: bool,

    /// Skip the upload phase.
    #[arg(long = "no-upload")]
    pub no_upload: bool,

    /// Print one human-readable line, no live updates.
    #[arg(long = "single-line")]
    pub single_line: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_all_false() {
        let args = Args::try_parse_from(["fastrs"]).unwrap();
        assert_eq!(
            args,
            Args {
                json: false,
                no_upload: false,
                single_line: false
            }
        );
    }

    #[test]
    fn parses_all_flags() {
        let args =
            Args::try_parse_from(["fastrs", "--json", "--no-upload", "--single-line"]).unwrap();
        assert!(args.json && args.no_upload && args.single_line);
    }
}
