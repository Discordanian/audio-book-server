use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, utf8_percent_encode};

const PATH_SEGMENT_ENCODE_SET: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'.')
    .remove(b'_')
    .remove(b'~');

#[derive(Debug, PartialEq, Eq)]
pub enum UrlBuildError {
    EmptyBaseUrl,
    EmptyDirectory,
    EmptyFileName,
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
}
