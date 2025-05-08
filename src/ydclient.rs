//! ydclient is client wrapper for Client;

use std::sync::LazyLock;

use super::ydresponse::YdResponse;
use anyhow::Result;
use nyquest::{
    BlockingClient, ClientBuilder, Request,
    header::{REFERER, USER_AGENT},
};
use url::Url;

static INIT_NYQUEST: LazyLock<()> = LazyLock::new(|| {
    nyquest_preset::register();
});

pub struct Client {
    client: BlockingClient,
}

impl Client {
    pub fn new() -> Self {
        let _ = &*INIT_NYQUEST;

        Self {
            client: ClientBuilder::default()
                .with_header(USER_AGENT, "Mozilla/5.0 (X11; AOSC OS; Linux x86_64; rv:109.0) Gecko/20100101 Firefox/117.0")
                .build_blocking()
                .unwrap(),
        }
    }

    pub fn lookup_word(&self, word: &str) -> Result<YdResponse> {
        let mut url = Url::parse("https://www.youdao.com/result")?;
        url.query_pairs_mut()
            .append_pair("word", word)
            .append_pair("lang", "en")
            .finish();

        let body = self
            .client
            .request(Request::get(url.to_string()).with_header(REFERER, "https://www.youdao.com"))?
            .text()?;

        let res = YdResponse::from_html(&body, word)?;

        Ok(res)
    }
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
