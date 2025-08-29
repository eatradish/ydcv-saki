//! parser for the returned result from YD

use crate::{formatters::Formatter, lang::is_chinese};
use anyhow::{Result, anyhow};
use scraper::{Html, Selector, error::SelectorErrorKind};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;

/// Basic result structure
#[derive(Serialize, Deserialize, Debug)]
pub struct YdBasic {
    explains: Vec<String>,
    phonetic: Option<String>,
    us_phonetic: Option<String>,
    uk_phonetic: Option<String>,
}

/// Web result structure
#[derive(Serialize, Deserialize, Debug)]
pub struct YdWeb {
    key: String,
    value: Vec<String>,
}

/// Full response structure
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct YdResponse {
    query: String,
    #[serde(flatten)]
    inner: Option<YdResponseInner>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct YdResponseInner {
    translation: Vec<String>,
    basic: YdBasic,
    web: Vec<YdWeb>,
}

impl YdResponse {
    pub fn from_html(body: &str, word: &str) -> Result<Self> {
        let html = Html::parse_document(body);
        let is_chinese = is_chinese(word);

        let no_data = Selector::parse(".no-data-prompt").map_err(|e| anyhow!("{e}"))?;
        let mut is_no_data = false;
        html.select(&no_data).for_each(|x| {
            x.text().for_each(|_| {
                is_no_data = true;
            });
        });

        if is_no_data {
            return Ok(YdResponse {
                query: word.to_string(),
                inner: None,
            });
        }

        let res = if is_chinese {
            Self::zh2en(&html)
        } else {
            Self::en2zh(&html)
        }
        .map_err(|e| anyhow!("{e}"))?;

        Ok(YdResponse {
            query: word.to_string(),
            inner: Some(res),
        })
    }

    /// Explain the result in text format using a formatter
    pub fn explain(&self, fmt: &dyn Formatter) -> String {
        let mut result: Vec<String> = vec![];

        match &self.inner {
            Some(YdResponseInner {
                translation,
                basic,
                web,
            }) => {
                if web.is_empty() {
                    result.push(fmt.underline(&self.query));
                    result.push(fmt.cyan("  Translation:"));
                    result.push(format!("    {}", translation.join("；")));
                    return result.join("\n");
                }

                let phonetic = if let (Some(us_phonetic), Some(uk_phonetic)) =
                    (&basic.us_phonetic, &basic.uk_phonetic)
                {
                    format!(
                        " UK: [{}], US: [{}]",
                        fmt.yellow(uk_phonetic),
                        fmt.yellow(us_phonetic)
                    )
                    .into()
                } else if let Some(phonetic) = &basic.phonetic {
                    format!("[{}]", fmt.yellow(phonetic)).into()
                } else {
                    Cow::Borrowed("")
                };

                result.push(format!(
                    "{} {} {}",
                    fmt.underline(&self.query),
                    phonetic,
                    fmt.default(&translation.join("; "))
                ));

                if !basic.explains.is_empty() {
                    result.push(fmt.cyan("  Word Explanation:"));
                    for exp in &basic.explains {
                        result.push(fmt.default(&format!("     * {exp}")));
                    }
                }

                if !web.is_empty() {
                    result.push(fmt.cyan("  Web Reference:"));
                    for item in web {
                        result.push(format!("     * {}", &fmt.yellow(&item.key)));
                        result.push(format!(
                            "       {}",
                            &item
                                .value
                                .iter()
                                .map(|x| fmt.purple(x))
                                .collect::<Vec<_>>()
                                .join("；")
                        ));
                    }
                }
            }
            None => {
                result.push(fmt.red(" -- No result for this query."));
                return result.join("\n");
            }
        }

        result.join("\n")
    }

    /// Lookup words by Chinese meaning.
    fn zh2en(html: &Html) -> Result<YdResponseInner, SelectorErrorKind<'_>> {
        let trans = Selector::parse(".basic .col2 .word-exp .point")?;
        let mut translations = vec![];
        html.select(&trans).for_each(|x| {
            x.text().for_each(|x| {
                translations.push(x.to_string());
            });
        });

        let mut explains = vec![];
        let explains_query = Selector::parse(".basic .col2 .word-exp .point")?;
        html.select(&explains_query).for_each(|x| {
            x.text().for_each(|x| {
                explains.push(x.to_string());
            });
        });

        let mut phonetic = String::new();
        let per_phone = Selector::parse(".phone_con .per-phone .phonetic")?;
        html.select(&per_phone).for_each(|x| {
            x.text().for_each(|x| {
                phonetic.push_str(x.replace('/', "").trim());
            });
        });

        let mut keys = vec![];
        let mut values = vec![];
        let key = Selector::parse(".web_trans .col2 .point")?;
        let value = Selector::parse(".web_trans .col2 .sen-phrase")?;
        html.select(&key).for_each(|x| {
            x.text().for_each(|x| {
                keys.push(x);
            });
        });
        html.select(&value).for_each(|x| {
            let v = x
                .text()
                .collect::<String>()
                .split(" ; ")
                .map(|x| x.trim().to_string())
                .collect::<Vec<_>>();
            values.push(v);
        });

        let mut webs = vec![];

        for (i, c) in keys.iter().enumerate() {
            webs.push(YdWeb {
                key: c.to_string(),
                value: values[i].clone(),
            });
        }

        let resp = YdResponseInner {
            translation: translations
                .first()
                .map(|x| vec![x.to_string()])
                .unwrap_or_default(),
            basic: YdBasic {
                explains,
                phonetic: Some(phonetic),
                us_phonetic: None,
                uk_phonetic: None,
            },
            web: webs,
        };

        Ok(resp)
    }

    /// Lookup words by English word.
    fn en2zh(html: &Html) -> Result<YdResponseInner, SelectorErrorKind<'_>> {
        let mut per_phone = vec![];
        let phonetic = Selector::parse(".phone_con .per-phone")?;
        html.select(&phonetic).for_each(|x| {
            x.text().for_each(|x| {
                per_phone.push(x.replace('/', "").trim().to_string());
            });
        });

        let mut uk_phonetic = None;
        let mut us_phonetic = None;
        for (i, c) in per_phone.iter().enumerate() {
            if c == "英" {
                uk_phonetic = per_phone.get(i + 1).map(|x| x.to_string());
            } else if c == "美" {
                us_phonetic = per_phone.get(i + 1).map(|x| x.to_string());
            }
        }

        if us_phonetic.is_none() && uk_phonetic.is_none() {
            let phonetic = Selector::parse(".phone_con .per-phone .phonetic")?;
            html.select(&phonetic).for_each(|x| {
                x.text().for_each(|x| {
                    per_phone.push(x.replace('/', "").trim().to_string());
                });
            });
        }

        let mut poss = vec![];
        let pos = Selector::parse(".basic .word-exp .pos")?;
        html.select(&pos).for_each(|x| {
            x.text().for_each(|x| {
                poss.push(x.to_string());
            });
        });

        let mut translations = vec![];
        let trans = Selector::parse(".basic .word-exp .trans")?;
        html.select(&trans).for_each(|x| {
            x.text().for_each(|x| {
                translations.push(x.to_string());
            });
        });

        let translations_format = translations
            .iter()
            .enumerate()
            .map(|(i, c)| {
                if let Some(pos) = poss.get(i) {
                    format!("{pos} {c}")
                } else {
                    c.to_string()
                }
            })
            .collect::<Vec<_>>();

        let mut keys = vec![];
        let mut values = vec![];
        let key = Selector::parse(".web_trans .col2 .point")?;
        let value = Selector::parse(".web_trans .col2 .sen-phrase")?;
        html.select(&key).for_each(|x| {
            x.text().for_each(|x| {
                keys.push(x);
            });
        });
        html.select(&value).for_each(|x| {
            let v = x
                .text()
                .collect::<String>()
                .split(" ; ")
                .map(|x| x.trim().to_string())
                .collect::<Vec<_>>();
            values.push(v);
        });

        let mut webs = vec![];

        for (i, c) in keys.iter().enumerate() {
            webs.push(YdWeb {
                key: c.to_string(),
                value: values[i].clone(),
            });
        }

        let resp = YdResponseInner {
            translation: translations
                .first()
                .and_then(|x| x.split('，').next())
                .or_else(|| translations.first().map(|x| x.as_str()))
                .map(|x| vec![x.to_string()])
                .unwrap_or_default(),
            basic: YdBasic {
                explains: translations_format,
                phonetic: us_phonetic
                    .clone()
                    .or_else(|| uk_phonetic.clone())
                    .or_else(|| per_phone.first().map(|x| x.to_string())),
                us_phonetic,
                uk_phonetic,
            },
            web: webs,
        };

        Ok(resp)
    }
}

// For testing

#[cfg(test)]
use std::fmt;

#[cfg(test)]
impl fmt::Display for YdResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "YdResponse('{}')", self.query)
    }
}
