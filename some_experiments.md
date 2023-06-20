can you use rust library nom to parse this message format into struct Message
here is format:
```html
<div class="message" data-id="MES_ID">
  <div class="message__header"><a href="https://vk.com/idFROM_ID">FROM_NAME</a>, DATE_TIME_STRING</div>
  <div>MESSAGE_TEXT<div class="kludges"><div class="attachment">
  <div class="attachment__description">Опрос</div>
  	
</div></div></div>
</div>
```
here is an example of message:
```
<div class="message" data-id="3597376">
  <div class="message__header"><a href="https://vk.com/id334240417">Илья Храмцов</a>, 19 июн 2023 в 17:18:36</div>
  <div><div class="kludges"><div class="attachment">
  <div class="attachment__description">Опрос</div>
  
</div></div></div>
</div>
```

```rust
struct Message<'a> {
    id: i32, // MES_ID
    from_id: i32, // FROM_ID
    date: i64, // convert from DATE_TIME_STRING to unix timestamp 
    message_text: &'a str, // MESSAGE_TEXT
}
```

/\d{1,2} {янв|фев|мар|апр|мая|июн|июл|авг|сен|окт|ноя|дек|} \d{4} в \d{1,2}:\d{2}:\d{2}/


```rust
/// parse .message div
/*<div class="message" data-id="3595193">
  <div class="message__header"><a href="https://vk.com/id550836470">Валера Горошик</a>, 18 июн 2023 в 15:51:33</div>
  <div>Пауля нашли<div class="kludges"></div></div>
</div></div>*/
fn parse_message(input: &str) -> IResult<&str, Message> {
    fn parse_mes_id(input: &str) -> IResult<&str, i32> {
        map_res(delimited(tag(""), digit1, tag("\">\n")), FromStr::from_str)(input)
    }
    fn parse_from_id(input: &str) -> IResult<&str, i32> {
        map_res(
            delimited(tag("https://vk.com/id"), digit1, tag("\">")),
            FromStr::from_str,
        )(input)
    }
    fn parse_message_text(input: &str) -> IResult<&str, &str> {
        let (input, text) = take_until("<div class=\"kludges\"></div>")(input)?;
        Ok((input, text.trim()))
    }
    
    let (input, _) = tag(
        "</div><div class=\"item\">\n  <div class='item__main'><div class=\"message\" data-id=",
    )(input)?;
    let (input, id) = parse_mes_id(input)?;
    let (input, _) = multispace0(input)?;
    let (input, _) = tag("<div class=\"message__header\"><a href=\"")(input)?;
    let (input, from_id) = parse_from_id(input)?;
    let (input, _) = take_until("</a>, ")(input)?;
    let (input, _) = tag("</a>, ")(input)?;
    let (input, date) = map(take_until("</div>"), parse_date_time)(input)?;
    let (input, _) = tag("</div>\n<div>")(input)?;
    let (input, message_text) = parse_message_text(input)?;
    let (input, _) = tag("<div class=\"kludges\"></div></div>")(input)?;

    let message = Message {
        id,
        from_id,
        date: date.timestamp() + TIME_ZONE_CORRECTION,
        message_text,
    };

    Ok((input, message))
}
```

```
/*
struct Attachment_<'a> {
    description: &'a str,
    data: Option<&'a str>
}

#[non_exhaustive]
enum Attachment<'a> {
    Photo{
        link: &'a str,
    },
    Video{
        link: &'a str,
    },
    Audio,
    Doc{
        link: &'a str,
    },
    Link{
        link:&'a str
    },
    Market, // товар
    MarketAlbum, // подборка товаров
    Wall, // запись на стене
    WallReply, // коментарий на стене
    Sticker,
    Graphity,
    Gift,
    Storie, // история
    StoriesSeq, // сюжет
    Poll, // опрос
    LinkedMessaegs{
        count: usize,
    }//LinkedMessaegs(Vec<Message<'a>>), // type recursion???
    DeletedMessage,
    Map,
    Call,
    Pdocast,
    AppAction,
    Musician,
    MoneyTransferRequest,
    MoneyTransfer,
    Playlist,
    Article,
    Band
    // else ???
}

/*
struct VkChat<'a> {
    id: isize, // can be negative
    title: String,
    users: Vec<usize>, // id-s
    photoURL: String,
    messages: Vec<Message<'a>> // can be very long
}

enum ChatType{
    Direct,
    Community,
    Group,
}
*/

*/
```