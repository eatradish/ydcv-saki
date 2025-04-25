//! ydclient is client wrapper for Client

use super::ydresponse::YdResponse;
use anyhow::Result;
use reqwest::blocking::Client;
use reqwest::header::{REFERER, USER_AGENT};
use std::io::Read;

/// Wrapper trait on `reqwest::Client`
pub trait YdClient {
    /// lookup a word on YD and returns a `YdPreponse`
    ///
    /// # Examples
    ///
    /// lookup "hello" and compare the result:
    ///
    /// ```
    /// assert_eq!("YdResponse('hello')",
    ///        format!("{}", Client::new().lookup_word("hello").unwrap()));
    /// ```
    fn lookup_word(&mut self, word: &str) -> Result<YdResponse>;
}

/// Implement wrapper client trait on `reqwest::Client`
impl YdClient for Client {

    #[cfg(all(not(feature = "native-tls"), not(feature = "rustls")))]
    fn lookup_word(&mut self, word: &str, raw: bool) -> Result<YdResponse> {
        panic!("https access has been disabled in this build of ydcv-rs");
    }

    /// lookup a word on YD and returns a `YdResponse`
    #[cfg(any(feature = "native-tls", feature = "rustls"))]
    fn lookup_word(&mut self, word: &str) -> Result<YdResponse> {
        let body = lookup_word(word, self)?;
        let res = YdResponse::from_html(&body, word)?;

        Ok(res)
    }
}

fn lookup_word(word: &str, client: &Client) -> Result<String> {
    let mut body = String::new();
    client
        .get("https://www.youdao.com/result")
        .header(REFERER, "https://www.youdao.com")
        .header(
            USER_AGENT,
            "Mozilla/5.0 (X11; AOSC OS; Linux x86_64; rv:109.0) Gecko/20100101 Firefox/117.0",
        )
        .query(&[("word", word), ("lang", "en")])
        .send()?
        .read_to_string(&mut body)?;

    Ok(body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lookup_word_0() {
        assert_eq!(
            "YdResponse('hello')",
            format!("{}", Client::new().lookup_word("hello").unwrap())
        );
    }

    #[test]
    fn test_lookup_word_1() {
        assert_eq!(
            "YdResponse('world')",
            format!("{}", Client::new().lookup_word("world").unwrap())
        );
    }

    #[test]
    fn test_lookup_word_2() {
        assert_eq!(
            "YdResponse('<+*>?_')",
            format!("{}", Client::new().lookup_word("<+*>?_").unwrap())
        );
    }
}
