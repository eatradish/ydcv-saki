//! Formatters used by `YdResponse::explain`

#[cfg(all(feature = "notify", unix))]
use notify_rust::Notification;

#[cfg(all(feature = "notify", windows))]
use winrt_notification::{Duration, Toast};

extern crate htmlescape;
use htmlescape::encode_minimal;

macro_rules! def {
    ($($n:ident),*) => { $(
        fn $n (&self, s: &str) -> String;
    )* }
}

/// Base trait for formatters
pub trait Formatter {
    def!(red);
    def!(yellow);
    def!(purple);
    def!(cyan);
    def!(underline);
    def!(default);

    fn print(&mut self, word: &str, body: &str);
}

/// Plain text formatter
pub struct PlainFormatter;

macro_rules! plain {
    ($($n:ident),*) => { $(
        fn $n (&self, s: &str) -> String { s.to_owned() }
    )* }
}

impl PlainFormatter {
    pub fn new(_: bool) -> PlainFormatter {
        PlainFormatter {}
    }
}

impl Formatter for PlainFormatter {
    plain!(default, red, yellow, purple, cyan, underline);

    fn print(&mut self, _: &str, body: &str) {
        println!("{}", body);
    }
}

/// WinFormatter text formatter

#[cfg(all(feature = "notify", windows))]
pub struct WinFormatter {
    notify: bool,
}

#[cfg(all(feature = "notify", windows))]
impl WinFormatter {
    pub fn new(notify: bool) -> WinFormatter {
        WinFormatter { notify }
    }
}

#[cfg(all(feature = "notify", windows))]
macro_rules! ignore {
    ($($n:ident),*) => { $(
        fn $n (&self, _s: &str) -> String { "".to_owned() }
    )* }
}

#[cfg(all(feature = "notify", windows))]
impl Formatter for WinFormatter {
    plain!(default, red, yellow, purple, underline);
    ignore!(cyan);

    fn print(&mut self, _word: &str, body: &str) {
        if self.notify {
            // windows notification has limited lines
            // so we display as little as possible
            let lines: Vec<&str> = body.split('\n').filter(|x| x.len() > 0).collect();
            Toast::new(Toast::POWERSHELL_APP_ID)
                .title(lines[0])
                .text1(&lines[1..].join("\n"))
                .duration(Duration::Long)
                .show()
                .expect("ydcv: unable to toast");
        } else {
            println!("{}", body);
        }
    }
}

/// Ansi escaped colored formatter
pub struct AnsiFormatter;

macro_rules! ansi {
    ($( $n:ident = $x:expr ),*) => { $(
        fn $n (&self, s: &str) -> String {
            format!("\x1b[{}m{}\x1b[0m", $x, s)
        }
    )* }
}

impl AnsiFormatter {
    pub fn new(_: bool) -> AnsiFormatter {
        AnsiFormatter {}
    }
}

impl Formatter for AnsiFormatter {
    ansi!(red = 31, yellow = 33, purple = 35, cyan = 36, underline = 4);

    fn default(&self, s: &str) -> String {
        s.to_owned()
    }

    fn print(&mut self, _: &str, body: &str) {
        println!("{}", body);
    }
}

/// HTML-style formatter, suitable for desktop notification
#[cfg(all(feature = "notify", unix))]
pub struct HtmlFormatter {
    notify: bool,
    notifier: Notification,
    timeout: i32,
}

#[cfg(not(all(feature = "notify", unix)))]
pub struct HtmlFormatter;

impl HtmlFormatter {
    #[cfg(all(feature = "notify", unix))]
    pub fn new(notify: bool) -> HtmlFormatter {
        HtmlFormatter {
            notify,
            notifier: Notification::new(),
            timeout: 30000,
        }
    }

    #[cfg(not(all(feature = "notify", unix)))]
    pub fn new(_: bool) -> HtmlFormatter {
        HtmlFormatter {}
    }

    #[cfg(all(feature = "notify", unix))]
    pub fn set_timeout(&mut self, timeout: i32) {
        self.timeout = timeout;
    }
}

