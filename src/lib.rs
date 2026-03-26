use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, utf8_percent_encode};

const PATH_SEGMENT_ENCODE_SET: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'.')
    .remove(b'_')
    .remove(b'~');

pub const MEDIA_BASE_URL_ENV: &str = "MEDIA_BASE_URL";
pub const PODCAST_TITLE_ENV: &str = "PODCAST_TITLE";
pub const PODCAST_LINK_ENV: &str = "PODCAST_LINK";
pub const PODCAST_DESCRIPTION_ENV: &str = "PODCAST_DESCRIPTION";
pub const RSS_SELF_URL_ENV: &str = "RSS_SELF_URL";

#[derive(Debug, PartialEq, Eq)]
pub enum UrlBuildError {
    EmptyBaseUrl,
    EmptyDirectory,
    EmptyFileName,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CliOverrides {
    pub media_base_url: Option<String>,
    pub podcast_title: Option<String>,
    pub podcast_link: Option<String>,
    pub podcast_description: Option<String>,
    pub rss_self_url: Option<String>,
    pub print_config: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppConfig {
    pub media_base_url: String,
    pub podcast_title: String,
    pub podcast_link: String,
    pub podcast_description: String,
    pub rss_self_url: String,
}

impl AppConfig {
    pub fn to_pretty_string(&self) -> String {
        format!(
            "Effective configuration:\n\
             - {MEDIA_BASE_URL_ENV}={}\n\
             - {PODCAST_TITLE_ENV}={}\n\
             - {PODCAST_LINK_ENV}={}\n\
             - {PODCAST_DESCRIPTION_ENV}={}\n\
             - {RSS_SELF_URL_ENV}={}",
            self.media_base_url,
            self.podcast_title,
            self.podcast_link,
            self.podcast_description,
            self.rss_self_url
        )
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum ConfigError {
    MissingOrEmpty(&'static str),
}

impl core::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::MissingOrEmpty(name) => {
                write!(
                    f,
                    "required configuration '{name}' is missing or empty (set env var or CLI override)"
                )
            }
        }
    }
}

fn get_required_value(
    cli_value: &Option<String>,
    env_name: &'static str,
    get_env: &impl Fn(&str) -> Option<String>,
) -> Result<String, ConfigError> {
    let candidate = cli_value
        .as_deref()
        .map(str::to_owned)
        .or_else(|| get_env(env_name));

    match candidate {
        Some(value) => {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                Err(ConfigError::MissingOrEmpty(env_name))
            } else {
                Ok(trimmed.to_owned())
            }
        }
        None => Err(ConfigError::MissingOrEmpty(env_name)),
    }
}

pub fn load_config_from_sources(
    cli: &CliOverrides,
    get_env: impl Fn(&str) -> Option<String>,
) -> Result<AppConfig, ConfigError> {
    Ok(AppConfig {
        media_base_url: get_required_value(&cli.media_base_url, MEDIA_BASE_URL_ENV, &get_env)?,
        podcast_title: get_required_value(&cli.podcast_title, PODCAST_TITLE_ENV, &get_env)?,
        podcast_link: get_required_value(&cli.podcast_link, PODCAST_LINK_ENV, &get_env)?,
        podcast_description: get_required_value(
            &cli.podcast_description,
            PODCAST_DESCRIPTION_ENV,
            &get_env,
        )?,
        rss_self_url: get_required_value(&cli.rss_self_url, RSS_SELF_URL_ENV, &get_env)?,
    })
}

pub fn load_config_from_env(cli: &CliOverrides) -> Result<AppConfig, ConfigError> {
    load_config_from_sources(cli, |name| std::env::var(name).ok())
}

fn encode_path_segment(value: &str) -> String {
    utf8_percent_encode(value, PATH_SEGMENT_ENCODE_SET).to_string()
}

/// Builds a media URL in the form: {base_url}/{dir}/{file}
///
/// Examples:
/// - https://media.example.com/A/chapter-01.mp3
/// - https://media.example.com/A/chapter%2001.mp3
pub fn build_media_url(base_url: &str, directory: &str, file_name: &str) -> Result<String, UrlBuildError> {
    let trimmed_base = base_url.trim().trim_end_matches('/');
    if trimmed_base.is_empty() {
        return Err(UrlBuildError::EmptyBaseUrl);
    }

    let trimmed_directory = directory.trim().trim_matches('/');
    if trimmed_directory.is_empty() {
        return Err(UrlBuildError::EmptyDirectory);
    }

    let trimmed_file_name = file_name.trim().trim_matches('/');
    if trimmed_file_name.is_empty() {
        return Err(UrlBuildError::EmptyFileName);
    }

    let encoded_directory = encode_path_segment(trimmed_directory);
    let encoded_file_name = encode_path_segment(trimmed_file_name);

    Ok(format!(
        "{trimmed_base}/{encoded_directory}/{encoded_file_name}"
    ))
}

/// Returns file names sorted in ascending lexical order.
///
/// The first element in the returned list should be treated as the oldest
/// release for RSS ordering semantics, and the last element as the newest.
pub fn sort_files_lexical(files: &[String]) -> Vec<String> {
    let mut sorted = files.to_vec();
    sorted.sort();
    sorted
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_media_url_from_prefix_directory_and_filename() {
        let url = build_media_url("https://media.example.com", "A", "chapter-01.mp3")
            .expect("valid URL parts");
        assert_eq!(url, "https://media.example.com/A/chapter-01.mp3");
    }

    #[test]
    fn trims_extra_slashes_and_whitespace() {
        let url = build_media_url(" https://media.example.com/ ", "/A/", "/chapter-01.mp3/")
            .expect("valid URL parts");
        assert_eq!(url, "https://media.example.com/A/chapter-01.mp3");
    }

    #[test]
    fn encodes_directory_and_filename() {
        let url = build_media_url("https://media.example.com", "Book A", "chapter 01.mp3")
            .expect("valid URL parts");
        assert_eq!(url, "https://media.example.com/Book%20A/chapter%2001.mp3");
    }

    #[test]
    fn rejects_empty_base_url() {
        let err = build_media_url("   ", "A", "chapter-01.mp3").expect_err("empty base URL");
        assert_eq!(err, UrlBuildError::EmptyBaseUrl);
    }

    #[test]
    fn rejects_empty_directory() {
        let err = build_media_url("https://media.example.com", " / ", "chapter-01.mp3")
            .expect_err("empty directory");
        assert_eq!(err, UrlBuildError::EmptyDirectory);
    }

    #[test]
    fn rejects_empty_filename() {
        let err =
            build_media_url("https://media.example.com", "A", " / ").expect_err("empty filename");
        assert_eq!(err, UrlBuildError::EmptyFileName);
    }

    #[test]
    fn sorts_lexical_for_release_order() {
        let files = vec![
            String::from("B.mp3"),
            String::from("A.mp3"),
            String::from("C.mp3"),
        ];
        let sorted = sort_files_lexical(&files);
        assert_eq!(
            sorted,
            vec![
                String::from("A.mp3"),
                String::from("B.mp3"),
                String::from("C.mp3"),
            ]
        );
    }

    #[test]
    fn loads_config_from_env_when_cli_not_set() {
        let cli = CliOverrides::default();
        let cfg = load_config_from_sources(&cli, |name| match name {
            MEDIA_BASE_URL_ENV => Some(String::from("https://media.example.com")),
            PODCAST_TITLE_ENV => Some(String::from("My Audio Book")),
            PODCAST_LINK_ENV => Some(String::from("https://example.com")),
            PODCAST_DESCRIPTION_ENV => Some(String::from("An audio feed")),
            RSS_SELF_URL_ENV => Some(String::from("https://feed.example.com/files/A")),
            _ => None,
        })
        .expect("config should load from env");

        assert_eq!(cfg.media_base_url, "https://media.example.com");
        assert_eq!(cfg.podcast_title, "My Audio Book");
    }

    #[test]
    fn cli_overrides_env_values() {
        let cli = CliOverrides {
            media_base_url: Some(String::from("https://override.example.com")),
            podcast_title: Some(String::from("Override Title")),
            podcast_link: Some(String::from("https://override-site.example.com")),
            podcast_description: Some(String::from("Override description")),
            rss_self_url: Some(String::from("https://override-feed.example.com/files/A")),
            print_config: false,
        };
        let cfg = load_config_from_sources(&cli, |_| Some(String::from("ignored")))
            .expect("config should load from cli");

        assert_eq!(cfg.media_base_url, "https://override.example.com");
        assert_eq!(cfg.podcast_title, "Override Title");
    }

    #[test]
    fn missing_or_empty_config_errors() {
        let cli = CliOverrides {
            media_base_url: Some(String::from(" ")),
            ..CliOverrides::default()
        };
        let err = load_config_from_sources(&cli, |_| None).expect_err("empty must fail");
        assert_eq!(err, ConfigError::MissingOrEmpty(MEDIA_BASE_URL_ENV));
    }
}
