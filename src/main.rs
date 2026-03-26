use std::process;

use audio_book_server::{
    CliOverrides, ConfigError, MEDIA_BASE_URL_ENV, PODCAST_DESCRIPTION_ENV, PODCAST_LINK_ENV,
    PODCAST_TITLE_ENV, RSS_SELF_URL_ENV, load_config_from_env,
};

const HELP_TEXT: &str = "\
Usage: audio-book-server [OPTIONS]

Required configuration (env vars or CLI overrides):
  MEDIA_BASE_URL, PODCAST_TITLE, PODCAST_LINK, PODCAST_DESCRIPTION, RSS_SELF_URL

Options:
  --media-base-url <URL>        Override MEDIA_BASE_URL
  --podcast-title <TEXT>        Override PODCAST_TITLE
  --podcast-link <URL>          Override PODCAST_LINK
  --podcast-description <TEXT>  Override PODCAST_DESCRIPTION
  --rss-self-url <URL>          Override RSS_SELF_URL
  --print-config                Print effective configuration and exit
  --help                        Show this help message
";

fn parse_value_arg(
    args: &[String],
    index: &mut usize,
    flag: &str,
    target: &mut Option<String>,
) -> Result<(), String> {
    let current = &args[*index];
    if let Some((_, value)) = current.split_once('=') {
        *target = Some(value.to_owned());
        return Ok(());
    }

    *index += 1;
    let next = args
        .get(*index)
        .ok_or_else(|| format!("missing value for {flag}"))?;
    *target = Some(next.to_owned());
    Ok(())
}

fn parse_cli_args(args: &[String]) -> Result<CliOverrides, String> {
    let mut cli = CliOverrides::default();
    let mut index = 0usize;

    while index < args.len() {
        let arg = &args[index];
        match arg.as_str() {
            "--help" => {
                println!("{HELP_TEXT}");
                process::exit(0);
            }
            "--print-config" => {
                cli.print_config = true;
            }
            flag if flag.starts_with("--media-base-url") => {
                parse_value_arg(args, &mut index, "--media-base-url", &mut cli.media_base_url)?;
            }
            flag if flag.starts_with("--podcast-title") => {
                parse_value_arg(args, &mut index, "--podcast-title", &mut cli.podcast_title)?;
            }
            flag if flag.starts_with("--podcast-link") => {
                parse_value_arg(args, &mut index, "--podcast-link", &mut cli.podcast_link)?;
            }
            flag if flag.starts_with("--podcast-description") => {
                parse_value_arg(
                    args,
                    &mut index,
                    "--podcast-description",
                    &mut cli.podcast_description,
                )?;
            }
            flag if flag.starts_with("--rss-self-url") => {
                parse_value_arg(args, &mut index, "--rss-self-url", &mut cli.rss_self_url)?;
            }
            _ => {
                return Err(format!("unknown argument: {arg}"));
            }
        }
        index += 1;
    }

    Ok(cli)
}

fn print_config_error(err: ConfigError) {
    eprintln!("Configuration error: {err}");
    eprintln!(
        "Required keys: {MEDIA_BASE_URL_ENV}, {PODCAST_TITLE_ENV}, {PODCAST_LINK_ENV}, {PODCAST_DESCRIPTION_ENV}, {RSS_SELF_URL_ENV}"
    );
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cli = match parse_cli_args(&args) {
        Ok(cli) => cli,
        Err(err) => {
            eprintln!("Argument error: {err}");
            eprintln!("Use --help for usage.");
            process::exit(2);
        }
    };

    let config = match load_config_from_env(&cli) {
        Ok(cfg) => cfg,
        Err(err) => {
            print_config_error(err);
            process::exit(2);
        }
    };

    if cli.print_config {
        println!("{}", config.to_pretty_string());
        return;
    }

    println!("Configuration loaded successfully.");
}
