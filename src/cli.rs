use clap::Parser;

/// Measure internet speed against fast.com.
#[derive(Parser, Debug, PartialEq, Eq)]
#[command(version, about)]
pub struct Args {
    /// Emit a single JSON object instead of human-friendly output.
    #[cfg_attr(feature = "tui", arg(long, conflicts_with_all = ["single_line", "tui"]))]
    #[cfg_attr(not(feature = "tui"), arg(long, conflicts_with_all = ["single_line"]))]
    pub json: bool,

    /// Skip the upload phase.
    #[arg(long = "no-upload")]
    pub no_upload: bool,

    /// Print one human-readable line instead of the multi-line summary.
    #[cfg_attr(feature = "tui", arg(long = "single-line", conflicts_with_all = ["json", "tui"]))]
    #[cfg_attr(not(feature = "tui"), arg(long = "single-line", conflicts_with_all = ["json"]))]
    pub single_line: bool,

    /// Run an interactive TUI with live charts.
    #[cfg(feature = "tui")]
    #[arg(long, conflicts_with_all = ["json", "single_line"])]
    pub tui: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_all_false() {
        let args = Args::try_parse_from(["fastrs"]).unwrap();
        assert!(!args.json);
        assert!(!args.no_upload);
        assert!(!args.single_line);
        #[cfg(feature = "tui")]
        assert!(!args.tui);
    }

    #[test]
    fn parses_no_upload_alone() {
        let args = Args::try_parse_from(["fastrs", "--no-upload"]).unwrap();
        assert!(args.no_upload);
    }

    #[test]
    fn parses_json_with_no_upload() {
        let args = Args::try_parse_from(["fastrs", "--json", "--no-upload"]).unwrap();
        assert!(args.json && args.no_upload);
    }

    #[cfg(feature = "tui")]
    #[test]
    fn parses_tui_alone() {
        let args = Args::try_parse_from(["fastrs", "--tui"]).unwrap();
        assert!(args.tui);
    }

    #[test]
    fn json_conflicts_with_single_line() {
        let r = Args::try_parse_from(["fastrs", "--json", "--single-line"]);
        assert!(r.is_err());
    }

    #[cfg(feature = "tui")]
    #[test]
    fn tui_conflicts_with_json() {
        let r = Args::try_parse_from(["fastrs", "--tui", "--json"]);
        assert!(r.is_err());
    }
}
