# vk-archive-parser
parse vk messages from archive

# setup
get archive from vk with only messages from [vk data protaction section](https://vk.com/data_protection?section=rules)

you will need to use [`fd`](https://github.com/sharkdp/fd) or `find`

change `TIME_ZONE_CORRECTION` in `src/main.rs` \
to your timezone in seconds of GMT offset (GMT+5 -> 5*3600)\

```bash
> mkdir vk && cd vk
> unzip Archive.zip
> fd -e html cat > /dev/null # notify system to load for faster(?) acsess
> cargo b -r
> cd messages
> fd -t d -x ./../../target/release/vk-archive-parser {}
> mkdir ../json && mv -- *.json ../json && cd .. && rm -fr messages
```

# STILL :warning:WORK IN PROGRESS:warning: 
it will parse all message*.html form single chat into CHAT_ID.json