macro_rules! html {
    ($( $n:ident = $x:expr ),*) => { $(
        fn $n (&self, s: &str) -> String {
            format!(r#"<span color="{}">{}</span>"#, $x, encode_minimal(s))
        }
    )* }
}

impl Formatter for HtmlFormatter {
    html!(
        red = "red",
        yellow = "goldenrod",
        purple = "purple",
        cyan = "navy"
    );
    fn underline(&self, s: &str) -> String {
        format!(r#"<u>{}</u>"#, encode_minimal(s))
    }
    fn default(&self, s: &str) -> String {
        encode_minimal(s)
    }

    #[cfg(all(feature = "notify", unix))]
    fn print(&mut self, word: &str, body: &str) {
        if self.notify {
            self.notifier
                .appname("ydcv")
                .summary(word)
                .body(body)
                .timeout(self.timeout)
                .show()
                .unwrap();
        } else {
            println!("{}", body);
        }
    }

    #[cfg(not(all(feature = "notify", unix)))]
    fn print(&mut self, _: &str, body: &str) {
        println!("{}", body);
    }
}

#[cfg(test)]
mod tests {
    use crate::formatters::HtmlFormatter;
    use crate::ydclient::*;
    use reqwest::blocking::Client;

    #[test]
    fn test_explain_html_1() {
        let result = format!(
            "\n{}\n",
            Client::new()
                .lookup_word("hakunamatata")
                .unwrap()
                .explain(&HtmlFormatter::new(false))
        );
        assert_eq!(
            r#"
<span color="red"> -- No result for this query.</span>
"#,
            result
        );
    }

    #[test]
    fn test_explain_html_2() {
        let result = format!(
            "\n{}\n",
            Client::new()
                .lookup_word("comment")
                .unwrap()
                .explain(&HtmlFormatter::new(false))
        );
        assert_eq!(
            r#"
<u>comment</u>  UK: [<span color="goldenrod">ˈkɒment</span>], US: [<span color="goldenrod">ˈkɑːment</span>] 评论
<span color="navy">  Word Explanation:</span>
     * n. 评论，意见；批评，指责；说明，写照；&lt;旧&gt;解说，注释；（计算机）注解
     * v. 评论，发表意见；（计算机）注解，把（部分程序）转成注解
     * 【名】 （Comment）（美、瑞、法）科门特（人名）
<span color="navy">  Web Reference:</span>
     * <span color="goldenrod">No Comment</span>
       <span color="purple">不予置评</span>；<span color="purple">无可奉告</span>；<span color="purple">不予回答</span>；<span color="purple">无意见</span>
     * <span color="goldenrod">Fair comment</span>
       <span color="purple">公正评论</span>；<span color="purple">公允评论</span>；<span color="purple">合理评论</span>；<span color="purple">公正的评论</span>
     * <span color="goldenrod">conditional comment</span>
       <span color="purple">条件注释</span>
"#,
            result
        );
    }

    #[test]
    fn test_explain_html_3() {
        let result = format!(
            "\n{}\n",
            Client::new()
                .lookup_word("暂时")
                .unwrap()
                .explain(&HtmlFormatter::new(false))
        );
        assert_eq!(
            r#"
<u>暂时</u> [<span color="goldenrod">zàn shí</span>] for the time being
<span color="navy">  Word Explanation:</span>
     * for the time being
     * for the moment
<span color="navy">  Web Reference:</span>
     * <span color="goldenrod">暂时的</span>
       <span color="purple">科技  temporary</span>；<span color="purple">interim</span>；<span color="purple">provisional</span>；<span color="purple">科技  temporal</span>
     * <span color="goldenrod">今天暂时停止</span>
       <span color="purple">Groundhog Day</span>；<span color="purple">Groundhog Day Phil Connors</span>；<span color="purple">The Groundhug Day</span>
     * <span color="goldenrod">暂时性</span>
       <span color="purple">Temporary</span>；<span color="purple">caducity</span>；<span color="purple">transiency</span>；<span color="purple">transient</span>
"#,
            result
        );
    }
}
