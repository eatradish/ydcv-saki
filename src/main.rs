//! main module of ydcv-rs

use std::fs::{self, create_dir_all};
use std::io::{IsTerminal, Write, stdout};

use anyhow::{Context, Result};
use clap::{ColorChoice, CommandFactory, Parser};
use clap_complete::CompleteEnv;
use dirs::cache_dir;
use log::warn;
use reqwest::blocking::{Client, ClientBuilder};
use rustyline::Editor;
use rustyline::config::Builder;
use rustyline::history::FileHistory;

mod formatters;
mod lang;
mod ydclient;
mod ydresponse;

#[cfg(windows)]
#[cfg(feature = "notify")]
use crate::formatters::WinFormatter;
use crate::formatters::{AnsiFormatter, Formatter, HtmlFormatter, PlainFormatter};
use crate::ydclient::YdClient;

fn lookup_explain(
    client: &mut Client,
    word: &str,
    fmt: &mut dyn Formatter,
    raw: bool,
) -> Result<()> {
    if raw {
        println!("{}", serde_json::to_string(&client.lookup_word(word)?)?);
    } else {
        match client.lookup_word(word) {
            Ok(ref result) => {
                let exp = result.explain(fmt);
                fmt.print(word, &exp);
            }
            Err(err) => fmt.print(word, &format!("Error looking-up word {word}: {err:?}")),
        }
    }

    Ok(())
}

#[derive(Parser)]
#[clap(version, about, max_term_width = 80)]
struct YdcvOptions {
    #[cfg(feature = "clipboard")]
    #[clap(short = 'x', long, help = "Show explanation of current selection")]
    selection: bool,

    #[cfg(windows)]
    #[cfg(feature = "clipboard")]
    #[clap(
        short,
        long,
        help = "Time interval between selection in msec (default: 1000 on windows and 0 on others)",
        default_value = "1000"
    )]
    interval: u64,

    #[cfg(unix)]
    #[cfg(feature = "clipboard")]
    #[clap(
        short,
        long,
        help = "Time interval between selection in msec (default: 1000 on windows and 0 on others)",
        default_value = "0"
    )]
    interval: u64,

    #[clap(short = 'H', long, help = "HTML-style output")]
    html: bool,

    #[cfg(feature = "notify")]
    #[clap(short, long, help = "Send desktop notifications (implies -H on X11)")]
    notify: bool,

    #[clap(
        short,
        long,
        help = "Dump raw json reply from server",
        conflicts_with = "html",
        conflicts_with = "notify"
    )]
    raw: bool,

    #[clap(short, long, default_value = "auto")]
    color: ColorChoice,

    #[cfg(unix)]
    #[cfg(feature = "notify")]
    #[clap(
        short,
        long,
        help = "Timeout of notification (second)",
        default_value = "30"
    )]
    timeout: i32,

    #[clap(value_name = "WORDS", help = "Words to lookup")]
    free: Vec<String>,
}

fn main() -> Result<()> {
    CompleteEnv::with_factory(YdcvOptions::command).complete();
    env_logger::init();

    let ydcv_options = YdcvOptions::parse();

    #[cfg(feature = "notify")]
    let notify_enabled = ydcv_options.notify;
    #[cfg(not(feature = "notify"))]
    let notify_enabled = false;

    #[cfg(feature = "clipboard")]
    let selection_enabled = ydcv_options.selection;

    #[cfg(feature = "clipboard")]
    let interval = ydcv_options.interval;

    #[cfg(not(feature = "clipboard"))]
    let selection_enabled = false;

    #[cfg(feature = "rustls")]
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("Failed to install rustls crypto provider");

    // reqwest will use HTTPS_PROXY env automatically
    let mut client = ClientBuilder::new().build()?;

    let mut html = HtmlFormatter::new(notify_enabled);
    let mut ansi = AnsiFormatter::new(notify_enabled);
    let mut plain = PlainFormatter::new(notify_enabled);
    #[cfg(windows)]
    #[cfg(feature = "notify")]
    let mut win = WinFormatter::new(notify_enabled);

    #[cfg(unix)]
    #[cfg(feature = "notify")]
    html.set_timeout(ydcv_options.timeout * 1000);

    let fmt: &mut dyn Formatter =
        if ydcv_options.html || (notify_enabled && cfg!(unix) && cfg!(feature = "notify")) {
            &mut html
        } else if notify_enabled {
            #[cfg(all(windows, feature = "notify"))]
            {
                &mut win
            }
            #[cfg(not(all(windows, feature = "notify")))]
            {
                &mut plain
            }
        } else if ydcv_options.color == ColorChoice::Always
            || stdout().is_terminal() && ydcv_options.color != ColorChoice::Never
        {
            &mut ansi
        } else {
            &mut plain
        };

    let history_path = cache_dir()
        .context("Failed to get cache dir path")?
        .join("ydcv")
        .join("history");

    let history_parent = history_path.parent().unwrap();

    if !history_parent.exists() {
        create_dir_all(history_parent)?;
    }

    let mut history_file = fs::OpenOptions::new().append(true).open(&history_path);

    if ydcv_options.free.is_empty() {
        if selection_enabled {
            #[cfg(feature = "clipboard")]
            {
                let mut clipboard = arboard::Clipboard::new()?;
                let mut last = String::new();

                println!("Waiting for selection> ");

                loop {
                    std::thread::sleep(std::time::Duration::from_millis(interval));
                    if let Ok(curr) = clipboard.get_text() {
                        let curr = curr.trim_matches('\u{0}').trim();
                        if !curr.is_empty() && last != curr {
                            last = curr.to_owned();
                            lookup_explain(&mut client, curr, fmt, ydcv_options.raw)?;

                            if let Ok(ref mut history_file) = history_file {
                                history_file.write_all(format!("{last}\n").as_bytes())?;
                            }

                            println!("Waiting for selection> ");
                        }
                    }
                }
            }
        } else {
            let mut reader = Editor::<(), FileHistory>::with_config(
                Builder::new().auto_add_history(true).build(),
            )?;

            if history_path.is_file() {
                reader
                    .load_history(&history_path)
                    .inspect_err(|e| warn!("Failed to load ydcv lookup history: {e}"))
                    .ok();
            }

            while let Ok(w) = reader.readline("> ") {
                let word = w.trim();
                if !word.is_empty() {
                    lookup_explain(&mut client, word, fmt, ydcv_options.raw)?;
                }
                reader
                    .save_history(&history_path)
                    .inspect_err(|e| warn!("Failed to load ydcv lookup history: {e}"))
                    .ok();
            }
        }
    } else {
        for word in &ydcv_options.free {
            lookup_explain(&mut client, word.trim(), fmt, ydcv_options.raw)?;
        }

        if let Ok(ref mut history_file) = history_file {
            history_file.write_all(format!("{}\n", ydcv_options.free.join(" ")).as_bytes())?;
        }
    }

    Ok(())
}
