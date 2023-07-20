use aho_corasick::AhoCorasick;
use chrono::NaiveDateTime;
use encoding_rs::WINDOWS_1251;
use encoding_rs_io::DecodeReaderBytesBuilder;
use indicatif::{ParallelProgressIterator, ProgressBar, ProgressStyle};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json;
use tl::ParserOptions;

use std::{
    collections::HashSet,
    fs::File,
    io::{BufRead, BufReader, BufWriter, Read, Write},
    path::Path,
    sync::OnceLock,
    time::Instant,
};

const TIME_ZONE_CORRECTION: i64 = 5 * 3600; // one hour is 3600 seconds
const SELF_ID_URL: &str = "https://vk.com/id0";
fn main() {
    let started = Instant::now();
    // take filename from argument and open file for reading
    let folder = std::env::args()
        .nth(1)
        .expect("vk-archive-parser [folder-name]");
    //let metadata = std::fs::metadata(folder.clone()).unwrap();

    let path = Path::new(&folder);
    print!("{:?} ", &folder);
    let chat = parse_vk_chat(path);

    let data_file = File::create(format!("{}.json", chat.id)).unwrap();
    let mut writer = BufWriter::new(data_file);
    let serialised = serde_json::to_string_pretty(&chat).expect("you have fuckted up");
    writer.write_all(serialised.as_ref()).unwrap();
    println!("Done in {:?}!", started.elapsed());
}

/// Pased vk chat.
#[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
struct VkChat {
    id: isize, // can be negative
    //title: String,
    users: HashSet<isize>,  // id-s
    messages: Vec<Message>, // can be very long
}

/// Single file parsed.
#[derive(Debug, Eq, PartialEq)]
struct VkPage {
    page_number: usize,
    message_items: Vec<Message>,
}

/// Contains parsed messages.
#[derive(Debug, Eq, PartialEq, Clone, Serialize, Deserialize)]
struct Message {
    id: usize,
    from_id: isize,
    date: i64,
    message_text: String,
    attachments: Option<Vec<Attachment>>,
}

/// Attachment to a `Message`.
#[derive(Debug, Eq, PartialEq, Clone, Serialize, Deserialize)]
struct Attachment {
    description: String,
    link: Option<String>,
}

/// Parse chat folder.
fn parse_vk_chat(folder: impl AsRef<Path>) -> VkChat {
    let id: isize = folder
        .as_ref()
        .file_name()
        .unwrap()
        .to_str()
        .unwrap()
        .parse::<_>()
        .unwrap();

    let file_paths: Vec<_> = std::fs::read_dir(&folder)
        .expect("access denied")
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.is_file())
        .collect();

    // let title = parse_title(&file_paths[0]);
    let mut pages: Vec<VkPage> = file_paths
        .par_iter()
        .progress_with(
            ProgressBar::new(file_paths.len() as u64).with_style(
                ProgressStyle::with_template("[{pos}/{len}] [{wide_bar}] {per_sec}")
                    .expect("incorect style")
                    .progress_chars("=> "),
            ),
        )
        .map(|file_path| parse_file(file_path))
        .collect();
    pages.sort_by(|a, b| a.page_number.cmp(&b.page_number));
    join_pages(&pages, id)
}

fn parse_file(file_path: impl AsRef<Path>) -> VkPage {
    let file = std::fs::File::open(file_path).unwrap();
    let file = DecodeReaderBytesBuilder::new()
        .encoding(Some(WINDOWS_1251))
        .build(file);
    let mut buffer = BufReader::new(file);
    let mut text = String::new();
    buffer.read_to_string(&mut text).unwrap();
    parse_text(&text)
}

// Take `&[VkPage]` and make all messages into VkChat
fn join_pages(pages: &[VkPage], id: isize) -> VkChat {
    let messages: Vec<_> = pages
        .iter()
        .flat_map(|page| page.message_items.iter())
        .cloned()
        .collect();

    let users: HashSet<_> = messages.par_iter().map(|message| message.from_id).collect();

    VkChat {
        id,
        users,
        messages,
    }
}

/// Parse html page.
fn parse_text(input: &str) -> VkPage {
    let dom = tl::parse(input, ParserOptions::default()).unwrap();
    let parser = dom.parser();
    let messages = dom.get_elements_by_class_name("message");

    let message_items: Vec<Message> = messages
        .map(|message| message.get(parser).unwrap().as_tag().unwrap())
        .map(|message| parse_message(message, parser))
        .collect();

    let page_number: usize = match dom.get_elements_by_class_name("pg_lnk_sel").next() {
        Some(link) => link
            .get(parser)
            .unwrap()
            .inner_text(parser)
            .parse()
            .unwrap(),
        None => 1,
    };

    VkPage {
        page_number,
        message_items,
    }
}

/// Parse message.
fn parse_message(message: &tl::HTMLTag<'_>, parser: &tl::Parser) -> Message {
    let id: usize = message // item_nodes[3]
        .attributes()
        .get("data-id")
        .unwrap()
        .unwrap()
        .as_utf8_str()
        .parse()
        .unwrap();

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
        parse_from_id(slug_str)
    };

    let date = {
        let header_str = header.inner_text(parser);
        let date_str = header_str.rsplit_once(", ").unwrap().1;
        parse_date_time(date_str) + TIME_ZONE_CORRECTION
    };

    let message_text = message.inner_text(parser).trim().to_string();

    let attachments = message.query_selector(parser, ".attachment").map(|iter| {
        iter.map(|handle| parse_attachment(handle.get(parser).unwrap().as_tag().unwrap(), parser))
            .collect()
    });

    Message {
        id,
        from_id,
        date,
        message_text,
        attachments,
    }
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

fn parse_from_id(slug_str: &str) -> isize {
    let (tp, from_id_str) = slug_str.split_at(slug_str.find(|c| char::is_ascii_digit(&c)).unwrap());
    let sign = if matches!(tp, "club" | "public") {
        -1
    } else {
        1
    };
    sign * from_id_str.parse::<isize>().unwrap()
}

#[test]
fn simple_from_id() {
    assert_eq!(parse_from_id(SELF_ID_URL.split_at(15).1), 123123123);
    assert_eq!(parse_from_id("club1"), -1);
}

static AC: OnceLock<AhoCorasick> = OnceLock::new();

fn parse_date_time(date_str: &str) -> i64 {
    let ac = AC.get_or_init(|| {
        AhoCorasick::new([
            "янв", "фев", "мар", "апр", "мая", "июн", "июл", "авг", "сен", "окт", "ноя", "дек",
        ])
        .unwrap()
    });
    let replace_with = &[
        "01", "02", "03", "04", "05", "06", "07", "08", "09", "10", "11", "12",
    ];
    let result = ac.replace_all(date_str, replace_with);
    NaiveDateTime::parse_and_remainder(&result, "%d %m %Y в %H:%M:%S")
        .expect(&format!("this is not a walid date time: {}", &result))
        .0
        .timestamp()
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
    assert_eq!(parse_date_time(string), dt);
}
