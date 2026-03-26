use std::fs;
use std::path::PathBuf;

use chrono::{DateTime, Duration, Utc};
use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, utf8_percent_encode};
use wasi::exports::http::incoming_handler::Guest;
use wasi::http::types::{
    Fields, IncomingRequest, OutgoingBody, OutgoingResponse, ResponseOutparam,
};

const PATH_SEGMENT_ENCODE_SET: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'.')
    .remove(b'_')
    .remove(b'~');

pub const MEDIA_BASE_URL_ENV: &str = "MEDIA_BASE_URL";
pub const PODCAST_TITLE_ENV: &str = "PODCAST_TITLE";
pub const PODCAST_LINK_ENV: &str = "PODCAST_LINK";
pub const PODCAST_DESCRIPTION_ENV: &str = "PODCAST_DESCRIPTION";
pub const FILES_ROOT: &str = "/files";

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
    pub print_config: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppConfig {
    pub media_base_url: String,
    pub podcast_title: String,
    pub podcast_link: String,
    pub podcast_description: String,
}

impl AppConfig {
    pub fn to_pretty_string(&self) -> String {
        format!(
            "Effective configuration:\n\
             - {MEDIA_BASE_URL_ENV}={}\n\
             - {PODCAST_TITLE_ENV}={}\n\
             - {PODCAST_LINK_ENV}={}\n\
             - {PODCAST_DESCRIPTION_ENV}={}",
            self.media_base_url,
            self.podcast_title,
            self.podcast_link,
            self.podcast_description,
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
    })
}

pub fn load_config_from_env(cli: &CliOverrides) -> Result<AppConfig, ConfigError> {
    load_config_from_sources(cli, |name| std::env::var(name).ok())
}

fn xml_escape(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn is_audio_file(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.ends_with(".mp3")
        || lower.ends_with(".m4a")
        || lower.ends_with(".aac")
        || lower.ends_with(".ogg")
        || lower.ends_with(".flac")
}

fn list_subdirectories() -> std::io::Result<Vec<String>> {
    let mut dirs: Vec<String> = fs::read_dir(FILES_ROOT)?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let ty = entry.file_type().ok()?;
            if !ty.is_dir() {
                return None;
            }
            Some(entry.file_name().to_string_lossy().to_string())
        })
        .collect();
    dirs.sort();
    Ok(dirs)
}

fn list_audio_files_for_dir(dir: &str) -> std::io::Result<Vec<String>> {
    let path = PathBuf::from(FILES_ROOT).join(dir);
    let mut files: Vec<String> = fs::read_dir(path)?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let ty = entry.file_type().ok()?;
            if !ty.is_file() {
                return None;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            if is_audio_file(&name) {
                Some(name)
            } else {
                None
            }
        })
        .collect();
    files.sort();
    Ok(files)
}

fn self_url_from_request(request: &IncomingRequest) -> String {
    let path = request
        .path_with_query()
        .unwrap_or_else(|| "/".to_string());

    let host = request
        .headers()
        .get(&"host".to_string())
        .into_iter()
        .next()
        .and_then(|v| String::from_utf8(v).ok())
        .unwrap_or_default();

    if host.is_empty() {
        path
    } else {
        format!("https://{host}{path}")
    }
}

fn build_feed_xml(config: &AppConfig, dir: &str, files: &[String], self_url: &str) -> String {
    let start_date = start_release_datetime_for_directory(dir);
    let mut out = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    out.push_str("<rss version=\"2.0\" xmlns:atom=\"http://www.w3.org/2005/Atom\">\n<channel>\n");
    out.push_str(&format!(
        "<atom:link href=\"{}\" rel=\"self\" type=\"application/rss+xml\"/>\n\
         <title>{}</title>\n<link>{}</link>\n<description>{}</description>\n",
        xml_escape(self_url),
        xml_escape(&format!("{} - {}", config.podcast_title, dir)),
        xml_escape(&config.podcast_link),
        xml_escape(&config.podcast_description)
    ));

    for (index, file) in files.iter().enumerate() {
        if let Ok(url) = build_media_url(&config.media_base_url, dir, file) {
            let pub_date = release_datetime_for_index(start_date, index);
            out.push_str("<item>\n");
            out.push_str(&format!("<title>{}</title>\n", xml_escape(file)));
            out.push_str(&format!("<guid>{}</guid>\n", xml_escape(&url)));
            out.push_str(&format!("<pubDate>{}</pubDate>\n", pub_date));
            out.push_str(&format!(
                "<enclosure url=\"{}\" type=\"audio/mpeg\" />\n",
                xml_escape(&url)
            ));
            out.push_str("</item>\n");
        }
    }

    out.push_str("</channel>\n</rss>\n");
    out
}

fn release_datetime_for_index(start: DateTime<Utc>, index: usize) -> String {
    (start + Duration::minutes(index as i64)).to_rfc2822()
}

