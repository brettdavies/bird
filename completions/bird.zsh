#compdef bird

autoload -U is-at-least

_bird() {
    typeset -A opt_args
    typeset -a _arguments_options
    local ret=1

    if is-at-least 5.2; then
        _arguments_options=(-s -S -C)
    else
        _arguments_options=(-s -C)
    fi

    local context curcontext="$curcontext" state line
    _arguments "${_arguments_options[@]}" : \
'-u+[Signed-in account to use when the app holds tokens for several (as \`xr -u\`)]:USERNAME:_default' \
'--username=[Signed-in account to use when the app holds tokens for several (as \`xr -u\`)]:USERNAME:_default' \
'-o+[Output format (text, json, jsonl, ndjson). Defaults to json when piped]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON envelope, no color"
jsonl\:"Streaming line-delimited JSON (one object per line; no wrapper)"
ndjson\:"Newline-delimited JSON, accepted as an alias for jsonl"))' \
'--output=[Output format (text, json, jsonl, ndjson). Defaults to json when piped]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON envelope, no color"
jsonl\:"Streaming line-delimited JSON (one object per line; no wrapper)"
ndjson\:"Newline-delimited JSON, accepted as an alias for jsonl"))' \
'--color=[Color mode\: auto (default), always, never]:COLOR:((auto\:"Auto-detect\: color when stderr is a TTY and \`NO_COLOR\` is unset"
always\:"Always emit colors"
never\:"Never emit colors"))' \
'--timeout=[Network timeout in seconds (default 30) for each X API request]:TIMEOUT:_default' \
'--limit=[Maximum number of results to return on list-style commands (default 100, ceiling 1000)]:N:_default' \
'--cursor=[Pagination cursor token for list-style commands (X API \`pagination_token\`/\`next_token\`)]:TOKEN:_default' \
'--app=[App in the token store to use. \`--app\` wins over \`BIRD_APP\`, which wins over the store'\''s default app. bird does not read \`XURL_APP\`; when it is set and \`BIRD_APP\` is not, commands that call the X API print a warning]:NAME:_default' \
'--auth=[Per-call auth scheme override\: \`app\`, \`oauth1\`, \`oauth2\`, or \`none\`. \`none\` is refused for a command whose X API endpoint requires authentication]:SCHEME:_default' \
'(-o --output --jsonl)--json[Shorthand for \`--output json\`]' \
'(-o --output)--jsonl[Shorthand for \`--output jsonl\`]' \
'-q[Suppress informational stderr output (keep only fatal errors)]' \
'--quiet[Suppress informational stderr output (keep only fatal errors)]' \
'*-v[Increase verbosity (repeatable\: -v info, -vv debug, -vvv trace)]' \
'*--verbose[Increase verbosity (repeatable\: -v info, -vv debug, -vvv trace)]' \
'--no-interactive[Disable interactive prompts (refuse anything that would block on stdin)]' \
'--raw[Emit unstyled, compact output\: pipe-safe text, and \`json\` on one line]' \
'--examples[Print curated examples block and exit]' \
'--refresh[Send every read instead of answering it from a stored response; still store the reply]' \
'--no-cache[Read and write no stored responses, notes, or ledger records]' \
'--cache-only[Answer only from stored responses; never make API requests]' \
'-h[Print help (see more with '\''--help'\'')]' \
'--help[Print help (see more with '\''--help'\'')]' \
'-V[Print version]' \
'--version[Print version]' \
":: :_bird_commands" \
"*::: :->bird" \
&& ret=0
    case $state in
    (bird)
        words=($line[1] "${words[@]}")
        (( CURRENT += 1 ))
        curcontext="${curcontext%:*:*}:bird-command-$line[1]:"
        case $line[1] in
            (login)
_arguments "${_arguments_options[@]}" : \
'-u+[Signed-in account to use when the app holds tokens for several (as \`xr -u\`)]:USERNAME:_default' \
'--username=[Signed-in account to use when the app holds tokens for several (as \`xr -u\`)]:USERNAME:_default' \
'-o+[Output format (text, json, jsonl, ndjson). Defaults to json when piped]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON envelope, no color"
jsonl\:"Streaming line-delimited JSON (one object per line; no wrapper)"
ndjson\:"Newline-delimited JSON, accepted as an alias for jsonl"))' \
'--output=[Output format (text, json, jsonl, ndjson). Defaults to json when piped]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON envelope, no color"
jsonl\:"Streaming line-delimited JSON (one object per line; no wrapper)"
ndjson\:"Newline-delimited JSON, accepted as an alias for jsonl"))' \
'--color=[Color mode\: auto (default), always, never]:COLOR:((auto\:"Auto-detect\: color when stderr is a TTY and \`NO_COLOR\` is unset"
always\:"Always emit colors"
never\:"Never emit colors"))' \
'--timeout=[Network timeout in seconds (default 30) for each X API request]:TIMEOUT:_default' \
'--limit=[Maximum number of results to return on list-style commands (default 100, ceiling 1000)]:N:_default' \
'--cursor=[Pagination cursor token for list-style commands (X API \`pagination_token\`/\`next_token\`)]:TOKEN:_default' \
'--app=[App in the token store to use. \`--app\` wins over \`BIRD_APP\`, which wins over the store'\''s default app. bird does not read \`XURL_APP\`; when it is set and \`BIRD_APP\` is not, commands that call the X API print a warning]:NAME:_default' \
'--auth=[Per-call auth scheme override\: \`app\`, \`oauth1\`, \`oauth2\`, or \`none\`. \`none\` is refused for a command whose X API endpoint requires authentication]:SCHEME:_default' \
'--no-browser[Print the authorization URL on stdout and read the redirect URL back from stdin. No browser is launched]' \
'(-o --output --jsonl)--json[Shorthand for \`--output json\`]' \
'(-o --output)--jsonl[Shorthand for \`--output jsonl\`]' \
'-q[Suppress informational stderr output (keep only fatal errors)]' \
'--quiet[Suppress informational stderr output (keep only fatal errors)]' \
'*-v[Increase verbosity (repeatable\: -v info, -vv debug, -vvv trace)]' \
'*--verbose[Increase verbosity (repeatable\: -v info, -vv debug, -vvv trace)]' \
'--no-interactive[Disable interactive prompts (refuse anything that would block on stdin)]' \
'--raw[Emit unstyled, compact output\: pipe-safe text, and \`json\` on one line]' \
'--examples[Print curated examples block and exit]' \
'--refresh[Send every read instead of answering it from a stored response; still store the reply]' \
'--no-cache[Read and write no stored responses, notes, or ledger records]' \
'--cache-only[Answer only from stored responses; never make API requests]' \
'-h[Print help (see more with '\''--help'\'')]' \
'--help[Print help (see more with '\''--help'\'')]' \
&& ret=0
;;
(me)
_arguments "${_arguments_options[@]}" : \
'-u+[Signed-in account to use when the app holds tokens for several (as \`xr -u\`)]:USERNAME:_default' \
'--username=[Signed-in account to use when the app holds tokens for several (as \`xr -u\`)]:USERNAME:_default' \
'-o+[Output format (text, json, jsonl, ndjson). Defaults to json when piped]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON envelope, no color"
jsonl\:"Streaming line-delimited JSON (one object per line; no wrapper)"
ndjson\:"Newline-delimited JSON, accepted as an alias for jsonl"))' \
'--output=[Output format (text, json, jsonl, ndjson). Defaults to json when piped]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON envelope, no color"
jsonl\:"Streaming line-delimited JSON (one object per line; no wrapper)"
ndjson\:"Newline-delimited JSON, accepted as an alias for jsonl"))' \
'--color=[Color mode\: auto (default), always, never]:COLOR:((auto\:"Auto-detect\: color when stderr is a TTY and \`NO_COLOR\` is unset"
always\:"Always emit colors"
never\:"Never emit colors"))' \
'--timeout=[Network timeout in seconds (default 30) for each X API request]:TIMEOUT:_default' \
'--limit=[Maximum number of results to return on list-style commands (default 100, ceiling 1000)]:N:_default' \
'--cursor=[Pagination cursor token for list-style commands (X API \`pagination_token\`/\`next_token\`)]:TOKEN:_default' \
'--app=[App in the token store to use. \`--app\` wins over \`BIRD_APP\`, which wins over the store'\''s default app. bird does not read \`XURL_APP\`; when it is set and \`BIRD_APP\` is not, commands that call the X API print a warning]:NAME:_default' \
'--auth=[Per-call auth scheme override\: \`app\`, \`oauth1\`, \`oauth2\`, or \`none\`. \`none\` is refused for a command whose X API endpoint requires authentication]:SCHEME:_default' \
'--pretty[Pretty-print human-readable output]' \
'(-o --output --jsonl)--json[Shorthand for \`--output json\`]' \
'(-o --output)--jsonl[Shorthand for \`--output jsonl\`]' \
'-q[Suppress informational stderr output (keep only fatal errors)]' \
'--quiet[Suppress informational stderr output (keep only fatal errors)]' \
'*-v[Increase verbosity (repeatable\: -v info, -vv debug, -vvv trace)]' \
'*--verbose[Increase verbosity (repeatable\: -v info, -vv debug, -vvv trace)]' \
'--no-interactive[Disable interactive prompts (refuse anything that would block on stdin)]' \
'--raw[Emit unstyled, compact output\: pipe-safe text, and \`json\` on one line]' \
'--examples[Print curated examples block and exit]' \
'--refresh[Send every read instead of answering it from a stored response; still store the reply]' \
'--no-cache[Read and write no stored responses, notes, or ledger records]' \
'--cache-only[Answer only from stored responses; never make API requests]' \
'-h[Print help (see more with '\''--help'\'')]' \
'--help[Print help (see more with '\''--help'\'')]' \
&& ret=0
;;
(bookmarks)
_arguments "${_arguments_options[@]}" : \
'-u+[Signed-in account to use when the app holds tokens for several (as \`xr -u\`)]:USERNAME:_default' \
'--username=[Signed-in account to use when the app holds tokens for several (as \`xr -u\`)]:USERNAME:_default' \
'-o+[Output format (text, json, jsonl, ndjson). Defaults to json when piped]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON envelope, no color"
jsonl\:"Streaming line-delimited JSON (one object per line; no wrapper)"
ndjson\:"Newline-delimited JSON, accepted as an alias for jsonl"))' \
'--output=[Output format (text, json, jsonl, ndjson). Defaults to json when piped]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON envelope, no color"
jsonl\:"Streaming line-delimited JSON (one object per line; no wrapper)"
ndjson\:"Newline-delimited JSON, accepted as an alias for jsonl"))' \
'--color=[Color mode\: auto (default), always, never]:COLOR:((auto\:"Auto-detect\: color when stderr is a TTY and \`NO_COLOR\` is unset"
always\:"Always emit colors"
never\:"Never emit colors"))' \
'--timeout=[Network timeout in seconds (default 30) for each X API request]:TIMEOUT:_default' \
'--limit=[Maximum number of results to return on list-style commands (default 100, ceiling 1000)]:N:_default' \
'--cursor=[Pagination cursor token for list-style commands (X API \`pagination_token\`/\`next_token\`)]:TOKEN:_default' \
'--app=[App in the token store to use. \`--app\` wins over \`BIRD_APP\`, which wins over the store'\''s default app. bird does not read \`XURL_APP\`; when it is set and \`BIRD_APP\` is not, commands that call the X API print a warning]:NAME:_default' \
'--auth=[Per-call auth scheme override\: \`app\`, \`oauth1\`, \`oauth2\`, or \`none\`. \`none\` is refused for a command whose X API endpoint requires authentication]:SCHEME:_default' \
'--pretty[Pretty-print human-readable output]' \
'(-o --output --jsonl)--json[Shorthand for \`--output json\`]' \
'(-o --output)--jsonl[Shorthand for \`--output jsonl\`]' \
'-q[Suppress informational stderr output (keep only fatal errors)]' \
'--quiet[Suppress informational stderr output (keep only fatal errors)]' \
'*-v[Increase verbosity (repeatable\: -v info, -vv debug, -vvv trace)]' \
'*--verbose[Increase verbosity (repeatable\: -v info, -vv debug, -vvv trace)]' \
'--no-interactive[Disable interactive prompts (refuse anything that would block on stdin)]' \
'--raw[Emit unstyled, compact output\: pipe-safe text, and \`json\` on one line]' \
'--examples[Print curated examples block and exit]' \
'--refresh[Send every read instead of answering it from a stored response; still store the reply]' \
'--no-cache[Read and write no stored responses, notes, or ledger records]' \
'--cache-only[Answer only from stored responses; never make API requests]' \
'-h[Print help (see more with '\''--help'\'')]' \
'--help[Print help (see more with '\''--help'\'')]' \
&& ret=0
;;
(profile)
_arguments "${_arguments_options[@]}" : \
'-o+[Output format (text, json, jsonl, ndjson). Defaults to json when piped]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON envelope, no color"
jsonl\:"Streaming line-delimited JSON (one object per line; no wrapper)"
ndjson\:"Newline-delimited JSON, accepted as an alias for jsonl"))' \
'--output=[Output format (text, json, jsonl, ndjson). Defaults to json when piped]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON envelope, no color"
jsonl\:"Streaming line-delimited JSON (one object per line; no wrapper)"
ndjson\:"Newline-delimited JSON, accepted as an alias for jsonl"))' \
'--color=[Color mode\: auto (default), always, never]:COLOR:((auto\:"Auto-detect\: color when stderr is a TTY and \`NO_COLOR\` is unset"
always\:"Always emit colors"
never\:"Never emit colors"))' \
'--timeout=[Network timeout in seconds (default 30) for each X API request]:TIMEOUT:_default' \
'--limit=[Maximum number of results to return on list-style commands (default 100, ceiling 1000)]:N:_default' \
'--cursor=[Pagination cursor token for list-style commands (X API \`pagination_token\`/\`next_token\`)]:TOKEN:_default' \
'--app=[App in the token store to use. \`--app\` wins over \`BIRD_APP\`, which wins over the store'\''s default app. bird does not read \`XURL_APP\`; when it is set and \`BIRD_APP\` is not, commands that call the X API print a warning]:NAME:_default' \
'--auth=[Per-call auth scheme override\: \`app\`, \`oauth1\`, \`oauth2\`, or \`none\`. \`none\` is refused for a command whose X API endpoint requires authentication]:SCHEME:_default' \
'--pretty[Pretty-print human-readable output]' \
'(-o --output --jsonl)--json[Shorthand for \`--output json\`]' \
'(-o --output)--jsonl[Shorthand for \`--output jsonl\`]' \
'-q[Suppress informational stderr output (keep only fatal errors)]' \
'--quiet[Suppress informational stderr output (keep only fatal errors)]' \
'*-v[Increase verbosity (repeatable\: -v info, -vv debug, -vvv trace)]' \
'*--verbose[Increase verbosity (repeatable\: -v info, -vv debug, -vvv trace)]' \
'--no-interactive[Disable interactive prompts (refuse anything that would block on stdin)]' \
'--raw[Emit unstyled, compact output\: pipe-safe text, and \`json\` on one line]' \
'--examples[Print curated examples block and exit]' \
'--refresh[Send every read instead of answering it from a stored response; still store the reply]' \
'--no-cache[Read and write no stored responses, notes, or ledger records]' \
'--cache-only[Answer only from stored responses; never make API requests]' \
'-h[Print help (see more with '\''--help'\'')]' \
'--help[Print help (see more with '\''--help'\'')]' \
':username -- X/Twitter username (with or without @):_default' \
&& ret=0
;;
(search)
_arguments "${_arguments_options[@]}" : \
'--sort=[Sort results\: recent (default), likes]:SORT:_default' \
'--min-likes=[Minimum like count threshold]:MIN_LIKES:_default' \
'--max-results=[Maximum results per page (10-100, default\: 100)]:MAX_RESULTS:_default' \
'--pages=[Number of pages to fetch (1-10, default\: 1)]:PAGES:_default' \
'-u+[Signed-in account to use when the app holds tokens for several (as \`xr -u\`)]:USERNAME:_default' \
'--username=[Signed-in account to use when the app holds tokens for several (as \`xr -u\`)]:USERNAME:_default' \
'-o+[Output format (text, json, jsonl, ndjson). Defaults to json when piped]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON envelope, no color"
jsonl\:"Streaming line-delimited JSON (one object per line; no wrapper)"
ndjson\:"Newline-delimited JSON, accepted as an alias for jsonl"))' \
'--output=[Output format (text, json, jsonl, ndjson). Defaults to json when piped]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON envelope, no color"
jsonl\:"Streaming line-delimited JSON (one object per line; no wrapper)"
ndjson\:"Newline-delimited JSON, accepted as an alias for jsonl"))' \
'--color=[Color mode\: auto (default), always, never]:COLOR:((auto\:"Auto-detect\: color when stderr is a TTY and \`NO_COLOR\` is unset"
always\:"Always emit colors"
never\:"Never emit colors"))' \
'--timeout=[Network timeout in seconds (default 30) for each X API request]:TIMEOUT:_default' \
'--limit=[Maximum number of results to return on list-style commands (default 100, ceiling 1000)]:N:_default' \
'--cursor=[Pagination cursor token for list-style commands (X API \`pagination_token\`/\`next_token\`)]:TOKEN:_default' \
'--app=[App in the token store to use. \`--app\` wins over \`BIRD_APP\`, which wins over the store'\''s default app. bird does not read \`XURL_APP\`; when it is set and \`BIRD_APP\` is not, commands that call the X API print a warning]:NAME:_default' \
'--auth=[Per-call auth scheme override\: \`app\`, \`oauth1\`, \`oauth2\`, or \`none\`. \`none\` is refused for a command whose X API endpoint requires authentication]:SCHEME:_default' \
'--pretty[Pretty-print human-readable output]' \
'(-o --output --jsonl)--json[Shorthand for \`--output json\`]' \
'(-o --output)--jsonl[Shorthand for \`--output jsonl\`]' \
'-q[Suppress informational stderr output (keep only fatal errors)]' \
'--quiet[Suppress informational stderr output (keep only fatal errors)]' \
'*-v[Increase verbosity (repeatable\: -v info, -vv debug, -vvv trace)]' \
'*--verbose[Increase verbosity (repeatable\: -v info, -vv debug, -vvv trace)]' \
'--no-interactive[Disable interactive prompts (refuse anything that would block on stdin)]' \
'--raw[Emit unstyled, compact output\: pipe-safe text, and \`json\` on one line]' \
'--examples[Print curated examples block and exit]' \
'--refresh[Send every read instead of answering it from a stored response; still store the reply]' \
'--no-cache[Read and write no stored responses, notes, or ledger records]' \
'--cache-only[Answer only from stored responses; never make API requests]' \
'-h[Print help (see more with '\''--help'\'')]' \
'--help[Print help (see more with '\''--help'\'')]' \
':query -- Search query (X API search syntax):_default' \
&& ret=0
;;
(thread)
_arguments "${_arguments_options[@]}" : \
'--max-pages=[Maximum number of search result pages (default\: 10, max\: 25)]:MAX_PAGES:_default' \
'-u+[Signed-in account to use when the app holds tokens for several (as \`xr -u\`)]:USERNAME:_default' \
'--username=[Signed-in account to use when the app holds tokens for several (as \`xr -u\`)]:USERNAME:_default' \
'-o+[Output format (text, json, jsonl, ndjson). Defaults to json when piped]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON envelope, no color"
jsonl\:"Streaming line-delimited JSON (one object per line; no wrapper)"
ndjson\:"Newline-delimited JSON, accepted as an alias for jsonl"))' \
'--output=[Output format (text, json, jsonl, ndjson). Defaults to json when piped]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON envelope, no color"
jsonl\:"Streaming line-delimited JSON (one object per line; no wrapper)"
ndjson\:"Newline-delimited JSON, accepted as an alias for jsonl"))' \
'--color=[Color mode\: auto (default), always, never]:COLOR:((auto\:"Auto-detect\: color when stderr is a TTY and \`NO_COLOR\` is unset"
always\:"Always emit colors"
never\:"Never emit colors"))' \
'--timeout=[Network timeout in seconds (default 30) for each X API request]:TIMEOUT:_default' \
'--limit=[Maximum number of results to return on list-style commands (default 100, ceiling 1000)]:N:_default' \
'--cursor=[Pagination cursor token for list-style commands (X API \`pagination_token\`/\`next_token\`)]:TOKEN:_default' \
'--app=[App in the token store to use. \`--app\` wins over \`BIRD_APP\`, which wins over the store'\''s default app. bird does not read \`XURL_APP\`; when it is set and \`BIRD_APP\` is not, commands that call the X API print a warning]:NAME:_default' \
'--auth=[Per-call auth scheme override\: \`app\`, \`oauth1\`, \`oauth2\`, or \`none\`. \`none\` is refused for a command whose X API endpoint requires authentication]:SCHEME:_default' \
'--pretty[Pretty-print human-readable output]' \
'(-o --output --jsonl)--json[Shorthand for \`--output json\`]' \
'(-o --output)--jsonl[Shorthand for \`--output jsonl\`]' \
'-q[Suppress informational stderr output (keep only fatal errors)]' \
'--quiet[Suppress informational stderr output (keep only fatal errors)]' \
'*-v[Increase verbosity (repeatable\: -v info, -vv debug, -vvv trace)]' \
'*--verbose[Increase verbosity (repeatable\: -v info, -vv debug, -vvv trace)]' \
'--no-interactive[Disable interactive prompts (refuse anything that would block on stdin)]' \
'--raw[Emit unstyled, compact output\: pipe-safe text, and \`json\` on one line]' \
'--examples[Print curated examples block and exit]' \
'--refresh[Send every read instead of answering it from a stored response; still store the reply]' \
'--no-cache[Read and write no stored responses, notes, or ledger records]' \
'--cache-only[Answer only from stored responses; never make API requests]' \
'-h[Print help (see more with '\''--help'\'')]' \
'--help[Print help (see more with '\''--help'\'')]' \
':post_id -- Post ID (the root post or any reply in the thread):_default' \
&& ret=0
;;
(watchlist)
_arguments "${_arguments_options[@]}" : \
'-u+[Signed-in account to use when the app holds tokens for several (as \`xr -u\`)]:USERNAME:_default' \
'--username=[Signed-in account to use when the app holds tokens for several (as \`xr -u\`)]:USERNAME:_default' \
'-o+[Output format (text, json, jsonl, ndjson). Defaults to json when piped]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON envelope, no color"
jsonl\:"Streaming line-delimited JSON (one object per line; no wrapper)"
ndjson\:"Newline-delimited JSON, accepted as an alias for jsonl"))' \
'--output=[Output format (text, json, jsonl, ndjson). Defaults to json when piped]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON envelope, no color"
jsonl\:"Streaming line-delimited JSON (one object per line; no wrapper)"
ndjson\:"Newline-delimited JSON, accepted as an alias for jsonl"))' \
'--color=[Color mode\: auto (default), always, never]:COLOR:((auto\:"Auto-detect\: color when stderr is a TTY and \`NO_COLOR\` is unset"
always\:"Always emit colors"
never\:"Never emit colors"))' \
'--timeout=[Network timeout in seconds (default 30) for each X API request]:TIMEOUT:_default' \
'--limit=[Maximum number of results to return on list-style commands (default 100, ceiling 1000)]:N:_default' \
'--cursor=[Pagination cursor token for list-style commands (X API \`pagination_token\`/\`next_token\`)]:TOKEN:_default' \
'--app=[App in the token store to use. \`--app\` wins over \`BIRD_APP\`, which wins over the store'\''s default app. bird does not read \`XURL_APP\`; when it is set and \`BIRD_APP\` is not, commands that call the X API print a warning]:NAME:_default' \
'--auth=[Per-call auth scheme override\: \`app\`, \`oauth1\`, \`oauth2\`, or \`none\`. \`none\` is refused for a command whose X API endpoint requires authentication]:SCHEME:_default' \
'--pretty[Pretty-print human-readable output]' \
'(-o --output --jsonl)--json[Shorthand for \`--output json\`]' \
'(-o --output)--jsonl[Shorthand for \`--output jsonl\`]' \
'-q[Suppress informational stderr output (keep only fatal errors)]' \
'--quiet[Suppress informational stderr output (keep only fatal errors)]' \
'*-v[Increase verbosity (repeatable\: -v info, -vv debug, -vvv trace)]' \
'*--verbose[Increase verbosity (repeatable\: -v info, -vv debug, -vvv trace)]' \
'--no-interactive[Disable interactive prompts (refuse anything that would block on stdin)]' \
'--raw[Emit unstyled, compact output\: pipe-safe text, and \`json\` on one line]' \
'--examples[Print curated examples block and exit]' \
'--refresh[Send every read instead of answering it from a stored response; still store the reply]' \
'--no-cache[Read and write no stored responses, notes, or ledger records]' \
'--cache-only[Answer only from stored responses; never make API requests]' \
'-h[Print help (see more with '\''--help'\'')]' \
'--help[Print help (see more with '\''--help'\'')]' \
":: :_bird__subcmd__watchlist_commands" \
"*::: :->watchlist" \
&& ret=0

    case $state in
    (watchlist)
        words=($line[1] "${words[@]}")
        (( CURRENT += 1 ))
        curcontext="${curcontext%:*:*}:bird-watchlist-command-$line[1]:"
        case $line[1] in
            (fetch)
_arguments "${_arguments_options[@]}" : \
'-u+[Signed-in account to use when the app holds tokens for several (as \`xr -u\`)]:USERNAME:_default' \
'--username=[Signed-in account to use when the app holds tokens for several (as \`xr -u\`)]:USERNAME:_default' \
'-o+[Output format (text, json, jsonl, ndjson). Defaults to json when piped]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON envelope, no color"
jsonl\:"Streaming line-delimited JSON (one object per line; no wrapper)"
ndjson\:"Newline-delimited JSON, accepted as an alias for jsonl"))' \
'--output=[Output format (text, json, jsonl, ndjson). Defaults to json when piped]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON envelope, no color"
jsonl\:"Streaming line-delimited JSON (one object per line; no wrapper)"
ndjson\:"Newline-delimited JSON, accepted as an alias for jsonl"))' \
'--color=[Color mode\: auto (default), always, never]:COLOR:((auto\:"Auto-detect\: color when stderr is a TTY and \`NO_COLOR\` is unset"
always\:"Always emit colors"
never\:"Never emit colors"))' \
'--timeout=[Network timeout in seconds (default 30) for each X API request]:TIMEOUT:_default' \
'--limit=[Maximum number of results to return on list-style commands (default 100, ceiling 1000)]:N:_default' \
'--cursor=[Pagination cursor token for list-style commands (X API \`pagination_token\`/\`next_token\`)]:TOKEN:_default' \
'--app=[App in the token store to use. \`--app\` wins over \`BIRD_APP\`, which wins over the store'\''s default app. bird does not read \`XURL_APP\`; when it is set and \`BIRD_APP\` is not, commands that call the X API print a warning]:NAME:_default' \
'--auth=[Per-call auth scheme override\: \`app\`, \`oauth1\`, \`oauth2\`, or \`none\`. \`none\` is refused for a command whose X API endpoint requires authentication]:SCHEME:_default' \
'(-o --output --jsonl)--json[Shorthand for \`--output json\`]' \
'(-o --output)--jsonl[Shorthand for \`--output jsonl\`]' \
'-q[Suppress informational stderr output (keep only fatal errors)]' \
'--quiet[Suppress informational stderr output (keep only fatal errors)]' \
'*-v[Increase verbosity (repeatable\: -v info, -vv debug, -vvv trace)]' \
'*--verbose[Increase verbosity (repeatable\: -v info, -vv debug, -vvv trace)]' \
'--no-interactive[Disable interactive prompts (refuse anything that would block on stdin)]' \
'--raw[Emit unstyled, compact output\: pipe-safe text, and \`json\` on one line]' \
'--examples[Print curated examples block and exit]' \
'--refresh[Send every read instead of answering it from a stored response; still store the reply]' \
'--no-cache[Read and write no stored responses, notes, or ledger records]' \
'--cache-only[Answer only from stored responses; never make API requests]' \
'-h[Print help (see more with '\''--help'\'')]' \
'--help[Print help (see more with '\''--help'\'')]' \
&& ret=0
;;
(add)
_arguments "${_arguments_options[@]}" : \
'-o+[Output format (text, json, jsonl, ndjson). Defaults to json when piped]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON envelope, no color"
jsonl\:"Streaming line-delimited JSON (one object per line; no wrapper)"
ndjson\:"Newline-delimited JSON, accepted as an alias for jsonl"))' \
'--output=[Output format (text, json, jsonl, ndjson). Defaults to json when piped]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON envelope, no color"
jsonl\:"Streaming line-delimited JSON (one object per line; no wrapper)"
ndjson\:"Newline-delimited JSON, accepted as an alias for jsonl"))' \
'--color=[Color mode\: auto (default), always, never]:COLOR:((auto\:"Auto-detect\: color when stderr is a TTY and \`NO_COLOR\` is unset"
always\:"Always emit colors"
never\:"Never emit colors"))' \
'--timeout=[Network timeout in seconds (default 30) for each X API request]:TIMEOUT:_default' \
'--limit=[Maximum number of results to return on list-style commands (default 100, ceiling 1000)]:N:_default' \
'--cursor=[Pagination cursor token for list-style commands (X API \`pagination_token\`/\`next_token\`)]:TOKEN:_default' \
'--app=[App in the token store to use. \`--app\` wins over \`BIRD_APP\`, which wins over the store'\''s default app. bird does not read \`XURL_APP\`; when it is set and \`BIRD_APP\` is not, commands that call the X API print a warning]:NAME:_default' \
'--auth=[Per-call auth scheme override\: \`app\`, \`oauth1\`, \`oauth2\`, or \`none\`. \`none\` is refused for a command whose X API endpoint requires authentication]:SCHEME:_default' \
'(-o --output --jsonl)--json[Shorthand for \`--output json\`]' \
'(-o --output)--jsonl[Shorthand for \`--output jsonl\`]' \
'-q[Suppress informational stderr output (keep only fatal errors)]' \
'--quiet[Suppress informational stderr output (keep only fatal errors)]' \
'*-v[Increase verbosity (repeatable\: -v info, -vv debug, -vvv trace)]' \
'*--verbose[Increase verbosity (repeatable\: -v info, -vv debug, -vvv trace)]' \
'--no-interactive[Disable interactive prompts (refuse anything that would block on stdin)]' \
'--raw[Emit unstyled, compact output\: pipe-safe text, and \`json\` on one line]' \
'--examples[Print curated examples block and exit]' \
'--refresh[Send every read instead of answering it from a stored response; still store the reply]' \
'--no-cache[Read and write no stored responses, notes, or ledger records]' \
'--cache-only[Answer only from stored responses; never make API requests]' \
'-h[Print help (see more with '\''--help'\'')]' \
'--help[Print help (see more with '\''--help'\'')]' \
':username -- X username (with or without @):_default' \
&& ret=0
;;
(remove)
_arguments "${_arguments_options[@]}" : \
'-o+[Output format (text, json, jsonl, ndjson). Defaults to json when piped]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON envelope, no color"
jsonl\:"Streaming line-delimited JSON (one object per line; no wrapper)"
ndjson\:"Newline-delimited JSON, accepted as an alias for jsonl"))' \
'--output=[Output format (text, json, jsonl, ndjson). Defaults to json when piped]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON envelope, no color"
jsonl\:"Streaming line-delimited JSON (one object per line; no wrapper)"
ndjson\:"Newline-delimited JSON, accepted as an alias for jsonl"))' \
'--color=[Color mode\: auto (default), always, never]:COLOR:((auto\:"Auto-detect\: color when stderr is a TTY and \`NO_COLOR\` is unset"
always\:"Always emit colors"
never\:"Never emit colors"))' \
'--timeout=[Network timeout in seconds (default 30) for each X API request]:TIMEOUT:_default' \
'--limit=[Maximum number of results to return on list-style commands (default 100, ceiling 1000)]:N:_default' \
'--cursor=[Pagination cursor token for list-style commands (X API \`pagination_token\`/\`next_token\`)]:TOKEN:_default' \
'--app=[App in the token store to use. \`--app\` wins over \`BIRD_APP\`, which wins over the store'\''s default app. bird does not read \`XURL_APP\`; when it is set and \`BIRD_APP\` is not, commands that call the X API print a warning]:NAME:_default' \
'--auth=[Per-call auth scheme override\: \`app\`, \`oauth1\`, \`oauth2\`, or \`none\`. \`none\` is refused for a command whose X API endpoint requires authentication]:SCHEME:_default' \
'-f[Skip the interactive confirmation prompt (alias\: --yes)]' \
'--force[Skip the interactive confirmation prompt (alias\: --yes)]' \
'--dry-run[Validate inputs and print the would-be request, then exit without calling the API]' \
'(-o --output --jsonl)--json[Shorthand for \`--output json\`]' \
'(-o --output)--jsonl[Shorthand for \`--output jsonl\`]' \
'-q[Suppress informational stderr output (keep only fatal errors)]' \
'--quiet[Suppress informational stderr output (keep only fatal errors)]' \
'*-v[Increase verbosity (repeatable\: -v info, -vv debug, -vvv trace)]' \
'*--verbose[Increase verbosity (repeatable\: -v info, -vv debug, -vvv trace)]' \
'--no-interactive[Disable interactive prompts (refuse anything that would block on stdin)]' \
'--raw[Emit unstyled, compact output\: pipe-safe text, and \`json\` on one line]' \
'--examples[Print curated examples block and exit]' \
'--refresh[Send every read instead of answering it from a stored response; still store the reply]' \
'--no-cache[Read and write no stored responses, notes, or ledger records]' \
'--cache-only[Answer only from stored responses; never make API requests]' \
'-h[Print help (see more with '\''--help'\'')]' \
'--help[Print help (see more with '\''--help'\'')]' \
':username -- X username to remove:_default' \
&& ret=0
;;
(list)
_arguments "${_arguments_options[@]}" : \
'-u+[Signed-in account to use when the app holds tokens for several (as \`xr -u\`)]:USERNAME:_default' \
'--username=[Signed-in account to use when the app holds tokens for several (as \`xr -u\`)]:USERNAME:_default' \
'-o+[Output format (text, json, jsonl, ndjson). Defaults to json when piped]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON envelope, no color"
jsonl\:"Streaming line-delimited JSON (one object per line; no wrapper)"
ndjson\:"Newline-delimited JSON, accepted as an alias for jsonl"))' \
'--output=[Output format (text, json, jsonl, ndjson). Defaults to json when piped]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON envelope, no color"
jsonl\:"Streaming line-delimited JSON (one object per line; no wrapper)"
ndjson\:"Newline-delimited JSON, accepted as an alias for jsonl"))' \
'--color=[Color mode\: auto (default), always, never]:COLOR:((auto\:"Auto-detect\: color when stderr is a TTY and \`NO_COLOR\` is unset"
always\:"Always emit colors"
never\:"Never emit colors"))' \
'--timeout=[Network timeout in seconds (default 30) for each X API request]:TIMEOUT:_default' \
'--limit=[Maximum number of results to return on list-style commands (default 100, ceiling 1000)]:N:_default' \
'--cursor=[Pagination cursor token for list-style commands (X API \`pagination_token\`/\`next_token\`)]:TOKEN:_default' \
'--app=[App in the token store to use. \`--app\` wins over \`BIRD_APP\`, which wins over the store'\''s default app. bird does not read \`XURL_APP\`; when it is set and \`BIRD_APP\` is not, commands that call the X API print a warning]:NAME:_default' \
'--auth=[Per-call auth scheme override\: \`app\`, \`oauth1\`, \`oauth2\`, or \`none\`. \`none\` is refused for a command whose X API endpoint requires authentication]:SCHEME:_default' \
'(-o --output --jsonl)--json[Shorthand for \`--output json\`]' \
'(-o --output)--jsonl[Shorthand for \`--output jsonl\`]' \
'-q[Suppress informational stderr output (keep only fatal errors)]' \
'--quiet[Suppress informational stderr output (keep only fatal errors)]' \
'*-v[Increase verbosity (repeatable\: -v info, -vv debug, -vvv trace)]' \
'*--verbose[Increase verbosity (repeatable\: -v info, -vv debug, -vvv trace)]' \
'--no-interactive[Disable interactive prompts (refuse anything that would block on stdin)]' \
'--raw[Emit unstyled, compact output\: pipe-safe text, and \`json\` on one line]' \
'--examples[Print curated examples block and exit]' \
'--refresh[Send every read instead of answering it from a stored response; still store the reply]' \
'--no-cache[Read and write no stored responses, notes, or ledger records]' \
'--cache-only[Answer only from stored responses; never make API requests]' \
'-h[Print help (see more with '\''--help'\'')]' \
'--help[Print help (see more with '\''--help'\'')]' \
&& ret=0
;;
(help)
_arguments "${_arguments_options[@]}" : \
":: :_bird__subcmd__watchlist__subcmd__help_commands" \
"*::: :->help" \
&& ret=0

    case $state in
    (help)
        words=($line[1] "${words[@]}")
        (( CURRENT += 1 ))
        curcontext="${curcontext%:*:*}:bird-watchlist-help-command-$line[1]:"
        case $line[1] in
            (fetch)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
(add)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
(remove)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
(list)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
(help)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
        esac
    ;;
esac
;;
        esac
    ;;
esac
;;
(spend)
_arguments "${_arguments_options[@]}" : \
'--since=[Show spend since this date (YYYY-MM-DD; default\: 30 days ago)]:SINCE:_default' \
'-u+[Signed-in account to use when the app holds tokens for several (as \`xr -u\`)]:USERNAME:_default' \
'--username=[Signed-in account to use when the app holds tokens for several (as \`xr -u\`)]:USERNAME:_default' \
'-o+[Output format (text, json, jsonl, ndjson). Defaults to json when piped]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON envelope, no color"
jsonl\:"Streaming line-delimited JSON (one object per line; no wrapper)"
ndjson\:"Newline-delimited JSON, accepted as an alias for jsonl"))' \
'--output=[Output format (text, json, jsonl, ndjson). Defaults to json when piped]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON envelope, no color"
jsonl\:"Streaming line-delimited JSON (one object per line; no wrapper)"
ndjson\:"Newline-delimited JSON, accepted as an alias for jsonl"))' \
'--color=[Color mode\: auto (default), always, never]:COLOR:((auto\:"Auto-detect\: color when stderr is a TTY and \`NO_COLOR\` is unset"
always\:"Always emit colors"
never\:"Never emit colors"))' \
'--timeout=[Network timeout in seconds (default 30) for each X API request]:TIMEOUT:_default' \
'--limit=[Maximum number of results to return on list-style commands (default 100, ceiling 1000)]:N:_default' \
'--cursor=[Pagination cursor token for list-style commands (X API \`pagination_token\`/\`next_token\`)]:TOKEN:_default' \
'--app=[App in the token store to use. \`--app\` wins over \`BIRD_APP\`, which wins over the store'\''s default app. bird does not read \`XURL_APP\`; when it is set and \`BIRD_APP\` is not, commands that call the X API print a warning]:NAME:_default' \
'--auth=[Per-call auth scheme override\: \`app\`, \`oauth1\`, \`oauth2\`, or \`none\`. \`none\` is refused for a command whose X API endpoint requires authentication]:SCHEME:_default' \
'--sync[Also fetch X'\''s actual usage (GET /2/usage/tweets) to set beside the estimates]' \
'--pretty[Pretty-print human-readable output]' \
'(-o --output --jsonl)--json[Shorthand for \`--output json\`]' \
'(-o --output)--jsonl[Shorthand for \`--output jsonl\`]' \
'-q[Suppress informational stderr output (keep only fatal errors)]' \
'--quiet[Suppress informational stderr output (keep only fatal errors)]' \
'*-v[Increase verbosity (repeatable\: -v info, -vv debug, -vvv trace)]' \
'*--verbose[Increase verbosity (repeatable\: -v info, -vv debug, -vvv trace)]' \
'--no-interactive[Disable interactive prompts (refuse anything that would block on stdin)]' \
'--raw[Emit unstyled, compact output\: pipe-safe text, and \`json\` on one line]' \
'--examples[Print curated examples block and exit]' \
'--refresh[Send every read instead of answering it from a stored response; still store the reply]' \
'--no-cache[Read and write no stored responses, notes, or ledger records]' \
'--cache-only[Answer only from stored responses; never make API requests]' \
'-h[Print help (see more with '\''--help'\'')]' \
'--help[Print help (see more with '\''--help'\'')]' \
&& ret=0
;;
(tweet)
_arguments "${_arguments_options[@]}" : \
'--media-id=[Media ID to attach]:MEDIA_ID:_default' \
'-u+[Signed-in account to use when the app holds tokens for several (as \`xr -u\`)]:USERNAME:_default' \
'--username=[Signed-in account to use when the app holds tokens for several (as \`xr -u\`)]:USERNAME:_default' \
'-o+[Output format (text, json, jsonl, ndjson). Defaults to json when piped]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON envelope, no color"
jsonl\:"Streaming line-delimited JSON (one object per line; no wrapper)"
ndjson\:"Newline-delimited JSON, accepted as an alias for jsonl"))' \
'--output=[Output format (text, json, jsonl, ndjson). Defaults to json when piped]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON envelope, no color"
jsonl\:"Streaming line-delimited JSON (one object per line; no wrapper)"
ndjson\:"Newline-delimited JSON, accepted as an alias for jsonl"))' \
'--color=[Color mode\: auto (default), always, never]:COLOR:((auto\:"Auto-detect\: color when stderr is a TTY and \`NO_COLOR\` is unset"
always\:"Always emit colors"
never\:"Never emit colors"))' \
'--timeout=[Network timeout in seconds (default 30) for each X API request]:TIMEOUT:_default' \
'--limit=[Maximum number of results to return on list-style commands (default 100, ceiling 1000)]:N:_default' \
'--cursor=[Pagination cursor token for list-style commands (X API \`pagination_token\`/\`next_token\`)]:TOKEN:_default' \
'--app=[App in the token store to use. \`--app\` wins over \`BIRD_APP\`, which wins over the store'\''s default app. bird does not read \`XURL_APP\`; when it is set and \`BIRD_APP\` is not, commands that call the X API print a warning]:NAME:_default' \
'--auth=[Per-call auth scheme override\: \`app\`, \`oauth1\`, \`oauth2\`, or \`none\`. \`none\` is refused for a command whose X API endpoint requires authentication]:SCHEME:_default' \
'-f[Skip the interactive confirmation prompt (alias\: --yes)]' \
'--force[Skip the interactive confirmation prompt (alias\: --yes)]' \
'--dry-run[Validate inputs and print the would-be request, then exit without calling the API]' \
'(-o --output --jsonl)--json[Shorthand for \`--output json\`]' \
'(-o --output)--jsonl[Shorthand for \`--output jsonl\`]' \
'-q[Suppress informational stderr output (keep only fatal errors)]' \
'--quiet[Suppress informational stderr output (keep only fatal errors)]' \
'*-v[Increase verbosity (repeatable\: -v info, -vv debug, -vvv trace)]' \
'*--verbose[Increase verbosity (repeatable\: -v info, -vv debug, -vvv trace)]' \
'--no-interactive[Disable interactive prompts (refuse anything that would block on stdin)]' \
'--raw[Emit unstyled, compact output\: pipe-safe text, and \`json\` on one line]' \
'--examples[Print curated examples block and exit]' \
'--refresh[Send every read instead of answering it from a stored response; still store the reply]' \
'--no-cache[Read and write no stored responses, notes, or ledger records]' \
'--cache-only[Answer only from stored responses; never make API requests]' \
'-h[Print help (see more with '\''--help'\'')]' \
'--help[Print help (see more with '\''--help'\'')]' \
':text -- Tweet text:_default' \
&& ret=0
;;
(reply)
_arguments "${_arguments_options[@]}" : \
'-u+[Signed-in account to use when the app holds tokens for several (as \`xr -u\`)]:USERNAME:_default' \
'--username=[Signed-in account to use when the app holds tokens for several (as \`xr -u\`)]:USERNAME:_default' \
'-o+[Output format (text, json, jsonl, ndjson). Defaults to json when piped]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON envelope, no color"
jsonl\:"Streaming line-delimited JSON (one object per line; no wrapper)"
ndjson\:"Newline-delimited JSON, accepted as an alias for jsonl"))' \
'--output=[Output format (text, json, jsonl, ndjson). Defaults to json when piped]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON envelope, no color"
jsonl\:"Streaming line-delimited JSON (one object per line; no wrapper)"
ndjson\:"Newline-delimited JSON, accepted as an alias for jsonl"))' \
'--color=[Color mode\: auto (default), always, never]:COLOR:((auto\:"Auto-detect\: color when stderr is a TTY and \`NO_COLOR\` is unset"
always\:"Always emit colors"
never\:"Never emit colors"))' \
'--timeout=[Network timeout in seconds (default 30) for each X API request]:TIMEOUT:_default' \
'--limit=[Maximum number of results to return on list-style commands (default 100, ceiling 1000)]:N:_default' \
'--cursor=[Pagination cursor token for list-style commands (X API \`pagination_token\`/\`next_token\`)]:TOKEN:_default' \
'--app=[App in the token store to use. \`--app\` wins over \`BIRD_APP\`, which wins over the store'\''s default app. bird does not read \`XURL_APP\`; when it is set and \`BIRD_APP\` is not, commands that call the X API print a warning]:NAME:_default' \
'--auth=[Per-call auth scheme override\: \`app\`, \`oauth1\`, \`oauth2\`, or \`none\`. \`none\` is refused for a command whose X API endpoint requires authentication]:SCHEME:_default' \
'-f[Skip the interactive confirmation prompt (alias\: --yes)]' \
'--force[Skip the interactive confirmation prompt (alias\: --yes)]' \
'--dry-run[Validate inputs and print the would-be request, then exit without calling the API]' \
'(-o --output --jsonl)--json[Shorthand for \`--output json\`]' \
'(-o --output)--jsonl[Shorthand for \`--output jsonl\`]' \
'-q[Suppress informational stderr output (keep only fatal errors)]' \
'--quiet[Suppress informational stderr output (keep only fatal errors)]' \
'*-v[Increase verbosity (repeatable\: -v info, -vv debug, -vvv trace)]' \
'*--verbose[Increase verbosity (repeatable\: -v info, -vv debug, -vvv trace)]' \
'--no-interactive[Disable interactive prompts (refuse anything that would block on stdin)]' \
'--raw[Emit unstyled, compact output\: pipe-safe text, and \`json\` on one line]' \
'--examples[Print curated examples block and exit]' \
'--refresh[Send every read instead of answering it from a stored response; still store the reply]' \
'--no-cache[Read and write no stored responses, notes, or ledger records]' \
'--cache-only[Answer only from stored responses; never make API requests]' \
'-h[Print help (see more with '\''--help'\'')]' \
'--help[Print help (see more with '\''--help'\'')]' \
':tweet_id -- Tweet ID to reply to:_default' \
':text -- Reply text:_default' \
&& ret=0
;;
(like)
_arguments "${_arguments_options[@]}" : \
'-u+[Signed-in account to use when the app holds tokens for several (as \`xr -u\`)]:USERNAME:_default' \
'--username=[Signed-in account to use when the app holds tokens for several (as \`xr -u\`)]:USERNAME:_default' \
'-o+[Output format (text, json, jsonl, ndjson). Defaults to json when piped]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON envelope, no color"
jsonl\:"Streaming line-delimited JSON (one object per line; no wrapper)"
ndjson\:"Newline-delimited JSON, accepted as an alias for jsonl"))' \
'--output=[Output format (text, json, jsonl, ndjson). Defaults to json when piped]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON envelope, no color"
jsonl\:"Streaming line-delimited JSON (one object per line; no wrapper)"
ndjson\:"Newline-delimited JSON, accepted as an alias for jsonl"))' \
'--color=[Color mode\: auto (default), always, never]:COLOR:((auto\:"Auto-detect\: color when stderr is a TTY and \`NO_COLOR\` is unset"
always\:"Always emit colors"
never\:"Never emit colors"))' \
'--timeout=[Network timeout in seconds (default 30) for each X API request]:TIMEOUT:_default' \
'--limit=[Maximum number of results to return on list-style commands (default 100, ceiling 1000)]:N:_default' \
'--cursor=[Pagination cursor token for list-style commands (X API \`pagination_token\`/\`next_token\`)]:TOKEN:_default' \
'--app=[App in the token store to use. \`--app\` wins over \`BIRD_APP\`, which wins over the store'\''s default app. bird does not read \`XURL_APP\`; when it is set and \`BIRD_APP\` is not, commands that call the X API print a warning]:NAME:_default' \
'--auth=[Per-call auth scheme override\: \`app\`, \`oauth1\`, \`oauth2\`, or \`none\`. \`none\` is refused for a command whose X API endpoint requires authentication]:SCHEME:_default' \
'-f[Skip the interactive confirmation prompt (alias\: --yes)]' \
'--force[Skip the interactive confirmation prompt (alias\: --yes)]' \
'--dry-run[Validate inputs and print the would-be request, then exit without calling the API]' \
'(-o --output --jsonl)--json[Shorthand for \`--output json\`]' \
'(-o --output)--jsonl[Shorthand for \`--output jsonl\`]' \
'-q[Suppress informational stderr output (keep only fatal errors)]' \
'--quiet[Suppress informational stderr output (keep only fatal errors)]' \
'*-v[Increase verbosity (repeatable\: -v info, -vv debug, -vvv trace)]' \
'*--verbose[Increase verbosity (repeatable\: -v info, -vv debug, -vvv trace)]' \
'--no-interactive[Disable interactive prompts (refuse anything that would block on stdin)]' \
'--raw[Emit unstyled, compact output\: pipe-safe text, and \`json\` on one line]' \
'--examples[Print curated examples block and exit]' \
'--refresh[Send every read instead of answering it from a stored response; still store the reply]' \
'--no-cache[Read and write no stored responses, notes, or ledger records]' \
'--cache-only[Answer only from stored responses; never make API requests]' \
'-h[Print help (see more with '\''--help'\'')]' \
'--help[Print help (see more with '\''--help'\'')]' \
':tweet_id -- Tweet ID to like:_default' \
&& ret=0
;;
(unlike)
_arguments "${_arguments_options[@]}" : \
'-u+[Signed-in account to use when the app holds tokens for several (as \`xr -u\`)]:USERNAME:_default' \
'--username=[Signed-in account to use when the app holds tokens for several (as \`xr -u\`)]:USERNAME:_default' \
'-o+[Output format (text, json, jsonl, ndjson). Defaults to json when piped]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON envelope, no color"
jsonl\:"Streaming line-delimited JSON (one object per line; no wrapper)"
ndjson\:"Newline-delimited JSON, accepted as an alias for jsonl"))' \
'--output=[Output format (text, json, jsonl, ndjson). Defaults to json when piped]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON envelope, no color"
jsonl\:"Streaming line-delimited JSON (one object per line; no wrapper)"
ndjson\:"Newline-delimited JSON, accepted as an alias for jsonl"))' \
'--color=[Color mode\: auto (default), always, never]:COLOR:((auto\:"Auto-detect\: color when stderr is a TTY and \`NO_COLOR\` is unset"
always\:"Always emit colors"
never\:"Never emit colors"))' \
'--timeout=[Network timeout in seconds (default 30) for each X API request]:TIMEOUT:_default' \
'--limit=[Maximum number of results to return on list-style commands (default 100, ceiling 1000)]:N:_default' \
'--cursor=[Pagination cursor token for list-style commands (X API \`pagination_token\`/\`next_token\`)]:TOKEN:_default' \
'--app=[App in the token store to use. \`--app\` wins over \`BIRD_APP\`, which wins over the store'\''s default app. bird does not read \`XURL_APP\`; when it is set and \`BIRD_APP\` is not, commands that call the X API print a warning]:NAME:_default' \
'--auth=[Per-call auth scheme override\: \`app\`, \`oauth1\`, \`oauth2\`, or \`none\`. \`none\` is refused for a command whose X API endpoint requires authentication]:SCHEME:_default' \
'-f[Skip the interactive confirmation prompt (alias\: --yes)]' \
'--force[Skip the interactive confirmation prompt (alias\: --yes)]' \
'--dry-run[Validate inputs and print the would-be request, then exit without calling the API]' \
'(-o --output --jsonl)--json[Shorthand for \`--output json\`]' \
'(-o --output)--jsonl[Shorthand for \`--output jsonl\`]' \
'-q[Suppress informational stderr output (keep only fatal errors)]' \
'--quiet[Suppress informational stderr output (keep only fatal errors)]' \
'*-v[Increase verbosity (repeatable\: -v info, -vv debug, -vvv trace)]' \
'*--verbose[Increase verbosity (repeatable\: -v info, -vv debug, -vvv trace)]' \
'--no-interactive[Disable interactive prompts (refuse anything that would block on stdin)]' \
'--raw[Emit unstyled, compact output\: pipe-safe text, and \`json\` on one line]' \
'--examples[Print curated examples block and exit]' \
'--refresh[Send every read instead of answering it from a stored response; still store the reply]' \
'--no-cache[Read and write no stored responses, notes, or ledger records]' \
'--cache-only[Answer only from stored responses; never make API requests]' \
'-h[Print help (see more with '\''--help'\'')]' \
'--help[Print help (see more with '\''--help'\'')]' \
':tweet_id -- Tweet ID to unlike:_default' \
&& ret=0
;;
(repost)
_arguments "${_arguments_options[@]}" : \
'-u+[Signed-in account to use when the app holds tokens for several (as \`xr -u\`)]:USERNAME:_default' \
'--username=[Signed-in account to use when the app holds tokens for several (as \`xr -u\`)]:USERNAME:_default' \
'-o+[Output format (text, json, jsonl, ndjson). Defaults to json when piped]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON envelope, no color"
jsonl\:"Streaming line-delimited JSON (one object per line; no wrapper)"
ndjson\:"Newline-delimited JSON, accepted as an alias for jsonl"))' \
'--output=[Output format (text, json, jsonl, ndjson). Defaults to json when piped]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON envelope, no color"
jsonl\:"Streaming line-delimited JSON (one object per line; no wrapper)"
ndjson\:"Newline-delimited JSON, accepted as an alias for jsonl"))' \
'--color=[Color mode\: auto (default), always, never]:COLOR:((auto\:"Auto-detect\: color when stderr is a TTY and \`NO_COLOR\` is unset"
always\:"Always emit colors"
never\:"Never emit colors"))' \
'--timeout=[Network timeout in seconds (default 30) for each X API request]:TIMEOUT:_default' \
'--limit=[Maximum number of results to return on list-style commands (default 100, ceiling 1000)]:N:_default' \
'--cursor=[Pagination cursor token for list-style commands (X API \`pagination_token\`/\`next_token\`)]:TOKEN:_default' \
'--app=[App in the token store to use. \`--app\` wins over \`BIRD_APP\`, which wins over the store'\''s default app. bird does not read \`XURL_APP\`; when it is set and \`BIRD_APP\` is not, commands that call the X API print a warning]:NAME:_default' \
'--auth=[Per-call auth scheme override\: \`app\`, \`oauth1\`, \`oauth2\`, or \`none\`. \`none\` is refused for a command whose X API endpoint requires authentication]:SCHEME:_default' \
'-f[Skip the interactive confirmation prompt (alias\: --yes)]' \
'--force[Skip the interactive confirmation prompt (alias\: --yes)]' \
'--dry-run[Validate inputs and print the would-be request, then exit without calling the API]' \
'(-o --output --jsonl)--json[Shorthand for \`--output json\`]' \
'(-o --output)--jsonl[Shorthand for \`--output jsonl\`]' \
'-q[Suppress informational stderr output (keep only fatal errors)]' \
'--quiet[Suppress informational stderr output (keep only fatal errors)]' \
'*-v[Increase verbosity (repeatable\: -v info, -vv debug, -vvv trace)]' \
'*--verbose[Increase verbosity (repeatable\: -v info, -vv debug, -vvv trace)]' \
'--no-interactive[Disable interactive prompts (refuse anything that would block on stdin)]' \
'--raw[Emit unstyled, compact output\: pipe-safe text, and \`json\` on one line]' \
'--examples[Print curated examples block and exit]' \
'--refresh[Send every read instead of answering it from a stored response; still store the reply]' \
'--no-cache[Read and write no stored responses, notes, or ledger records]' \
'--cache-only[Answer only from stored responses; never make API requests]' \
'-h[Print help (see more with '\''--help'\'')]' \
'--help[Print help (see more with '\''--help'\'')]' \
':tweet_id -- Tweet ID to repost:_default' \
&& ret=0
;;
(unrepost)
_arguments "${_arguments_options[@]}" : \
'-u+[Signed-in account to use when the app holds tokens for several (as \`xr -u\`)]:USERNAME:_default' \
'--username=[Signed-in account to use when the app holds tokens for several (as \`xr -u\`)]:USERNAME:_default' \
'-o+[Output format (text, json, jsonl, ndjson). Defaults to json when piped]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON envelope, no color"
jsonl\:"Streaming line-delimited JSON (one object per line; no wrapper)"
ndjson\:"Newline-delimited JSON, accepted as an alias for jsonl"))' \
'--output=[Output format (text, json, jsonl, ndjson). Defaults to json when piped]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON envelope, no color"
jsonl\:"Streaming line-delimited JSON (one object per line; no wrapper)"
ndjson\:"Newline-delimited JSON, accepted as an alias for jsonl"))' \
'--color=[Color mode\: auto (default), always, never]:COLOR:((auto\:"Auto-detect\: color when stderr is a TTY and \`NO_COLOR\` is unset"
always\:"Always emit colors"
never\:"Never emit colors"))' \
'--timeout=[Network timeout in seconds (default 30) for each X API request]:TIMEOUT:_default' \
'--limit=[Maximum number of results to return on list-style commands (default 100, ceiling 1000)]:N:_default' \
'--cursor=[Pagination cursor token for list-style commands (X API \`pagination_token\`/\`next_token\`)]:TOKEN:_default' \
'--app=[App in the token store to use. \`--app\` wins over \`BIRD_APP\`, which wins over the store'\''s default app. bird does not read \`XURL_APP\`; when it is set and \`BIRD_APP\` is not, commands that call the X API print a warning]:NAME:_default' \
'--auth=[Per-call auth scheme override\: \`app\`, \`oauth1\`, \`oauth2\`, or \`none\`. \`none\` is refused for a command whose X API endpoint requires authentication]:SCHEME:_default' \
'-f[Skip the interactive confirmation prompt (alias\: --yes)]' \
'--force[Skip the interactive confirmation prompt (alias\: --yes)]' \
'--dry-run[Validate inputs and print the would-be request, then exit without calling the API]' \
'(-o --output --jsonl)--json[Shorthand for \`--output json\`]' \
'(-o --output)--jsonl[Shorthand for \`--output jsonl\`]' \
'-q[Suppress informational stderr output (keep only fatal errors)]' \
'--quiet[Suppress informational stderr output (keep only fatal errors)]' \
'*-v[Increase verbosity (repeatable\: -v info, -vv debug, -vvv trace)]' \
'*--verbose[Increase verbosity (repeatable\: -v info, -vv debug, -vvv trace)]' \
'--no-interactive[Disable interactive prompts (refuse anything that would block on stdin)]' \
'--raw[Emit unstyled, compact output\: pipe-safe text, and \`json\` on one line]' \
'--examples[Print curated examples block and exit]' \
'--refresh[Send every read instead of answering it from a stored response; still store the reply]' \
'--no-cache[Read and write no stored responses, notes, or ledger records]' \
'--cache-only[Answer only from stored responses; never make API requests]' \
'-h[Print help (see more with '\''--help'\'')]' \
'--help[Print help (see more with '\''--help'\'')]' \
':tweet_id -- Tweet ID to unrepost:_default' \
&& ret=0
;;
(follow)
_arguments "${_arguments_options[@]}" : \
'-o+[Output format (text, json, jsonl, ndjson). Defaults to json when piped]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON envelope, no color"
jsonl\:"Streaming line-delimited JSON (one object per line; no wrapper)"
ndjson\:"Newline-delimited JSON, accepted as an alias for jsonl"))' \
'--output=[Output format (text, json, jsonl, ndjson). Defaults to json when piped]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON envelope, no color"
jsonl\:"Streaming line-delimited JSON (one object per line; no wrapper)"
ndjson\:"Newline-delimited JSON, accepted as an alias for jsonl"))' \
'--color=[Color mode\: auto (default), always, never]:COLOR:((auto\:"Auto-detect\: color when stderr is a TTY and \`NO_COLOR\` is unset"
always\:"Always emit colors"
never\:"Never emit colors"))' \
'--timeout=[Network timeout in seconds (default 30) for each X API request]:TIMEOUT:_default' \
'--limit=[Maximum number of results to return on list-style commands (default 100, ceiling 1000)]:N:_default' \
'--cursor=[Pagination cursor token for list-style commands (X API \`pagination_token\`/\`next_token\`)]:TOKEN:_default' \
'--app=[App in the token store to use. \`--app\` wins over \`BIRD_APP\`, which wins over the store'\''s default app. bird does not read \`XURL_APP\`; when it is set and \`BIRD_APP\` is not, commands that call the X API print a warning]:NAME:_default' \
'--auth=[Per-call auth scheme override\: \`app\`, \`oauth1\`, \`oauth2\`, or \`none\`. \`none\` is refused for a command whose X API endpoint requires authentication]:SCHEME:_default' \
'-f[Skip the interactive confirmation prompt (alias\: --yes)]' \
'--force[Skip the interactive confirmation prompt (alias\: --yes)]' \
'--dry-run[Validate inputs and print the would-be request, then exit without calling the API]' \
'(-o --output --jsonl)--json[Shorthand for \`--output json\`]' \
'(-o --output)--jsonl[Shorthand for \`--output jsonl\`]' \
'-q[Suppress informational stderr output (keep only fatal errors)]' \
'--quiet[Suppress informational stderr output (keep only fatal errors)]' \
'*-v[Increase verbosity (repeatable\: -v info, -vv debug, -vvv trace)]' \
'*--verbose[Increase verbosity (repeatable\: -v info, -vv debug, -vvv trace)]' \
'--no-interactive[Disable interactive prompts (refuse anything that would block on stdin)]' \
'--raw[Emit unstyled, compact output\: pipe-safe text, and \`json\` on one line]' \
'--examples[Print curated examples block and exit]' \
'--refresh[Send every read instead of answering it from a stored response; still store the reply]' \
'--no-cache[Read and write no stored responses, notes, or ledger records]' \
'--cache-only[Answer only from stored responses; never make API requests]' \
'-h[Print help (see more with '\''--help'\'')]' \
'--help[Print help (see more with '\''--help'\'')]' \
':username -- Username to follow:_default' \
&& ret=0
;;
(unfollow)
_arguments "${_arguments_options[@]}" : \
'-o+[Output format (text, json, jsonl, ndjson). Defaults to json when piped]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON envelope, no color"
jsonl\:"Streaming line-delimited JSON (one object per line; no wrapper)"
ndjson\:"Newline-delimited JSON, accepted as an alias for jsonl"))' \
'--output=[Output format (text, json, jsonl, ndjson). Defaults to json when piped]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON envelope, no color"
jsonl\:"Streaming line-delimited JSON (one object per line; no wrapper)"
ndjson\:"Newline-delimited JSON, accepted as an alias for jsonl"))' \
'--color=[Color mode\: auto (default), always, never]:COLOR:((auto\:"Auto-detect\: color when stderr is a TTY and \`NO_COLOR\` is unset"
always\:"Always emit colors"
never\:"Never emit colors"))' \
'--timeout=[Network timeout in seconds (default 30) for each X API request]:TIMEOUT:_default' \
'--limit=[Maximum number of results to return on list-style commands (default 100, ceiling 1000)]:N:_default' \
'--cursor=[Pagination cursor token for list-style commands (X API \`pagination_token\`/\`next_token\`)]:TOKEN:_default' \
'--app=[App in the token store to use. \`--app\` wins over \`BIRD_APP\`, which wins over the store'\''s default app. bird does not read \`XURL_APP\`; when it is set and \`BIRD_APP\` is not, commands that call the X API print a warning]:NAME:_default' \
'--auth=[Per-call auth scheme override\: \`app\`, \`oauth1\`, \`oauth2\`, or \`none\`. \`none\` is refused for a command whose X API endpoint requires authentication]:SCHEME:_default' \
'-f[Skip the interactive confirmation prompt (alias\: --yes)]' \
'--force[Skip the interactive confirmation prompt (alias\: --yes)]' \
'--dry-run[Validate inputs and print the would-be request, then exit without calling the API]' \
'(-o --output --jsonl)--json[Shorthand for \`--output json\`]' \
'(-o --output)--jsonl[Shorthand for \`--output jsonl\`]' \
'-q[Suppress informational stderr output (keep only fatal errors)]' \
'--quiet[Suppress informational stderr output (keep only fatal errors)]' \
'*-v[Increase verbosity (repeatable\: -v info, -vv debug, -vvv trace)]' \
'*--verbose[Increase verbosity (repeatable\: -v info, -vv debug, -vvv trace)]' \
'--no-interactive[Disable interactive prompts (refuse anything that would block on stdin)]' \
'--raw[Emit unstyled, compact output\: pipe-safe text, and \`json\` on one line]' \
'--examples[Print curated examples block and exit]' \
'--refresh[Send every read instead of answering it from a stored response; still store the reply]' \
'--no-cache[Read and write no stored responses, notes, or ledger records]' \
'--cache-only[Answer only from stored responses; never make API requests]' \
'-h[Print help (see more with '\''--help'\'')]' \
'--help[Print help (see more with '\''--help'\'')]' \
':username -- Username to unfollow:_default' \
&& ret=0
;;
(dm)
_arguments "${_arguments_options[@]}" : \
'-o+[Output format (text, json, jsonl, ndjson). Defaults to json when piped]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON envelope, no color"
jsonl\:"Streaming line-delimited JSON (one object per line; no wrapper)"
ndjson\:"Newline-delimited JSON, accepted as an alias for jsonl"))' \
'--output=[Output format (text, json, jsonl, ndjson). Defaults to json when piped]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON envelope, no color"
jsonl\:"Streaming line-delimited JSON (one object per line; no wrapper)"
ndjson\:"Newline-delimited JSON, accepted as an alias for jsonl"))' \
'--color=[Color mode\: auto (default), always, never]:COLOR:((auto\:"Auto-detect\: color when stderr is a TTY and \`NO_COLOR\` is unset"
always\:"Always emit colors"
never\:"Never emit colors"))' \
'--timeout=[Network timeout in seconds (default 30) for each X API request]:TIMEOUT:_default' \
'--limit=[Maximum number of results to return on list-style commands (default 100, ceiling 1000)]:N:_default' \
'--cursor=[Pagination cursor token for list-style commands (X API \`pagination_token\`/\`next_token\`)]:TOKEN:_default' \
'--app=[App in the token store to use. \`--app\` wins over \`BIRD_APP\`, which wins over the store'\''s default app. bird does not read \`XURL_APP\`; when it is set and \`BIRD_APP\` is not, commands that call the X API print a warning]:NAME:_default' \
'--auth=[Per-call auth scheme override\: \`app\`, \`oauth1\`, \`oauth2\`, or \`none\`. \`none\` is refused for a command whose X API endpoint requires authentication]:SCHEME:_default' \
'-f[Skip the interactive confirmation prompt (alias\: --yes)]' \
'--force[Skip the interactive confirmation prompt (alias\: --yes)]' \
'--dry-run[Validate inputs and print the would-be request, then exit without calling the API]' \
'(-o --output --jsonl)--json[Shorthand for \`--output json\`]' \
'(-o --output)--jsonl[Shorthand for \`--output jsonl\`]' \
'-q[Suppress informational stderr output (keep only fatal errors)]' \
'--quiet[Suppress informational stderr output (keep only fatal errors)]' \
'*-v[Increase verbosity (repeatable\: -v info, -vv debug, -vvv trace)]' \
'*--verbose[Increase verbosity (repeatable\: -v info, -vv debug, -vvv trace)]' \
'--no-interactive[Disable interactive prompts (refuse anything that would block on stdin)]' \
'--raw[Emit unstyled, compact output\: pipe-safe text, and \`json\` on one line]' \
'--examples[Print curated examples block and exit]' \
'--refresh[Send every read instead of answering it from a stored response; still store the reply]' \
'--no-cache[Read and write no stored responses, notes, or ledger records]' \
'--cache-only[Answer only from stored responses; never make API requests]' \
'-h[Print help (see more with '\''--help'\'')]' \
'--help[Print help (see more with '\''--help'\'')]' \
':username -- Username to message:_default' \
':text -- Message text:_default' \
&& ret=0
;;
(block)
_arguments "${_arguments_options[@]}" : \
'-o+[Output format (text, json, jsonl, ndjson). Defaults to json when piped]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON envelope, no color"
jsonl\:"Streaming line-delimited JSON (one object per line; no wrapper)"
ndjson\:"Newline-delimited JSON, accepted as an alias for jsonl"))' \
'--output=[Output format (text, json, jsonl, ndjson). Defaults to json when piped]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON envelope, no color"
jsonl\:"Streaming line-delimited JSON (one object per line; no wrapper)"
ndjson\:"Newline-delimited JSON, accepted as an alias for jsonl"))' \
'--color=[Color mode\: auto (default), always, never]:COLOR:((auto\:"Auto-detect\: color when stderr is a TTY and \`NO_COLOR\` is unset"
always\:"Always emit colors"
never\:"Never emit colors"))' \
'--timeout=[Network timeout in seconds (default 30) for each X API request]:TIMEOUT:_default' \
'--limit=[Maximum number of results to return on list-style commands (default 100, ceiling 1000)]:N:_default' \
'--cursor=[Pagination cursor token for list-style commands (X API \`pagination_token\`/\`next_token\`)]:TOKEN:_default' \
'--app=[App in the token store to use. \`--app\` wins over \`BIRD_APP\`, which wins over the store'\''s default app. bird does not read \`XURL_APP\`; when it is set and \`BIRD_APP\` is not, commands that call the X API print a warning]:NAME:_default' \
'--auth=[Per-call auth scheme override\: \`app\`, \`oauth1\`, \`oauth2\`, or \`none\`. \`none\` is refused for a command whose X API endpoint requires authentication]:SCHEME:_default' \
'-f[Skip the interactive confirmation prompt (alias\: --yes)]' \
'--force[Skip the interactive confirmation prompt (alias\: --yes)]' \
'--dry-run[Validate inputs and print the would-be request, then exit without calling the API]' \
'(-o --output --jsonl)--json[Shorthand for \`--output json\`]' \
'(-o --output)--jsonl[Shorthand for \`--output jsonl\`]' \
'-q[Suppress informational stderr output (keep only fatal errors)]' \
'--quiet[Suppress informational stderr output (keep only fatal errors)]' \
'*-v[Increase verbosity (repeatable\: -v info, -vv debug, -vvv trace)]' \
'*--verbose[Increase verbosity (repeatable\: -v info, -vv debug, -vvv trace)]' \
'--no-interactive[Disable interactive prompts (refuse anything that would block on stdin)]' \
'--raw[Emit unstyled, compact output\: pipe-safe text, and \`json\` on one line]' \
'--examples[Print curated examples block and exit]' \
'--refresh[Send every read instead of answering it from a stored response; still store the reply]' \
'--no-cache[Read and write no stored responses, notes, or ledger records]' \
'--cache-only[Answer only from stored responses; never make API requests]' \
'-h[Print help (see more with '\''--help'\'')]' \
'--help[Print help (see more with '\''--help'\'')]' \
':username -- Username to block:_default' \
&& ret=0
;;
(unblock)
_arguments "${_arguments_options[@]}" : \
'-o+[Output format (text, json, jsonl, ndjson). Defaults to json when piped]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON envelope, no color"
jsonl\:"Streaming line-delimited JSON (one object per line; no wrapper)"
ndjson\:"Newline-delimited JSON, accepted as an alias for jsonl"))' \
'--output=[Output format (text, json, jsonl, ndjson). Defaults to json when piped]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON envelope, no color"
jsonl\:"Streaming line-delimited JSON (one object per line; no wrapper)"
ndjson\:"Newline-delimited JSON, accepted as an alias for jsonl"))' \
'--color=[Color mode\: auto (default), always, never]:COLOR:((auto\:"Auto-detect\: color when stderr is a TTY and \`NO_COLOR\` is unset"
always\:"Always emit colors"
never\:"Never emit colors"))' \
'--timeout=[Network timeout in seconds (default 30) for each X API request]:TIMEOUT:_default' \
'--limit=[Maximum number of results to return on list-style commands (default 100, ceiling 1000)]:N:_default' \
'--cursor=[Pagination cursor token for list-style commands (X API \`pagination_token\`/\`next_token\`)]:TOKEN:_default' \
'--app=[App in the token store to use. \`--app\` wins over \`BIRD_APP\`, which wins over the store'\''s default app. bird does not read \`XURL_APP\`; when it is set and \`BIRD_APP\` is not, commands that call the X API print a warning]:NAME:_default' \
'--auth=[Per-call auth scheme override\: \`app\`, \`oauth1\`, \`oauth2\`, or \`none\`. \`none\` is refused for a command whose X API endpoint requires authentication]:SCHEME:_default' \
'-f[Skip the interactive confirmation prompt (alias\: --yes)]' \
'--force[Skip the interactive confirmation prompt (alias\: --yes)]' \
'--dry-run[Validate inputs and print the would-be request, then exit without calling the API]' \
'(-o --output --jsonl)--json[Shorthand for \`--output json\`]' \
'(-o --output)--jsonl[Shorthand for \`--output jsonl\`]' \
'-q[Suppress informational stderr output (keep only fatal errors)]' \
'--quiet[Suppress informational stderr output (keep only fatal errors)]' \
'*-v[Increase verbosity (repeatable\: -v info, -vv debug, -vvv trace)]' \
'*--verbose[Increase verbosity (repeatable\: -v info, -vv debug, -vvv trace)]' \
'--no-interactive[Disable interactive prompts (refuse anything that would block on stdin)]' \
'--raw[Emit unstyled, compact output\: pipe-safe text, and \`json\` on one line]' \
'--examples[Print curated examples block and exit]' \
'--refresh[Send every read instead of answering it from a stored response; still store the reply]' \
'--no-cache[Read and write no stored responses, notes, or ledger records]' \
'--cache-only[Answer only from stored responses; never make API requests]' \
'-h[Print help (see more with '\''--help'\'')]' \
'--help[Print help (see more with '\''--help'\'')]' \
':username -- Username to unblock:_default' \
&& ret=0
;;
(mute)
_arguments "${_arguments_options[@]}" : \
'-o+[Output format (text, json, jsonl, ndjson). Defaults to json when piped]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON envelope, no color"
jsonl\:"Streaming line-delimited JSON (one object per line; no wrapper)"
ndjson\:"Newline-delimited JSON, accepted as an alias for jsonl"))' \
'--output=[Output format (text, json, jsonl, ndjson). Defaults to json when piped]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON envelope, no color"
jsonl\:"Streaming line-delimited JSON (one object per line; no wrapper)"
ndjson\:"Newline-delimited JSON, accepted as an alias for jsonl"))' \
'--color=[Color mode\: auto (default), always, never]:COLOR:((auto\:"Auto-detect\: color when stderr is a TTY and \`NO_COLOR\` is unset"
always\:"Always emit colors"
never\:"Never emit colors"))' \
'--timeout=[Network timeout in seconds (default 30) for each X API request]:TIMEOUT:_default' \
'--limit=[Maximum number of results to return on list-style commands (default 100, ceiling 1000)]:N:_default' \
'--cursor=[Pagination cursor token for list-style commands (X API \`pagination_token\`/\`next_token\`)]:TOKEN:_default' \
'--app=[App in the token store to use. \`--app\` wins over \`BIRD_APP\`, which wins over the store'\''s default app. bird does not read \`XURL_APP\`; when it is set and \`BIRD_APP\` is not, commands that call the X API print a warning]:NAME:_default' \
'--auth=[Per-call auth scheme override\: \`app\`, \`oauth1\`, \`oauth2\`, or \`none\`. \`none\` is refused for a command whose X API endpoint requires authentication]:SCHEME:_default' \
'-f[Skip the interactive confirmation prompt (alias\: --yes)]' \
'--force[Skip the interactive confirmation prompt (alias\: --yes)]' \
'--dry-run[Validate inputs and print the would-be request, then exit without calling the API]' \
'(-o --output --jsonl)--json[Shorthand for \`--output json\`]' \
'(-o --output)--jsonl[Shorthand for \`--output jsonl\`]' \
'-q[Suppress informational stderr output (keep only fatal errors)]' \
'--quiet[Suppress informational stderr output (keep only fatal errors)]' \
'*-v[Increase verbosity (repeatable\: -v info, -vv debug, -vvv trace)]' \
'*--verbose[Increase verbosity (repeatable\: -v info, -vv debug, -vvv trace)]' \
'--no-interactive[Disable interactive prompts (refuse anything that would block on stdin)]' \
'--raw[Emit unstyled, compact output\: pipe-safe text, and \`json\` on one line]' \
'--examples[Print curated examples block and exit]' \
'--refresh[Send every read instead of answering it from a stored response; still store the reply]' \
'--no-cache[Read and write no stored responses, notes, or ledger records]' \
'--cache-only[Answer only from stored responses; never make API requests]' \
'-h[Print help (see more with '\''--help'\'')]' \
'--help[Print help (see more with '\''--help'\'')]' \
':username -- Username to mute:_default' \
&& ret=0
;;
(unmute)
_arguments "${_arguments_options[@]}" : \
'-o+[Output format (text, json, jsonl, ndjson). Defaults to json when piped]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON envelope, no color"
jsonl\:"Streaming line-delimited JSON (one object per line; no wrapper)"
ndjson\:"Newline-delimited JSON, accepted as an alias for jsonl"))' \
'--output=[Output format (text, json, jsonl, ndjson). Defaults to json when piped]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON envelope, no color"
jsonl\:"Streaming line-delimited JSON (one object per line; no wrapper)"
ndjson\:"Newline-delimited JSON, accepted as an alias for jsonl"))' \
'--color=[Color mode\: auto (default), always, never]:COLOR:((auto\:"Auto-detect\: color when stderr is a TTY and \`NO_COLOR\` is unset"
always\:"Always emit colors"
never\:"Never emit colors"))' \
'--timeout=[Network timeout in seconds (default 30) for each X API request]:TIMEOUT:_default' \
'--limit=[Maximum number of results to return on list-style commands (default 100, ceiling 1000)]:N:_default' \
'--cursor=[Pagination cursor token for list-style commands (X API \`pagination_token\`/\`next_token\`)]:TOKEN:_default' \
'--app=[App in the token store to use. \`--app\` wins over \`BIRD_APP\`, which wins over the store'\''s default app. bird does not read \`XURL_APP\`; when it is set and \`BIRD_APP\` is not, commands that call the X API print a warning]:NAME:_default' \
'--auth=[Per-call auth scheme override\: \`app\`, \`oauth1\`, \`oauth2\`, or \`none\`. \`none\` is refused for a command whose X API endpoint requires authentication]:SCHEME:_default' \
'-f[Skip the interactive confirmation prompt (alias\: --yes)]' \
'--force[Skip the interactive confirmation prompt (alias\: --yes)]' \
'--dry-run[Validate inputs and print the would-be request, then exit without calling the API]' \
'(-o --output --jsonl)--json[Shorthand for \`--output json\`]' \
'(-o --output)--jsonl[Shorthand for \`--output jsonl\`]' \
'-q[Suppress informational stderr output (keep only fatal errors)]' \
'--quiet[Suppress informational stderr output (keep only fatal errors)]' \
'*-v[Increase verbosity (repeatable\: -v info, -vv debug, -vvv trace)]' \
'*--verbose[Increase verbosity (repeatable\: -v info, -vv debug, -vvv trace)]' \
'--no-interactive[Disable interactive prompts (refuse anything that would block on stdin)]' \
'--raw[Emit unstyled, compact output\: pipe-safe text, and \`json\` on one line]' \
'--examples[Print curated examples block and exit]' \
'--refresh[Send every read instead of answering it from a stored response; still store the reply]' \
'--no-cache[Read and write no stored responses, notes, or ledger records]' \
'--cache-only[Answer only from stored responses; never make API requests]' \
'-h[Print help (see more with '\''--help'\'')]' \
'--help[Print help (see more with '\''--help'\'')]' \
':username -- Username to unmute:_default' \
&& ret=0
;;
(doctor)
_arguments "${_arguments_options[@]}" : \
'-u+[Signed-in account to use when the app holds tokens for several (as \`xr -u\`)]:USERNAME:_default' \
'--username=[Signed-in account to use when the app holds tokens for several (as \`xr -u\`)]:USERNAME:_default' \
'-o+[Output format (text, json, jsonl, ndjson). Defaults to json when piped]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON envelope, no color"
jsonl\:"Streaming line-delimited JSON (one object per line; no wrapper)"
ndjson\:"Newline-delimited JSON, accepted as an alias for jsonl"))' \
'--output=[Output format (text, json, jsonl, ndjson). Defaults to json when piped]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON envelope, no color"
jsonl\:"Streaming line-delimited JSON (one object per line; no wrapper)"
ndjson\:"Newline-delimited JSON, accepted as an alias for jsonl"))' \
'--color=[Color mode\: auto (default), always, never]:COLOR:((auto\:"Auto-detect\: color when stderr is a TTY and \`NO_COLOR\` is unset"
always\:"Always emit colors"
never\:"Never emit colors"))' \
'--timeout=[Network timeout in seconds (default 30) for each X API request]:TIMEOUT:_default' \
'--limit=[Maximum number of results to return on list-style commands (default 100, ceiling 1000)]:N:_default' \
'--cursor=[Pagination cursor token for list-style commands (X API \`pagination_token\`/\`next_token\`)]:TOKEN:_default' \
'--app=[App in the token store to use. \`--app\` wins over \`BIRD_APP\`, which wins over the store'\''s default app. bird does not read \`XURL_APP\`; when it is set and \`BIRD_APP\` is not, commands that call the X API print a warning]:NAME:_default' \
'--auth=[Per-call auth scheme override\: \`app\`, \`oauth1\`, \`oauth2\`, or \`none\`. \`none\` is refused for a command whose X API endpoint requires authentication]:SCHEME:_default' \
'--pretty[Pretty-print human-readable output]' \
'(-o --output --jsonl)--json[Shorthand for \`--output json\`]' \
'(-o --output)--jsonl[Shorthand for \`--output jsonl\`]' \
'-q[Suppress informational stderr output (keep only fatal errors)]' \
'--quiet[Suppress informational stderr output (keep only fatal errors)]' \
'*-v[Increase verbosity (repeatable\: -v info, -vv debug, -vvv trace)]' \
'*--verbose[Increase verbosity (repeatable\: -v info, -vv debug, -vvv trace)]' \
'--no-interactive[Disable interactive prompts (refuse anything that would block on stdin)]' \
'--raw[Emit unstyled, compact output\: pipe-safe text, and \`json\` on one line]' \
'--examples[Print curated examples block and exit]' \
'--refresh[Send every read instead of answering it from a stored response; still store the reply]' \
'--no-cache[Read and write no stored responses, notes, or ledger records]' \
'--cache-only[Answer only from stored responses; never make API requests]' \
'-h[Print help (see more with '\''--help'\'')]' \
'--help[Print help (see more with '\''--help'\'')]' \
'::command -- Scope report to this command only (e.g. me, bookmarks, search):_default' \
&& ret=0
;;
(cache)
_arguments "${_arguments_options[@]}" : \
'-u+[Signed-in account to use when the app holds tokens for several (as \`xr -u\`)]:USERNAME:_default' \
'--username=[Signed-in account to use when the app holds tokens for several (as \`xr -u\`)]:USERNAME:_default' \
'-o+[Output format (text, json, jsonl, ndjson). Defaults to json when piped]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON envelope, no color"
jsonl\:"Streaming line-delimited JSON (one object per line; no wrapper)"
ndjson\:"Newline-delimited JSON, accepted as an alias for jsonl"))' \
'--output=[Output format (text, json, jsonl, ndjson). Defaults to json when piped]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON envelope, no color"
jsonl\:"Streaming line-delimited JSON (one object per line; no wrapper)"
ndjson\:"Newline-delimited JSON, accepted as an alias for jsonl"))' \
'--color=[Color mode\: auto (default), always, never]:COLOR:((auto\:"Auto-detect\: color when stderr is a TTY and \`NO_COLOR\` is unset"
always\:"Always emit colors"
never\:"Never emit colors"))' \
'--timeout=[Network timeout in seconds (default 30) for each X API request]:TIMEOUT:_default' \
'--limit=[Maximum number of results to return on list-style commands (default 100, ceiling 1000)]:N:_default' \
'--cursor=[Pagination cursor token for list-style commands (X API \`pagination_token\`/\`next_token\`)]:TOKEN:_default' \
'--app=[App in the token store to use. \`--app\` wins over \`BIRD_APP\`, which wins over the store'\''s default app. bird does not read \`XURL_APP\`; when it is set and \`BIRD_APP\` is not, commands that call the X API print a warning]:NAME:_default' \
'--auth=[Per-call auth scheme override\: \`app\`, \`oauth1\`, \`oauth2\`, or \`none\`. \`none\` is refused for a command whose X API endpoint requires authentication]:SCHEME:_default' \
'(-o --output --jsonl)--json[Shorthand for \`--output json\`]' \
'(-o --output)--jsonl[Shorthand for \`--output jsonl\`]' \
'-q[Suppress informational stderr output (keep only fatal errors)]' \
'--quiet[Suppress informational stderr output (keep only fatal errors)]' \
'*-v[Increase verbosity (repeatable\: -v info, -vv debug, -vvv trace)]' \
'*--verbose[Increase verbosity (repeatable\: -v info, -vv debug, -vvv trace)]' \
'--no-interactive[Disable interactive prompts (refuse anything that would block on stdin)]' \
'--raw[Emit unstyled, compact output\: pipe-safe text, and \`json\` on one line]' \
'--examples[Print curated examples block and exit]' \
'--refresh[Send every read instead of answering it from a stored response; still store the reply]' \
'--no-cache[Read and write no stored responses, notes, or ledger records]' \
'--cache-only[Answer only from stored responses; never make API requests]' \
'-h[Print help (see more with '\''--help'\'')]' \
'--help[Print help (see more with '\''--help'\'')]' \
":: :_bird__subcmd__cache_commands" \
"*::: :->cache" \
&& ret=0

    case $state in
    (cache)
        words=($line[1] "${words[@]}")
        (( CURRENT += 1 ))
        curcontext="${curcontext%:*:*}:bird-cache-command-$line[1]:"
        case $line[1] in
            (clear)
_arguments "${_arguments_options[@]}" : \
'-u+[Signed-in account to use when the app holds tokens for several (as \`xr -u\`)]:USERNAME:_default' \
'--username=[Signed-in account to use when the app holds tokens for several (as \`xr -u\`)]:USERNAME:_default' \
'-o+[Output format (text, json, jsonl, ndjson). Defaults to json when piped]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON envelope, no color"
jsonl\:"Streaming line-delimited JSON (one object per line; no wrapper)"
ndjson\:"Newline-delimited JSON, accepted as an alias for jsonl"))' \
'--output=[Output format (text, json, jsonl, ndjson). Defaults to json when piped]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON envelope, no color"
jsonl\:"Streaming line-delimited JSON (one object per line; no wrapper)"
ndjson\:"Newline-delimited JSON, accepted as an alias for jsonl"))' \
'--color=[Color mode\: auto (default), always, never]:COLOR:((auto\:"Auto-detect\: color when stderr is a TTY and \`NO_COLOR\` is unset"
always\:"Always emit colors"
never\:"Never emit colors"))' \
'--timeout=[Network timeout in seconds (default 30) for each X API request]:TIMEOUT:_default' \
'--limit=[Maximum number of results to return on list-style commands (default 100, ceiling 1000)]:N:_default' \
'--cursor=[Pagination cursor token for list-style commands (X API \`pagination_token\`/\`next_token\`)]:TOKEN:_default' \
'--app=[App in the token store to use. \`--app\` wins over \`BIRD_APP\`, which wins over the store'\''s default app. bird does not read \`XURL_APP\`; when it is set and \`BIRD_APP\` is not, commands that call the X API print a warning]:NAME:_default' \
'--auth=[Per-call auth scheme override\: \`app\`, \`oauth1\`, \`oauth2\`, or \`none\`. \`none\` is refused for a command whose X API endpoint requires authentication]:SCHEME:_default' \
'-f[Skip the interactive confirmation prompt (alias\: --yes)]' \
'--force[Skip the interactive confirmation prompt (alias\: --yes)]' \
'--dry-run[Print the stored-responses directory that would be cleared, and remove nothing]' \
'(-o --output --jsonl)--json[Shorthand for \`--output json\`]' \
'(-o --output)--jsonl[Shorthand for \`--output jsonl\`]' \
'-q[Suppress informational stderr output (keep only fatal errors)]' \
'--quiet[Suppress informational stderr output (keep only fatal errors)]' \
'*-v[Increase verbosity (repeatable\: -v info, -vv debug, -vvv trace)]' \
'*--verbose[Increase verbosity (repeatable\: -v info, -vv debug, -vvv trace)]' \
'--no-interactive[Disable interactive prompts (refuse anything that would block on stdin)]' \
'--raw[Emit unstyled, compact output\: pipe-safe text, and \`json\` on one line]' \
'--examples[Print curated examples block and exit]' \
'--refresh[Send every read instead of answering it from a stored response; still store the reply]' \
'--no-cache[Read and write no stored responses, notes, or ledger records]' \
'--cache-only[Answer only from stored responses; never make API requests]' \
'-h[Print help (see more with '\''--help'\'')]' \
'--help[Print help (see more with '\''--help'\'')]' \
&& ret=0
;;
(stats)
_arguments "${_arguments_options[@]}" : \
'-u+[Signed-in account to use when the app holds tokens for several (as \`xr -u\`)]:USERNAME:_default' \
'--username=[Signed-in account to use when the app holds tokens for several (as \`xr -u\`)]:USERNAME:_default' \
'-o+[Output format (text, json, jsonl, ndjson). Defaults to json when piped]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON envelope, no color"
jsonl\:"Streaming line-delimited JSON (one object per line; no wrapper)"
ndjson\:"Newline-delimited JSON, accepted as an alias for jsonl"))' \
'--output=[Output format (text, json, jsonl, ndjson). Defaults to json when piped]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON envelope, no color"
jsonl\:"Streaming line-delimited JSON (one object per line; no wrapper)"
ndjson\:"Newline-delimited JSON, accepted as an alias for jsonl"))' \
'--color=[Color mode\: auto (default), always, never]:COLOR:((auto\:"Auto-detect\: color when stderr is a TTY and \`NO_COLOR\` is unset"
always\:"Always emit colors"
never\:"Never emit colors"))' \
'--timeout=[Network timeout in seconds (default 30) for each X API request]:TIMEOUT:_default' \
'--limit=[Maximum number of results to return on list-style commands (default 100, ceiling 1000)]:N:_default' \
'--cursor=[Pagination cursor token for list-style commands (X API \`pagination_token\`/\`next_token\`)]:TOKEN:_default' \
'--app=[App in the token store to use. \`--app\` wins over \`BIRD_APP\`, which wins over the store'\''s default app. bird does not read \`XURL_APP\`; when it is set and \`BIRD_APP\` is not, commands that call the X API print a warning]:NAME:_default' \
'--auth=[Per-call auth scheme override\: \`app\`, \`oauth1\`, \`oauth2\`, or \`none\`. \`none\` is refused for a command whose X API endpoint requires authentication]:SCHEME:_default' \
'--pretty[Pretty-print human-readable output]' \
'(-o --output --jsonl)--json[Shorthand for \`--output json\`]' \
'(-o --output)--jsonl[Shorthand for \`--output jsonl\`]' \
'-q[Suppress informational stderr output (keep only fatal errors)]' \
'--quiet[Suppress informational stderr output (keep only fatal errors)]' \
'*-v[Increase verbosity (repeatable\: -v info, -vv debug, -vvv trace)]' \
'*--verbose[Increase verbosity (repeatable\: -v info, -vv debug, -vvv trace)]' \
'--no-interactive[Disable interactive prompts (refuse anything that would block on stdin)]' \
'--raw[Emit unstyled, compact output\: pipe-safe text, and \`json\` on one line]' \
'--examples[Print curated examples block and exit]' \
'--refresh[Send every read instead of answering it from a stored response; still store the reply]' \
'--no-cache[Read and write no stored responses, notes, or ledger records]' \
'--cache-only[Answer only from stored responses; never make API requests]' \
'-h[Print help (see more with '\''--help'\'')]' \
'--help[Print help (see more with '\''--help'\'')]' \
&& ret=0
;;
(help)
_arguments "${_arguments_options[@]}" : \
":: :_bird__subcmd__cache__subcmd__help_commands" \
"*::: :->help" \
&& ret=0

    case $state in
    (help)
        words=($line[1] "${words[@]}")
        (( CURRENT += 1 ))
        curcontext="${curcontext%:*:*}:bird-cache-help-command-$line[1]:"
        case $line[1] in
            (clear)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
(stats)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
(help)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
        esac
    ;;
esac
;;
        esac
    ;;
esac
;;
(completions)
_arguments "${_arguments_options[@]}" : \
'-u+[Signed-in account to use when the app holds tokens for several (as \`xr -u\`)]:USERNAME:_default' \
'--username=[Signed-in account to use when the app holds tokens for several (as \`xr -u\`)]:USERNAME:_default' \
'-o+[Output format (text, json, jsonl, ndjson). Defaults to json when piped]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON envelope, no color"
jsonl\:"Streaming line-delimited JSON (one object per line; no wrapper)"
ndjson\:"Newline-delimited JSON, accepted as an alias for jsonl"))' \
'--output=[Output format (text, json, jsonl, ndjson). Defaults to json when piped]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON envelope, no color"
jsonl\:"Streaming line-delimited JSON (one object per line; no wrapper)"
ndjson\:"Newline-delimited JSON, accepted as an alias for jsonl"))' \
'--color=[Color mode\: auto (default), always, never]:COLOR:((auto\:"Auto-detect\: color when stderr is a TTY and \`NO_COLOR\` is unset"
always\:"Always emit colors"
never\:"Never emit colors"))' \
'--timeout=[Network timeout in seconds (default 30) for each X API request]:TIMEOUT:_default' \
'--limit=[Maximum number of results to return on list-style commands (default 100, ceiling 1000)]:N:_default' \
'--cursor=[Pagination cursor token for list-style commands (X API \`pagination_token\`/\`next_token\`)]:TOKEN:_default' \
'--app=[App in the token store to use. \`--app\` wins over \`BIRD_APP\`, which wins over the store'\''s default app. bird does not read \`XURL_APP\`; when it is set and \`BIRD_APP\` is not, commands that call the X API print a warning]:NAME:_default' \
'--auth=[Per-call auth scheme override\: \`app\`, \`oauth1\`, \`oauth2\`, or \`none\`. \`none\` is refused for a command whose X API endpoint requires authentication]:SCHEME:_default' \
'(-o --output --jsonl)--json[Shorthand for \`--output json\`]' \
'(-o --output)--jsonl[Shorthand for \`--output jsonl\`]' \
'-q[Suppress informational stderr output (keep only fatal errors)]' \
'--quiet[Suppress informational stderr output (keep only fatal errors)]' \
'*-v[Increase verbosity (repeatable\: -v info, -vv debug, -vvv trace)]' \
'*--verbose[Increase verbosity (repeatable\: -v info, -vv debug, -vvv trace)]' \
'--no-interactive[Disable interactive prompts (refuse anything that would block on stdin)]' \
'--raw[Emit unstyled, compact output\: pipe-safe text, and \`json\` on one line]' \
'--examples[Print curated examples block and exit]' \
'--refresh[Send every read instead of answering it from a stored response; still store the reply]' \
'--no-cache[Read and write no stored responses, notes, or ledger records]' \
'--cache-only[Answer only from stored responses; never make API requests]' \
'-h[Print help (see more with '\''--help'\'')]' \
'--help[Print help (see more with '\''--help'\'')]' \
':shell -- Shell to generate completions for:(bash elvish fish powershell zsh)' \
&& ret=0
;;
(skill)
_arguments "${_arguments_options[@]}" : \
'-u+[Signed-in account to use when the app holds tokens for several (as \`xr -u\`)]:USERNAME:_default' \
'--username=[Signed-in account to use when the app holds tokens for several (as \`xr -u\`)]:USERNAME:_default' \
'-o+[Output format (text, json, jsonl, ndjson). Defaults to json when piped]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON envelope, no color"
jsonl\:"Streaming line-delimited JSON (one object per line; no wrapper)"
ndjson\:"Newline-delimited JSON, accepted as an alias for jsonl"))' \
'--output=[Output format (text, json, jsonl, ndjson). Defaults to json when piped]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON envelope, no color"
jsonl\:"Streaming line-delimited JSON (one object per line; no wrapper)"
ndjson\:"Newline-delimited JSON, accepted as an alias for jsonl"))' \
'--color=[Color mode\: auto (default), always, never]:COLOR:((auto\:"Auto-detect\: color when stderr is a TTY and \`NO_COLOR\` is unset"
always\:"Always emit colors"
never\:"Never emit colors"))' \
'--timeout=[Network timeout in seconds (default 30) for each X API request]:TIMEOUT:_default' \
'--limit=[Maximum number of results to return on list-style commands (default 100, ceiling 1000)]:N:_default' \
'--cursor=[Pagination cursor token for list-style commands (X API \`pagination_token\`/\`next_token\`)]:TOKEN:_default' \
'--app=[App in the token store to use. \`--app\` wins over \`BIRD_APP\`, which wins over the store'\''s default app. bird does not read \`XURL_APP\`; when it is set and \`BIRD_APP\` is not, commands that call the X API print a warning]:NAME:_default' \
'--auth=[Per-call auth scheme override\: \`app\`, \`oauth1\`, \`oauth2\`, or \`none\`. \`none\` is refused for a command whose X API endpoint requires authentication]:SCHEME:_default' \
'(-o --output --jsonl)--json[Shorthand for \`--output json\`]' \
'(-o --output)--jsonl[Shorthand for \`--output jsonl\`]' \
'-q[Suppress informational stderr output (keep only fatal errors)]' \
'--quiet[Suppress informational stderr output (keep only fatal errors)]' \
'*-v[Increase verbosity (repeatable\: -v info, -vv debug, -vvv trace)]' \
'*--verbose[Increase verbosity (repeatable\: -v info, -vv debug, -vvv trace)]' \
'--no-interactive[Disable interactive prompts (refuse anything that would block on stdin)]' \
'--raw[Emit unstyled, compact output\: pipe-safe text, and \`json\` on one line]' \
'--examples[Print curated examples block and exit]' \
'--refresh[Send every read instead of answering it from a stored response; still store the reply]' \
'--no-cache[Read and write no stored responses, notes, or ledger records]' \
'--cache-only[Answer only from stored responses; never make API requests]' \
'-h[Print help (see more with '\''--help'\'')]' \
'--help[Print help (see more with '\''--help'\'')]' \
":: :_bird__subcmd__skill_commands" \
"*::: :->skill" \
&& ret=0

    case $state in
    (skill)
        words=($line[1] "${words[@]}")
        (( CURRENT += 1 ))
        curcontext="${curcontext%:*:*}:bird-skill-command-$line[1]:"
        case $line[1] in
            (install)
_arguments "${_arguments_options[@]}" : \
'-u+[Signed-in account to use when the app holds tokens for several (as \`xr -u\`)]:USERNAME:_default' \
'--username=[Signed-in account to use when the app holds tokens for several (as \`xr -u\`)]:USERNAME:_default' \
'-o+[Output format (text, json, jsonl, ndjson). Defaults to json when piped]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON envelope, no color"
jsonl\:"Streaming line-delimited JSON (one object per line; no wrapper)"
ndjson\:"Newline-delimited JSON, accepted as an alias for jsonl"))' \
'--output=[Output format (text, json, jsonl, ndjson). Defaults to json when piped]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON envelope, no color"
jsonl\:"Streaming line-delimited JSON (one object per line; no wrapper)"
ndjson\:"Newline-delimited JSON, accepted as an alias for jsonl"))' \
'--color=[Color mode\: auto (default), always, never]:COLOR:((auto\:"Auto-detect\: color when stderr is a TTY and \`NO_COLOR\` is unset"
always\:"Always emit colors"
never\:"Never emit colors"))' \
'--timeout=[Network timeout in seconds (default 30) for each X API request]:TIMEOUT:_default' \
'--limit=[Maximum number of results to return on list-style commands (default 100, ceiling 1000)]:N:_default' \
'--cursor=[Pagination cursor token for list-style commands (X API \`pagination_token\`/\`next_token\`)]:TOKEN:_default' \
'--app=[App in the token store to use. \`--app\` wins over \`BIRD_APP\`, which wins over the store'\''s default app. bird does not read \`XURL_APP\`; when it is set and \`BIRD_APP\` is not, commands that call the X API print a warning]:NAME:_default' \
'--auth=[Per-call auth scheme override\: \`app\`, \`oauth1\`, \`oauth2\`, or \`none\`. \`none\` is refused for a command whose X API endpoint requires authentication]:SCHEME:_default' \
'--all[Install into every supported host in one invocation]' \
'--dry-run[Print the planned clone command without spawning git]' \
'(-o --output --jsonl)--json[Shorthand for \`--output json\`]' \
'(-o --output)--jsonl[Shorthand for \`--output jsonl\`]' \
'-q[Suppress informational stderr output (keep only fatal errors)]' \
'--quiet[Suppress informational stderr output (keep only fatal errors)]' \
'*-v[Increase verbosity (repeatable\: -v info, -vv debug, -vvv trace)]' \
'*--verbose[Increase verbosity (repeatable\: -v info, -vv debug, -vvv trace)]' \
'--no-interactive[Disable interactive prompts (refuse anything that would block on stdin)]' \
'--raw[Emit unstyled, compact output\: pipe-safe text, and \`json\` on one line]' \
'--examples[Print curated examples block and exit]' \
'--refresh[Send every read instead of answering it from a stored response; still store the reply]' \
'--no-cache[Read and write no stored responses, notes, or ledger records]' \
'--cache-only[Answer only from stored responses; never make API requests]' \
'-h[Print help (see more with '\''--help'\'')]' \
'--help[Print help (see more with '\''--help'\'')]' \
'::host -- Target host (omit and pass --all to install everywhere):(claude_code codex cursor factory kiro opencode)' \
&& ret=0
;;
(update)
_arguments "${_arguments_options[@]}" : \
'-u+[Signed-in account to use when the app holds tokens for several (as \`xr -u\`)]:USERNAME:_default' \
'--username=[Signed-in account to use when the app holds tokens for several (as \`xr -u\`)]:USERNAME:_default' \
'-o+[Output format (text, json, jsonl, ndjson). Defaults to json when piped]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON envelope, no color"
jsonl\:"Streaming line-delimited JSON (one object per line; no wrapper)"
ndjson\:"Newline-delimited JSON, accepted as an alias for jsonl"))' \
'--output=[Output format (text, json, jsonl, ndjson). Defaults to json when piped]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON envelope, no color"
jsonl\:"Streaming line-delimited JSON (one object per line; no wrapper)"
ndjson\:"Newline-delimited JSON, accepted as an alias for jsonl"))' \
'--color=[Color mode\: auto (default), always, never]:COLOR:((auto\:"Auto-detect\: color when stderr is a TTY and \`NO_COLOR\` is unset"
always\:"Always emit colors"
never\:"Never emit colors"))' \
'--timeout=[Network timeout in seconds (default 30) for each X API request]:TIMEOUT:_default' \
'--limit=[Maximum number of results to return on list-style commands (default 100, ceiling 1000)]:N:_default' \
'--cursor=[Pagination cursor token for list-style commands (X API \`pagination_token\`/\`next_token\`)]:TOKEN:_default' \
'--app=[App in the token store to use. \`--app\` wins over \`BIRD_APP\`, which wins over the store'\''s default app. bird does not read \`XURL_APP\`; when it is set and \`BIRD_APP\` is not, commands that call the X API print a warning]:NAME:_default' \
'--auth=[Per-call auth scheme override\: \`app\`, \`oauth1\`, \`oauth2\`, or \`none\`. \`none\` is refused for a command whose X API endpoint requires authentication]:SCHEME:_default' \
'--all[Update every supported host that has a copy; report the rest as skipped]' \
'--dry-run[Print the planned operation without touching the filesystem]' \
'(-o --output --jsonl)--json[Shorthand for \`--output json\`]' \
'(-o --output)--jsonl[Shorthand for \`--output jsonl\`]' \
'-q[Suppress informational stderr output (keep only fatal errors)]' \
'--quiet[Suppress informational stderr output (keep only fatal errors)]' \
'*-v[Increase verbosity (repeatable\: -v info, -vv debug, -vvv trace)]' \
'*--verbose[Increase verbosity (repeatable\: -v info, -vv debug, -vvv trace)]' \
'--no-interactive[Disable interactive prompts (refuse anything that would block on stdin)]' \
'--raw[Emit unstyled, compact output\: pipe-safe text, and \`json\` on one line]' \
'--examples[Print curated examples block and exit]' \
'--refresh[Send every read instead of answering it from a stored response; still store the reply]' \
'--no-cache[Read and write no stored responses, notes, or ledger records]' \
'--cache-only[Answer only from stored responses; never make API requests]' \
'-h[Print help (see more with '\''--help'\'')]' \
'--help[Print help (see more with '\''--help'\'')]' \
'::host -- Target host (omit and pass --all to update everywhere):(claude_code codex cursor factory kiro opencode)' \
&& ret=0
;;
(help)
_arguments "${_arguments_options[@]}" : \
":: :_bird__subcmd__skill__subcmd__help_commands" \
"*::: :->help" \
&& ret=0

    case $state in
    (help)
        words=($line[1] "${words[@]}")
        (( CURRENT += 1 ))
        curcontext="${curcontext%:*:*}:bird-skill-help-command-$line[1]:"
        case $line[1] in
            (install)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
(update)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
(help)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
        esac
    ;;
esac
;;
        esac
    ;;
esac
;;
(schema)
_arguments "${_arguments_options[@]}" : \
'-u+[Signed-in account to use when the app holds tokens for several (as \`xr -u\`)]:USERNAME:_default' \
'--username=[Signed-in account to use when the app holds tokens for several (as \`xr -u\`)]:USERNAME:_default' \
'-o+[Output format (text, json, jsonl, ndjson). Defaults to json when piped]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON envelope, no color"
jsonl\:"Streaming line-delimited JSON (one object per line; no wrapper)"
ndjson\:"Newline-delimited JSON, accepted as an alias for jsonl"))' \
'--output=[Output format (text, json, jsonl, ndjson). Defaults to json when piped]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON envelope, no color"
jsonl\:"Streaming line-delimited JSON (one object per line; no wrapper)"
ndjson\:"Newline-delimited JSON, accepted as an alias for jsonl"))' \
'--color=[Color mode\: auto (default), always, never]:COLOR:((auto\:"Auto-detect\: color when stderr is a TTY and \`NO_COLOR\` is unset"
always\:"Always emit colors"
never\:"Never emit colors"))' \
'--timeout=[Network timeout in seconds (default 30) for each X API request]:TIMEOUT:_default' \
'--limit=[Maximum number of results to return on list-style commands (default 100, ceiling 1000)]:N:_default' \
'--cursor=[Pagination cursor token for list-style commands (X API \`pagination_token\`/\`next_token\`)]:TOKEN:_default' \
'--app=[App in the token store to use. \`--app\` wins over \`BIRD_APP\`, which wins over the store'\''s default app. bird does not read \`XURL_APP\`; when it is set and \`BIRD_APP\` is not, commands that call the X API print a warning]:NAME:_default' \
'--auth=[Per-call auth scheme override\: \`app\`, \`oauth1\`, \`oauth2\`, or \`none\`. \`none\` is refused for a command whose X API endpoint requires authentication]:SCHEME:_default' \
'(--all)--list[List every schema name and title instead of printing a schema]' \
'--all[Print every schema in one object keyed by name]' \
'(-o --output --jsonl)--json[Shorthand for \`--output json\`]' \
'(-o --output)--jsonl[Shorthand for \`--output jsonl\`]' \
'-q[Suppress informational stderr output (keep only fatal errors)]' \
'--quiet[Suppress informational stderr output (keep only fatal errors)]' \
'*-v[Increase verbosity (repeatable\: -v info, -vv debug, -vvv trace)]' \
'*--verbose[Increase verbosity (repeatable\: -v info, -vv debug, -vvv trace)]' \
'--no-interactive[Disable interactive prompts (refuse anything that would block on stdin)]' \
'--raw[Emit unstyled, compact output\: pipe-safe text, and \`json\` on one line]' \
'--examples[Print curated examples block and exit]' \
'--refresh[Send every read instead of answering it from a stored response; still store the reply]' \
'--no-cache[Read and write no stored responses, notes, or ledger records]' \
'--cache-only[Answer only from stored responses; never make API requests]' \
'-h[Print help (see more with '\''--help'\'')]' \
'--help[Print help (see more with '\''--help'\'')]' \
'::name -- Command whose output schema to print, or `envelope`:_default' \
&& ret=0
;;
(help)
_arguments "${_arguments_options[@]}" : \
":: :_bird__subcmd__help_commands" \
"*::: :->help" \
&& ret=0

    case $state in
    (help)
        words=($line[1] "${words[@]}")
        (( CURRENT += 1 ))
        curcontext="${curcontext%:*:*}:bird-help-command-$line[1]:"
        case $line[1] in
            (login)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
(me)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
(bookmarks)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
(profile)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
(search)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
(thread)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
(watchlist)
_arguments "${_arguments_options[@]}" : \
":: :_bird__subcmd__help__subcmd__watchlist_commands" \
"*::: :->watchlist" \
&& ret=0

    case $state in
    (watchlist)
        words=($line[1] "${words[@]}")
        (( CURRENT += 1 ))
        curcontext="${curcontext%:*:*}:bird-help-watchlist-command-$line[1]:"
        case $line[1] in
            (fetch)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
(add)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
(remove)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
(list)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
        esac
    ;;
esac
;;
(spend)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
(tweet)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
(reply)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
(like)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
(unlike)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
(repost)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
(unrepost)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
(follow)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
(unfollow)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
(dm)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
(block)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
(unblock)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
(mute)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
(unmute)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
(doctor)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
(cache)
_arguments "${_arguments_options[@]}" : \
":: :_bird__subcmd__help__subcmd__cache_commands" \
"*::: :->cache" \
&& ret=0

    case $state in
    (cache)
        words=($line[1] "${words[@]}")
        (( CURRENT += 1 ))
        curcontext="${curcontext%:*:*}:bird-help-cache-command-$line[1]:"
        case $line[1] in
            (clear)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
(stats)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
        esac
    ;;
esac
;;
(completions)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
(skill)
_arguments "${_arguments_options[@]}" : \
":: :_bird__subcmd__help__subcmd__skill_commands" \
"*::: :->skill" \
&& ret=0

    case $state in
    (skill)
        words=($line[1] "${words[@]}")
        (( CURRENT += 1 ))
        curcontext="${curcontext%:*:*}:bird-help-skill-command-$line[1]:"
        case $line[1] in
            (install)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
(update)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
        esac
    ;;
esac
;;
(schema)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
(help)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
        esac
    ;;
esac
;;
        esac
    ;;
esac
}

(( $+functions[_bird_commands] )) ||
_bird_commands() {
    local commands; commands=(
'login:Authenticate via xurl (OAuth2 PKCE browser flow)' \
'me:Show current user (GET /2/users/me)' \
'bookmarks:List bookmarks for the current user (paginated, max_results=100)' \
'profile:Look up a user profile by username' \
'search:Search recent tweets (GET /2/tweets/search/recent)' \
'thread:Reconstruct a conversation thread from a post' \
'watchlist:Monitor users\: check recent activity, manage watchlist' \
'spend:Show the local spend ledger\: estimated API cost per call' \
'tweet:Post a tweet (via xurl)' \
'reply:Reply to a tweet (via xurl)' \
'like:Like a tweet (via xurl)' \
'unlike:Unlike a tweet (via xurl)' \
'repost:Repost (retweet) a tweet (via xurl)' \
'unrepost:Undo a repost (via xurl)' \
'follow:Follow a user (via xurl)' \
'unfollow:Unfollow a user (via xurl)' \
'dm:Send a direct message (via xurl)' \
'block:Block a user (via xurl)' \
'unblock:Unblock a user (via xurl)' \
'mute:Mute a user (via xurl)' \
'unmute:Unmute a user (via xurl)' \
'doctor:Show what is available\: the X API client, auth, commands, and the file store' \
'cache:Report the file store and clear its stored responses' \
'completions:Generate shell completions' \
'skill:Manage the bird agent-skill bundle (clone from brettdavies/bird-skill into a host'\''s skills dir)' \
'schema:Print the JSON Schema of a command'\''s output, or of the envelope' \
'help:Print this message or the help of the given subcommand(s)' \
    )
    _describe -t commands 'bird commands' commands "$@"
}
(( $+functions[_bird__subcmd__block_commands] )) ||
_bird__subcmd__block_commands() {
    local commands; commands=()
    _describe -t commands 'bird block commands' commands "$@"
}
(( $+functions[_bird__subcmd__bookmarks_commands] )) ||
_bird__subcmd__bookmarks_commands() {
    local commands; commands=()
    _describe -t commands 'bird bookmarks commands' commands "$@"
}
(( $+functions[_bird__subcmd__cache_commands] )) ||
_bird__subcmd__cache_commands() {
    local commands; commands=(
'clear:Delete every stored response; the archive and the ledger stay' \
'stats:Show the archive, stored responses, and ledger' \
'help:Print this message or the help of the given subcommand(s)' \
    )
    _describe -t commands 'bird cache commands' commands "$@"
}
(( $+functions[_bird__subcmd__cache__subcmd__clear_commands] )) ||
_bird__subcmd__cache__subcmd__clear_commands() {
    local commands; commands=()
    _describe -t commands 'bird cache clear commands' commands "$@"
}
(( $+functions[_bird__subcmd__cache__subcmd__help_commands] )) ||
_bird__subcmd__cache__subcmd__help_commands() {
    local commands; commands=(
'clear:Delete every stored response; the archive and the ledger stay' \
'stats:Show the archive, stored responses, and ledger' \
'help:Print this message or the help of the given subcommand(s)' \
    )
    _describe -t commands 'bird cache help commands' commands "$@"
}
(( $+functions[_bird__subcmd__cache__subcmd__help__subcmd__clear_commands] )) ||
_bird__subcmd__cache__subcmd__help__subcmd__clear_commands() {
    local commands; commands=()
    _describe -t commands 'bird cache help clear commands' commands "$@"
}
(( $+functions[_bird__subcmd__cache__subcmd__help__subcmd__help_commands] )) ||
_bird__subcmd__cache__subcmd__help__subcmd__help_commands() {
    local commands; commands=()
    _describe -t commands 'bird cache help help commands' commands "$@"
}
(( $+functions[_bird__subcmd__cache__subcmd__help__subcmd__stats_commands] )) ||
_bird__subcmd__cache__subcmd__help__subcmd__stats_commands() {
    local commands; commands=()
    _describe -t commands 'bird cache help stats commands' commands "$@"
}
(( $+functions[_bird__subcmd__cache__subcmd__stats_commands] )) ||
_bird__subcmd__cache__subcmd__stats_commands() {
    local commands; commands=()
    _describe -t commands 'bird cache stats commands' commands "$@"
}
(( $+functions[_bird__subcmd__completions_commands] )) ||
_bird__subcmd__completions_commands() {
    local commands; commands=()
    _describe -t commands 'bird completions commands' commands "$@"
}
(( $+functions[_bird__subcmd__dm_commands] )) ||
_bird__subcmd__dm_commands() {
    local commands; commands=()
    _describe -t commands 'bird dm commands' commands "$@"
}
(( $+functions[_bird__subcmd__doctor_commands] )) ||
_bird__subcmd__doctor_commands() {
    local commands; commands=()
    _describe -t commands 'bird doctor commands' commands "$@"
}
(( $+functions[_bird__subcmd__follow_commands] )) ||
_bird__subcmd__follow_commands() {
    local commands; commands=()
    _describe -t commands 'bird follow commands' commands "$@"
}
(( $+functions[_bird__subcmd__help_commands] )) ||
_bird__subcmd__help_commands() {
    local commands; commands=(
'login:Authenticate via xurl (OAuth2 PKCE browser flow)' \
'me:Show current user (GET /2/users/me)' \
'bookmarks:List bookmarks for the current user (paginated, max_results=100)' \
'profile:Look up a user profile by username' \
'search:Search recent tweets (GET /2/tweets/search/recent)' \
'thread:Reconstruct a conversation thread from a post' \
'watchlist:Monitor users\: check recent activity, manage watchlist' \
'spend:Show the local spend ledger\: estimated API cost per call' \
'tweet:Post a tweet (via xurl)' \
'reply:Reply to a tweet (via xurl)' \
'like:Like a tweet (via xurl)' \
'unlike:Unlike a tweet (via xurl)' \
'repost:Repost (retweet) a tweet (via xurl)' \
'unrepost:Undo a repost (via xurl)' \
'follow:Follow a user (via xurl)' \
'unfollow:Unfollow a user (via xurl)' \
'dm:Send a direct message (via xurl)' \
'block:Block a user (via xurl)' \
'unblock:Unblock a user (via xurl)' \
'mute:Mute a user (via xurl)' \
'unmute:Unmute a user (via xurl)' \
'doctor:Show what is available\: the X API client, auth, commands, and the file store' \
'cache:Report the file store and clear its stored responses' \
'completions:Generate shell completions' \
'skill:Manage the bird agent-skill bundle (clone from brettdavies/bird-skill into a host'\''s skills dir)' \
'schema:Print the JSON Schema of a command'\''s output, or of the envelope' \
'help:Print this message or the help of the given subcommand(s)' \
    )
    _describe -t commands 'bird help commands' commands "$@"
}
(( $+functions[_bird__subcmd__help__subcmd__block_commands] )) ||
_bird__subcmd__help__subcmd__block_commands() {
    local commands; commands=()
    _describe -t commands 'bird help block commands' commands "$@"
}
(( $+functions[_bird__subcmd__help__subcmd__bookmarks_commands] )) ||
_bird__subcmd__help__subcmd__bookmarks_commands() {
    local commands; commands=()
    _describe -t commands 'bird help bookmarks commands' commands "$@"
}
(( $+functions[_bird__subcmd__help__subcmd__cache_commands] )) ||
_bird__subcmd__help__subcmd__cache_commands() {
    local commands; commands=(
'clear:Delete every stored response; the archive and the ledger stay' \
'stats:Show the archive, stored responses, and ledger' \
    )
    _describe -t commands 'bird help cache commands' commands "$@"
}
(( $+functions[_bird__subcmd__help__subcmd__cache__subcmd__clear_commands] )) ||
_bird__subcmd__help__subcmd__cache__subcmd__clear_commands() {
    local commands; commands=()
    _describe -t commands 'bird help cache clear commands' commands "$@"
}
(( $+functions[_bird__subcmd__help__subcmd__cache__subcmd__stats_commands] )) ||
_bird__subcmd__help__subcmd__cache__subcmd__stats_commands() {
    local commands; commands=()
    _describe -t commands 'bird help cache stats commands' commands "$@"
}
(( $+functions[_bird__subcmd__help__subcmd__completions_commands] )) ||
_bird__subcmd__help__subcmd__completions_commands() {
    local commands; commands=()
    _describe -t commands 'bird help completions commands' commands "$@"
}
(( $+functions[_bird__subcmd__help__subcmd__dm_commands] )) ||
_bird__subcmd__help__subcmd__dm_commands() {
    local commands; commands=()
    _describe -t commands 'bird help dm commands' commands "$@"
}
(( $+functions[_bird__subcmd__help__subcmd__doctor_commands] )) ||
_bird__subcmd__help__subcmd__doctor_commands() {
    local commands; commands=()
    _describe -t commands 'bird help doctor commands' commands "$@"
}
(( $+functions[_bird__subcmd__help__subcmd__follow_commands] )) ||
_bird__subcmd__help__subcmd__follow_commands() {
    local commands; commands=()
    _describe -t commands 'bird help follow commands' commands "$@"
}
(( $+functions[_bird__subcmd__help__subcmd__help_commands] )) ||
_bird__subcmd__help__subcmd__help_commands() {
    local commands; commands=()
    _describe -t commands 'bird help help commands' commands "$@"
}
(( $+functions[_bird__subcmd__help__subcmd__like_commands] )) ||
_bird__subcmd__help__subcmd__like_commands() {
    local commands; commands=()
    _describe -t commands 'bird help like commands' commands "$@"
}
(( $+functions[_bird__subcmd__help__subcmd__login_commands] )) ||
_bird__subcmd__help__subcmd__login_commands() {
    local commands; commands=()
    _describe -t commands 'bird help login commands' commands "$@"
}
(( $+functions[_bird__subcmd__help__subcmd__me_commands] )) ||
_bird__subcmd__help__subcmd__me_commands() {
    local commands; commands=()
    _describe -t commands 'bird help me commands' commands "$@"
}
(( $+functions[_bird__subcmd__help__subcmd__mute_commands] )) ||
_bird__subcmd__help__subcmd__mute_commands() {
    local commands; commands=()
    _describe -t commands 'bird help mute commands' commands "$@"
}
(( $+functions[_bird__subcmd__help__subcmd__profile_commands] )) ||
_bird__subcmd__help__subcmd__profile_commands() {
    local commands; commands=()
    _describe -t commands 'bird help profile commands' commands "$@"
}
(( $+functions[_bird__subcmd__help__subcmd__reply_commands] )) ||
_bird__subcmd__help__subcmd__reply_commands() {
    local commands; commands=()
    _describe -t commands 'bird help reply commands' commands "$@"
}
(( $+functions[_bird__subcmd__help__subcmd__repost_commands] )) ||
_bird__subcmd__help__subcmd__repost_commands() {
    local commands; commands=()
    _describe -t commands 'bird help repost commands' commands "$@"
}
(( $+functions[_bird__subcmd__help__subcmd__schema_commands] )) ||
_bird__subcmd__help__subcmd__schema_commands() {
    local commands; commands=()
    _describe -t commands 'bird help schema commands' commands "$@"
}
(( $+functions[_bird__subcmd__help__subcmd__search_commands] )) ||
_bird__subcmd__help__subcmd__search_commands() {
    local commands; commands=()
    _describe -t commands 'bird help search commands' commands "$@"
}
(( $+functions[_bird__subcmd__help__subcmd__skill_commands] )) ||
_bird__subcmd__help__subcmd__skill_commands() {
    local commands; commands=(
'install:Clone the bird skill bundle into a host'\''s canonical skills directory' \
'update:Remove the existing destination and re-clone the bird skill bundle' \
    )
    _describe -t commands 'bird help skill commands' commands "$@"
}
(( $+functions[_bird__subcmd__help__subcmd__skill__subcmd__install_commands] )) ||
_bird__subcmd__help__subcmd__skill__subcmd__install_commands() {
    local commands; commands=()
    _describe -t commands 'bird help skill install commands' commands "$@"
}
(( $+functions[_bird__subcmd__help__subcmd__skill__subcmd__update_commands] )) ||
_bird__subcmd__help__subcmd__skill__subcmd__update_commands() {
    local commands; commands=()
    _describe -t commands 'bird help skill update commands' commands "$@"
}
(( $+functions[_bird__subcmd__help__subcmd__spend_commands] )) ||
_bird__subcmd__help__subcmd__spend_commands() {
    local commands; commands=()
    _describe -t commands 'bird help spend commands' commands "$@"
}
(( $+functions[_bird__subcmd__help__subcmd__thread_commands] )) ||
_bird__subcmd__help__subcmd__thread_commands() {
    local commands; commands=()
    _describe -t commands 'bird help thread commands' commands "$@"
}
(( $+functions[_bird__subcmd__help__subcmd__tweet_commands] )) ||
_bird__subcmd__help__subcmd__tweet_commands() {
    local commands; commands=()
    _describe -t commands 'bird help tweet commands' commands "$@"
}
(( $+functions[_bird__subcmd__help__subcmd__unblock_commands] )) ||
_bird__subcmd__help__subcmd__unblock_commands() {
    local commands; commands=()
    _describe -t commands 'bird help unblock commands' commands "$@"
}
(( $+functions[_bird__subcmd__help__subcmd__unfollow_commands] )) ||
_bird__subcmd__help__subcmd__unfollow_commands() {
    local commands; commands=()
    _describe -t commands 'bird help unfollow commands' commands "$@"
}
(( $+functions[_bird__subcmd__help__subcmd__unlike_commands] )) ||
_bird__subcmd__help__subcmd__unlike_commands() {
    local commands; commands=()
    _describe -t commands 'bird help unlike commands' commands "$@"
}
(( $+functions[_bird__subcmd__help__subcmd__unmute_commands] )) ||
_bird__subcmd__help__subcmd__unmute_commands() {
    local commands; commands=()
    _describe -t commands 'bird help unmute commands' commands "$@"
}
(( $+functions[_bird__subcmd__help__subcmd__unrepost_commands] )) ||
_bird__subcmd__help__subcmd__unrepost_commands() {
    local commands; commands=()
    _describe -t commands 'bird help unrepost commands' commands "$@"
}
(( $+functions[_bird__subcmd__help__subcmd__watchlist_commands] )) ||
_bird__subcmd__help__subcmd__watchlist_commands() {
    local commands; commands=(
'fetch:Fetch recent activity for all watched users' \
'add:Add a user to the watchlist' \
'remove:Remove a user from the watchlist' \
'list:Show the current watchlist' \
    )
    _describe -t commands 'bird help watchlist commands' commands "$@"
}
(( $+functions[_bird__subcmd__help__subcmd__watchlist__subcmd__add_commands] )) ||
_bird__subcmd__help__subcmd__watchlist__subcmd__add_commands() {
    local commands; commands=()
    _describe -t commands 'bird help watchlist add commands' commands "$@"
}
(( $+functions[_bird__subcmd__help__subcmd__watchlist__subcmd__fetch_commands] )) ||
_bird__subcmd__help__subcmd__watchlist__subcmd__fetch_commands() {
    local commands; commands=()
    _describe -t commands 'bird help watchlist fetch commands' commands "$@"
}
(( $+functions[_bird__subcmd__help__subcmd__watchlist__subcmd__list_commands] )) ||
_bird__subcmd__help__subcmd__watchlist__subcmd__list_commands() {
    local commands; commands=()
    _describe -t commands 'bird help watchlist list commands' commands "$@"
}
(( $+functions[_bird__subcmd__help__subcmd__watchlist__subcmd__remove_commands] )) ||
_bird__subcmd__help__subcmd__watchlist__subcmd__remove_commands() {
    local commands; commands=()
    _describe -t commands 'bird help watchlist remove commands' commands "$@"
}
(( $+functions[_bird__subcmd__like_commands] )) ||
_bird__subcmd__like_commands() {
    local commands; commands=()
    _describe -t commands 'bird like commands' commands "$@"
}
(( $+functions[_bird__subcmd__login_commands] )) ||
_bird__subcmd__login_commands() {
    local commands; commands=()
    _describe -t commands 'bird login commands' commands "$@"
}
(( $+functions[_bird__subcmd__me_commands] )) ||
_bird__subcmd__me_commands() {
    local commands; commands=()
    _describe -t commands 'bird me commands' commands "$@"
}
(( $+functions[_bird__subcmd__mute_commands] )) ||
_bird__subcmd__mute_commands() {
    local commands; commands=()
    _describe -t commands 'bird mute commands' commands "$@"
}
(( $+functions[_bird__subcmd__profile_commands] )) ||
_bird__subcmd__profile_commands() {
    local commands; commands=()
    _describe -t commands 'bird profile commands' commands "$@"
}
(( $+functions[_bird__subcmd__reply_commands] )) ||
_bird__subcmd__reply_commands() {
    local commands; commands=()
    _describe -t commands 'bird reply commands' commands "$@"
}
(( $+functions[_bird__subcmd__repost_commands] )) ||
_bird__subcmd__repost_commands() {
    local commands; commands=()
    _describe -t commands 'bird repost commands' commands "$@"
}
(( $+functions[_bird__subcmd__schema_commands] )) ||
_bird__subcmd__schema_commands() {
    local commands; commands=()
    _describe -t commands 'bird schema commands' commands "$@"
}
(( $+functions[_bird__subcmd__search_commands] )) ||
_bird__subcmd__search_commands() {
    local commands; commands=()
    _describe -t commands 'bird search commands' commands "$@"
}
(( $+functions[_bird__subcmd__skill_commands] )) ||
_bird__subcmd__skill_commands() {
    local commands; commands=(
'install:Clone the bird skill bundle into a host'\''s canonical skills directory' \
'update:Remove the existing destination and re-clone the bird skill bundle' \
'help:Print this message or the help of the given subcommand(s)' \
    )
    _describe -t commands 'bird skill commands' commands "$@"
}
(( $+functions[_bird__subcmd__skill__subcmd__help_commands] )) ||
_bird__subcmd__skill__subcmd__help_commands() {
    local commands; commands=(
'install:Clone the bird skill bundle into a host'\''s canonical skills directory' \
'update:Remove the existing destination and re-clone the bird skill bundle' \
'help:Print this message or the help of the given subcommand(s)' \
    )
    _describe -t commands 'bird skill help commands' commands "$@"
}
(( $+functions[_bird__subcmd__skill__subcmd__help__subcmd__help_commands] )) ||
_bird__subcmd__skill__subcmd__help__subcmd__help_commands() {
    local commands; commands=()
    _describe -t commands 'bird skill help help commands' commands "$@"
}
(( $+functions[_bird__subcmd__skill__subcmd__help__subcmd__install_commands] )) ||
_bird__subcmd__skill__subcmd__help__subcmd__install_commands() {
    local commands; commands=()
    _describe -t commands 'bird skill help install commands' commands "$@"
}
(( $+functions[_bird__subcmd__skill__subcmd__help__subcmd__update_commands] )) ||
_bird__subcmd__skill__subcmd__help__subcmd__update_commands() {
    local commands; commands=()
    _describe -t commands 'bird skill help update commands' commands "$@"
}
(( $+functions[_bird__subcmd__skill__subcmd__install_commands] )) ||
_bird__subcmd__skill__subcmd__install_commands() {
    local commands; commands=()
    _describe -t commands 'bird skill install commands' commands "$@"
}
(( $+functions[_bird__subcmd__skill__subcmd__update_commands] )) ||
_bird__subcmd__skill__subcmd__update_commands() {
    local commands; commands=()
    _describe -t commands 'bird skill update commands' commands "$@"
}
(( $+functions[_bird__subcmd__spend_commands] )) ||
_bird__subcmd__spend_commands() {
    local commands; commands=()
    _describe -t commands 'bird spend commands' commands "$@"
}
(( $+functions[_bird__subcmd__thread_commands] )) ||
_bird__subcmd__thread_commands() {
    local commands; commands=()
    _describe -t commands 'bird thread commands' commands "$@"
}
(( $+functions[_bird__subcmd__tweet_commands] )) ||
_bird__subcmd__tweet_commands() {
    local commands; commands=()
    _describe -t commands 'bird tweet commands' commands "$@"
}
(( $+functions[_bird__subcmd__unblock_commands] )) ||
_bird__subcmd__unblock_commands() {
    local commands; commands=()
    _describe -t commands 'bird unblock commands' commands "$@"
}
(( $+functions[_bird__subcmd__unfollow_commands] )) ||
_bird__subcmd__unfollow_commands() {
    local commands; commands=()
    _describe -t commands 'bird unfollow commands' commands "$@"
}
(( $+functions[_bird__subcmd__unlike_commands] )) ||
_bird__subcmd__unlike_commands() {
    local commands; commands=()
    _describe -t commands 'bird unlike commands' commands "$@"
}
(( $+functions[_bird__subcmd__unmute_commands] )) ||
_bird__subcmd__unmute_commands() {
    local commands; commands=()
    _describe -t commands 'bird unmute commands' commands "$@"
}
(( $+functions[_bird__subcmd__unrepost_commands] )) ||
_bird__subcmd__unrepost_commands() {
    local commands; commands=()
    _describe -t commands 'bird unrepost commands' commands "$@"
}
(( $+functions[_bird__subcmd__watchlist_commands] )) ||
_bird__subcmd__watchlist_commands() {
    local commands; commands=(
'fetch:Fetch recent activity for all watched users' \
'add:Add a user to the watchlist' \
'remove:Remove a user from the watchlist' \
'list:Show the current watchlist' \
'help:Print this message or the help of the given subcommand(s)' \
    )
    _describe -t commands 'bird watchlist commands' commands "$@"
}
(( $+functions[_bird__subcmd__watchlist__subcmd__add_commands] )) ||
_bird__subcmd__watchlist__subcmd__add_commands() {
    local commands; commands=()
    _describe -t commands 'bird watchlist add commands' commands "$@"
}
(( $+functions[_bird__subcmd__watchlist__subcmd__fetch_commands] )) ||
_bird__subcmd__watchlist__subcmd__fetch_commands() {
    local commands; commands=()
    _describe -t commands 'bird watchlist fetch commands' commands "$@"
}
(( $+functions[_bird__subcmd__watchlist__subcmd__help_commands] )) ||
_bird__subcmd__watchlist__subcmd__help_commands() {
    local commands; commands=(
'fetch:Fetch recent activity for all watched users' \
'add:Add a user to the watchlist' \
'remove:Remove a user from the watchlist' \
'list:Show the current watchlist' \
'help:Print this message or the help of the given subcommand(s)' \
    )
    _describe -t commands 'bird watchlist help commands' commands "$@"
}
(( $+functions[_bird__subcmd__watchlist__subcmd__help__subcmd__add_commands] )) ||
_bird__subcmd__watchlist__subcmd__help__subcmd__add_commands() {
    local commands; commands=()
    _describe -t commands 'bird watchlist help add commands' commands "$@"
}
(( $+functions[_bird__subcmd__watchlist__subcmd__help__subcmd__fetch_commands] )) ||
_bird__subcmd__watchlist__subcmd__help__subcmd__fetch_commands() {
    local commands; commands=()
    _describe -t commands 'bird watchlist help fetch commands' commands "$@"
}
(( $+functions[_bird__subcmd__watchlist__subcmd__help__subcmd__help_commands] )) ||
_bird__subcmd__watchlist__subcmd__help__subcmd__help_commands() {
    local commands; commands=()
    _describe -t commands 'bird watchlist help help commands' commands "$@"
}
(( $+functions[_bird__subcmd__watchlist__subcmd__help__subcmd__list_commands] )) ||
_bird__subcmd__watchlist__subcmd__help__subcmd__list_commands() {
    local commands; commands=()
    _describe -t commands 'bird watchlist help list commands' commands "$@"
}
(( $+functions[_bird__subcmd__watchlist__subcmd__help__subcmd__remove_commands] )) ||
_bird__subcmd__watchlist__subcmd__help__subcmd__remove_commands() {
    local commands; commands=()
    _describe -t commands 'bird watchlist help remove commands' commands "$@"
}
(( $+functions[_bird__subcmd__watchlist__subcmd__list_commands] )) ||
_bird__subcmd__watchlist__subcmd__list_commands() {
    local commands; commands=()
    _describe -t commands 'bird watchlist list commands' commands "$@"
}
(( $+functions[_bird__subcmd__watchlist__subcmd__remove_commands] )) ||
_bird__subcmd__watchlist__subcmd__remove_commands() {
    local commands; commands=()
    _describe -t commands 'bird watchlist remove commands' commands "$@"
}

if [ "$funcstack[1]" = "_bird" ]; then
    _bird "$@"
else
    compdef _bird bird
fi
