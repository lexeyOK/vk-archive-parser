use aho_corasick::AhoCorasick;
use anyhow::{Context, Result};
use chrono::NaiveDateTime;
use encoding_rs::WINDOWS_1251;
use encoding_rs_io::DecodeReaderBytesBuilder;
use indicatif::{ParallelProgressIterator, ProgressBar, ProgressStyle};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use std::{
    collections::HashSet,
    fs::File,
    io::{BufReader, Read},
    path::Path,
    sync::OnceLock,
};
use tl::ParserOptions;

const TIME_ZONE_CORRECTION: i64 = 5 * 3600; // one hour is 3600 seconds
const SELF_ID_URL: &str = "https://vk.com/id0";
/// Parsed vk chat.
#[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct VkChat {
    pub id: isize, // can be negative
    //title: String,
    pub users: HashSet<isize>,  // id-s
    pub messages: Vec<Message>, // can be very long
}

/// Single file parsed.
#[derive(Debug, Eq, PartialEq)]
pub struct VkPage {
    page_number: usize,
    message_items: Vec<Message>,
}

/// Contains parsed messages.
#[derive(Debug, Eq, PartialEq, Clone, Serialize, Deserialize)]
pub struct Message {
    id: usize,
    from_id: isize,
    date: i64,
    message_text: String,
    attachments: Option<Vec<Attachment>>,
}

/// Attachment to a `Message`.
#[derive(Debug, Eq, PartialEq, Clone, Serialize, Deserialize)]
pub struct Attachment {
    description: String,
    link: Option<String>,
}

/// Take `&[VkPage]` and collect all messages into VkChat
pub fn join_pages(pages: &[VkPage], id: isize) -> VkChat {
    let messages = pages
        .iter()
        .flat_map(|page| page.message_items.iter())
        .cloned()
        .collect::<Vec<_>>();

    let users = messages
        .par_iter()
        .map(|message| message.from_id)
        .collect::<HashSet<_>>();

    VkChat {
        id,
        users,
        messages,
    }
}

/// Parse chat folder.
pub fn parse_pages(folder: impl AsRef<Path>) -> anyhow::Result<Vec<VkPage>> {
    let file_paths = std::fs::read_dir(&folder)?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.is_file())
        .collect::<Vec<_>>();

    let progress_style = ProgressStyle::with_template("[{pos}/{len}] [{wide_bar}] {per_sec}")
        .expect("invalid style")
        .progress_chars("=> ");

    // let title = parse_title(&file_paths[0]);
    let mut pages = file_paths
        .par_iter()
        .progress_with(ProgressBar::new(file_paths.len() as u64).with_style(progress_style))
        .map(|file_path| {
            let text = decode_text(file_path)?;
            parse_text(&text)
        })
        .collect::<Result<Vec<VkPage>>>()?;
    pages.sort_by(|a, b| a.page_number.cmp(&b.page_number));
    Ok(pages)
}

/// Decode text from file in cp1251 encoding.
fn decode_text(file_path: impl AsRef<Path>) -> anyhow::Result<String> {
    let file = File::open(file_path).unwrap();
    let file = DecodeReaderBytesBuilder::new()
        .encoding(Some(WINDOWS_1251))
        .build(file);
    let mut buffer = BufReader::new(file);
    let mut text = String::new();
    buffer.read_to_string(&mut text)?;
    Ok(text)
}

/// Parse html page.
fn parse_text(input: &str) -> anyhow::Result<VkPage> {
    let dom = tl::parse(input, ParserOptions::default()).unwrap();
    let parser = dom.parser();
    let messages = dom.get_elements_by_class_name("message");

    let message_items = messages
        .map(|message| message.get(parser).unwrap().as_tag().unwrap())
        .map(|message| parse_message(message, parser))
        .collect::<anyhow::Result<Vec<_>>>()?;

    let page_number: usize = match dom.get_elements_by_class_name("pg_lnk_sel").next() {
        Some(link) => link
            .get(parser)
            .unwrap()
            .inner_text(parser)
            .parse()
            .unwrap(),
        None => 1,
    };

    Ok(VkPage {
        page_number,
        message_items,
    })
}

