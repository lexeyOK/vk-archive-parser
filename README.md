# vk-archive-parser
parse vk messages from archive
uses rust (**nightly**) and some cool crates
you will need to setup it first as it is still **WIP**

# setup
get archive from vk with only messages from [here](https://vk.com/data_protection?section=rules)

so you basically execute `utils.sh` line by line
you will need to use `fd` or `find` (idk how to do this in `find` use `xargs` :lips:)
```bash
> mkdir vk;cd vk
> unzip Archive.zip
> mkdir -p ../vk_utf8
> fd -t d -x mkdir -p ../vk_utf8/{}
> fd -e html -x iconv -f WINDOWS-1251 -t UTF-8 {} -o ../vk_utf8/{.}.html
> fd -e html -x sed -i 's/windows-1251/utf-8/g' {}
```

change `TIME_ZONE_CORRECTION` and `SELF_ID_URL` in `src/main.rs` \
to your timezone in seconds of GMT offset (GMT+5 -> 5*3600) \
and your vk account url
then you do 
```bash
> cargo b -r
> mkdir json;cd json
> fd -t d --search-path ~/dev/vk_utf8/messages/ -x ./../target/release/vk-archive-parser {}
```
this will parse chats into json files and painc if something is wrong (my bad)
# STILL :warning:WORK IN PROGRESS:warning: DON"T USE 
it will parse all message*.html form single chat into CHAT_ID.json \
attachments are still waky and messages are just inner_text of `.message` div

