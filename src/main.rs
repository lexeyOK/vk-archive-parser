use anyhow;
use std::{
    fs::File,
    io::{BufWriter, Write},
    path::Path,
    time::Instant,
};

mod vk_chat;
use vk_chat::{join_pages, parse_pages};

fn main() -> anyhow::Result<()> {
    let started = Instant::now();

    let folder = std::env::args()
        .nth(1)
        .expect("vk-archive-parser [folder-name]");

    let path = Path::new(&folder);
    print!("{:?} ", &folder);

    let id: isize = path.file_name().unwrap().to_str().unwrap().parse::<_>()?;

    let pages = parse_pages(path)?;
    let chat = join_pages(&pages, id);

    let data_file = File::create(format!("{}.json", chat.id))?;
    let mut writer = BufWriter::new(data_file);

    let serialised = serde_json::to_string(&chat)?;
    writer.write_all(serialised.as_ref())?;

    println!("Done in {} s!", started.elapsed().as_secs_f32());
    Ok(())
}