fn start_release_datetime_for_directory(dir: &str) -> DateTime<Utc> {
    // Keep all generated episode dates safely in the past, while making the
    // start date deterministic per directory.
    let days_back = 365_i64 + (stable_directory_hash(dir) % 3650) as i64;
    Utc::now() - Duration::days(days_back)
}

fn stable_directory_hash(value: &str) -> u64 {
    // FNV-1a 64-bit for stable deterministic hashing.
    let mut hash = 0xcbf29ce484222325_u64;
    for b in value.as_bytes() {
        hash ^= u64::from(*b);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn html_index_page(dirs: &[String]) -> String {
    let mut out = String::from(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>Audio Book Server</title></head><body><h1>Available feeds</h1><ul>",
    );
    for dir in dirs {
        out.push_str(&format!(
            "<li><a href=\"/{0}\">/{0}</a></li>",
            xml_escape(dir)
        ));
    }
    out.push_str("</ul></body></html>");
    out
}

fn send_response(status: u16, content_type: &str, body: &[u8], response_out: ResponseOutparam) {
    let headers = Fields::new();
    headers
        .append(&"content-type".to_string(), content_type.as_bytes())
        .ok();
    let response = OutgoingResponse::new(headers);
    response.set_status_code(status).ok();
    let response_body = response.body().expect("response body handle");
    ResponseOutparam::set(response_out, Ok(response));
    let stream = response_body.write().expect("response stream");
    stream.blocking_write_and_flush(body).ok();
    drop(stream);
    OutgoingBody::finish(response_body, None).ok();
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

struct Component;

impl Guest for Component {
    fn handle(request: IncomingRequest, response_out: ResponseOutparam) {
        let cli = CliOverrides::default();
        let config = match load_config_from_env(&cli) {
            Ok(cfg) => cfg,
            Err(err) => {
                send_response(
                    500,
                    "text/plain; charset=utf-8",
                    format!("Configuration error: {err}").as_bytes(),
                    response_out,
                );
                return;
            }
        };

        let self_url = self_url_from_request(&request);
        let path = self_url
            .split('?')
            .next()
            .map(|p| {
                // strip the host portion if present, keeping only the path
                if let Some(after_scheme) = p.strip_prefix("https://").or_else(|| p.strip_prefix("http://")) {
                    after_scheme
                        .find('/')
                        .map(|i| &after_scheme[i..])
                        .unwrap_or("/")
                        .to_string()
                } else {
                    p.to_string()
                }
            })
            .unwrap_or_else(|| "/".to_string());

        if path == "/" {
            match list_subdirectories() {
                Ok(dirs) => send_response(
                    200,
                    "text/html; charset=utf-8",
                    html_index_page(&dirs).as_bytes(),
                    response_out,
                ),
                Err(err) => send_response(
                    500,
                    "text/plain; charset=utf-8",
                    format!("Failed to read files root: {err}").as_bytes(),
                    response_out,
                ),
            }
        } else if let Some(dir) = path.strip_prefix('/').map(|s| s.trim_end_matches('/')).filter(|s| !s.is_empty()) {
            if dir.contains('/') {
                send_response(
                    400,
                    "text/plain; charset=utf-8",
                    b"Invalid directory path",
                    response_out,
                )
            } else {
                match list_audio_files_for_dir(dir) {
                    Ok(files) => {
                        let sorted = sort_files_lexical(&files);
                        let rss = build_feed_xml(&config, dir, &sorted, &self_url);
                        send_response(
                            200,
                            "application/rss+xml; charset=utf-8",
                            rss.as_bytes(),
                            response_out,
                        )
                    }
                    Err(err) => send_response(
                        404,
                        "text/plain; charset=utf-8",
                        format!("Directory not found: {err}").as_bytes(),
                        response_out,
                    ),
                }
            }
        } else {
            send_response(404, "text/plain; charset=utf-8", b"Not found", response_out)
        }
    }
}

wasi::http::proxy::export!(Component);

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

    #[test]
    fn release_datetimes_increase_by_one_minute() {
        let start = DateTime::parse_from_rfc2822("Wed, 01 Jan 2020 00:00:00 +0000")
            .expect("valid date")
            .with_timezone(&Utc);
        let first = release_datetime_for_index(start, 0);
        let second = release_datetime_for_index(start, 1);
        let first_dt = DateTime::parse_from_rfc2822(&first)
            .expect("valid first pubDate")
            .with_timezone(&Utc);
        let second_dt = DateTime::parse_from_rfc2822(&second)
            .expect("valid second pubDate")
            .with_timezone(&Utc);
        assert_eq!(first_dt, start);
        assert_eq!(second_dt - first_dt, Duration::minutes(1));
    }

    #[test]
    fn directory_start_date_is_deterministic_and_in_past() {
        let now = Utc::now();
        let d1 = start_release_datetime_for_directory("A");
        let d2 = start_release_datetime_for_directory("A");
        assert!(d1 <= now);
        assert!(d2 <= now);

        let delta = d2 - d1;
        assert!(delta.num_seconds().abs() < 2);
    }
}