/// Parse message.
fn parse_message(message: &tl::HTMLTag<'_>, parser: &tl::Parser) -> anyhow::Result<Message> {
    let id: usize = message // item_nodes[3]
        .attributes()
        .get("data-id")
        .unwrap()
        .unwrap()
        .as_utf8_str()
        .parse()?;
    let header = message
        .query_selector(parser, ".message__header")
        .unwrap()
        .next()
        .unwrap()
        .get(parser)
        .unwrap()
        .as_tag()
        .unwrap();

    let from_id = {
        let link_href = match header.query_selector(parser, "a").unwrap().next() {
            Some(link) => link
                .get(parser)
                .unwrap()
                .as_tag()
                .unwrap()
                .attributes()
                .get("href")
                .unwrap()
                .unwrap()
                .as_utf8_str(),
            None => std::borrow::Cow::Borrowed(SELF_ID_URL),
        };
        let slug_str = link_href.split_at(15).1;
        parse_from_id(slug_str)?
    };

    let message_inner_text = message.inner_text(parser);
    let split_message = message_inner_text.split("  ").collect::<Vec<_>>();
    let date = {
        let header_str = split_message[1];
        let date_str = header_str.rsplit_once(", ").unwrap().1;
        parse_date_time(date_str)? + TIME_ZONE_CORRECTION
    };

    let message_text = split_message[2].trim().to_string();

    let attachments = message.query_selector(parser, ".attachment").map(|iter| {
        iter.map(|handle| parse_attachment(handle.get(parser).unwrap().as_tag().unwrap(), parser))
            .collect()
    });

    Ok(Message {
        id,
        from_id,
        date,
        message_text,
        attachments,
    })
}

fn parse_attachment(item: &tl::HTMLTag<'_>, parser: &tl::Parser) -> Attachment {
    let description = item
        .query_selector(parser, ".attachment__description")
        .unwrap()
        .next()
        .unwrap()
        .get(parser)
        .unwrap()
        .inner_text(parser)
        .to_string();
    let link = item
        .query_selector(parser, ".attachment__link")
        .and_then(|mut iter| iter.next())
        .map(|node| node.get(parser).unwrap().inner_text(parser).to_string());
    Attachment { description, link }
}

fn parse_from_id(slug_str: &str) -> anyhow::Result<isize> {
    let (tp, from_id_str) = slug_str.split_at(
        slug_str
            .find(|c| char::is_ascii_digit(&c))
            .with_context(|| format!("wrong slug: {}", slug_str))?,
    );
    let sign = if matches!(tp, "club" | "public") {
        -1
    } else {
        1
    };
    Ok(sign * from_id_str.parse::<isize>()?)
}

#[test]
fn simple_from_id() {
    assert_eq!(parse_from_id(SELF_ID_URL.split_at(15).1).unwrap(), 0);
    assert_eq!(parse_from_id("club1").unwrap(), -1);
}

static AC: OnceLock<AhoCorasick> = OnceLock::new();

fn parse_date_time(date_str: &str) -> anyhow::Result<i64> {
    let ac = AC.get_or_init(|| {
        AhoCorasick::new([
            "янв", "фев", "мар", "апр", "мая", "июн", "июл", "авг", "сен", "окт", "ноя", "дек",
        ])
        .expect("walid haystack")
    });
    let replace_with = &[
        "01", "02", "03", "04", "05", "06", "07", "08", "09", "10", "11", "12",
    ];
    let result = ac.replace_all(date_str, replace_with);
    let timestamp = NaiveDateTime::parse_and_remainder(&result, "%d %m %Y в %H:%M:%S")
        .with_context(|| format!("this is not a valid date time: {}", &result))?
        .0
        .timestamp();
    Ok(timestamp)
}

#[test]
fn simple_date_time() {
    use chrono::NaiveDate;
    let string = "20 июн 2023 в 8:34:00 (ред.)";
    let dt = NaiveDate::from_ymd_opt(2023, 6, 20)
        .unwrap()
        .and_hms_opt(8, 34, 0)
        .unwrap()
        .timestamp();
    assert_eq!(parse_date_time(string).unwrap(), dt);
}
