#![allow(unused_imports, unused_variables, dead_code)]
use aho_corasick::AhoCorasick;
use chrono::{NaiveDate, NaiveDateTime};
use rayon::prelude::*;
use std::fs;
use std::io::BufRead;
use std::sync::OnceLock;
use tl::{parse, HTMLTag, Parser};
const TIME_ZONE_CORRECTION: i64 = 5 * 3600; // one hour is 3600 seconds
const SELF_ID_URL: &str = "https://vk.com/id321553803";
static AC: OnceLock<AhoCorasick> = OnceLock::new();

fn main() {
    // take filename from argument and open file for reading
    let folder = std::env::args()
        .nth(1)
        .expect("vk-archive-parser [folder-name]");
    assert!(
        std::fs::metadata(folder.clone())
            .expect("is valid folder")
            .is_dir(),
        "{:?} is not valid folder",
        folder
    );

    let file_paths = fs::read_dir(folder)
        .expect("access denied")
        .filter_map(|entry| {
            let path = entry.unwrap().path();
            if path.is_file() {
                Some(path)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    file_paths.par_iter().for_each(|file_path| {
        let file = fs::File::open(file_path).unwrap();
        let page = parse_file(file);
        println!("{:?} {}", file_path, page.message_items.len());
    });

    //let pages = file_paths
    //    .map(|file_path| parse_file(std::fs::File::open(file_path.unwrap()).message_items.len());
    //file_paths.zip(pages).for_each(|(file_path,message_count)| println!("{:?} {}", file_path,message_count));
    //let page = parse_file(file);
    // page.message_items.iter().for_each(|e| println!("{:?}",e));
    //println!("{:?}", page.message_items.len());
}

// Single file parsed
struct VkPage {
    page_number: usize,
    message_items: Vec<Message>,
}

/// Contains parsed messages
#[derive(Debug, Eq, PartialEq)]
struct Message {
    id: i32,
    from_id: i32,
    date: i64,
    message_text: String,
}

/// parse single archive file
fn parse_file(file: impl std::io::Read) -> VkPage {
    // read file into a string
    let contents = std::io::BufReader::new(file);
    let text = contents.lines().map(|l| l.unwrap()).collect::<String>();
    parse_text(&text)
}

/// parse html page.
fn parse_text(input: &str) -> VkPage {
    let dom = tl::parse(input, Default::default()).unwrap();
    let parser = dom.parser();
    let messages = dom.get_elements_by_class_name("message");
    let message_items: Vec<Message> = messages
        .map(|message| message.get(parser).unwrap().as_tag().unwrap())
        .map(|message| parse_message(message,parser))
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

fn parse_message(item: &tl::HTMLTag<'_>, parser: &tl::Parser) -> Message {
    let id: i32 = item
        .attributes()
        .get("data-id")
        .unwrap()
        .unwrap()
        .as_utf8_str()
        .parse()
        .unwrap();
    let header = item
        .query_selector(parser, ".message__header")
        .unwrap()
        .next()
        .unwrap()
        .get(parser)
        .unwrap()
        .as_tag()
        .unwrap();
    let (from_id, date) = parse_header(header, parser);
    let item_nodes = item.children().all(parser);
    let message_text = item_nodes[item_nodes.len() - 2]
        .inner_text(parser)
        .to_string();
    Message {
        id,
        from_id,
        date,
        message_text,
    }
}

fn parse_date_time(input: &str) -> NaiveDateTime {
    let ac = AC.get_or_init(|| {
        AhoCorasick::new([
            "янв", "фев", "мар", "апр", "мая", "июн", "июл", "авг", "сен", "окт", "ноя", "дек",
        ])
        .unwrap()
    });
    let replace_with = &[
        "01", "02", "03", "04", "05", "06", "07", "08", "09", "10", "11", "12",
    ];
    let result = ac.replace_all(input, replace_with);
    NaiveDateTime::parse_and_remainder(&result, "%d %m %Y в %H:%M:%S")
        .unwrap()
        .0
}

#[test]
fn simple() {
    let string = "20 июн 2023 в 8:34:00 (ред.)";
    let dt = NaiveDate::from_ymd_opt(2023, 6, 20)
        .unwrap()
        .and_hms_opt(8, 34, 0)
        .unwrap();
    assert_eq!(parse_date_time(string), dt);
}

fn parse_header(header: &HTMLTag, parser: &Parser) -> (i32, i64) {
    let from_link_href = match header.query_selector(parser, "a").unwrap().next() {
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

    let from_id_str = from_link_href.split_at(15).1;
    let from_id: i32 = from_id_str
        .matches(char::is_numeric)
        .collect::<String>()
        .parse()
        .unwrap();

    let header_str = header.inner_text(parser);
    let time_str = header_str.rsplit_once(", ").unwrap().1;
    let date: i64 = parse_date_time(&time_str).timestamp() + TIME_ZONE_CORRECTION;
    (from_id, date)
}

#[test]
fn header_test() {
    let doc = tl::parse(
        r#"
        <div class="message__header">
            <a href="https://vk.com/id334240417">
                Илья Храмцов
            </a>
            , 14 июл 2021 в 11:17:48</div>"#,
        Default::default(),
    )
    .unwrap();
    let parser = doc.parser();
    let header = doc
        .query_selector(".message__header")
        .unwrap()
        .next()
        .unwrap()
        .get(parser)
        .unwrap()
        .as_tag()
        .unwrap();
    let (from_id, date) = parse_header(header.into(), parser);
    assert_eq!(from_id, 334240417);
    assert_eq!(
        date,
        parse_date_time("14 июл 2021 в 11:17:48").timestamp() + TIME_ZONE_CORRECTION
    );
}
