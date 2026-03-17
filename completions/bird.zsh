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
'-u+[Username for multi-user token selection (maps to xurl -u)]:USERNAME:_default' \
'--username=[Username for multi-user token selection (maps to xurl -u)]:USERNAME:_default' \
'--output=[Error output format\: text (default for TTY), json (default for non-TTY)]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON, no color"))' \
'--plain[Plain output (no color, no hyperlinks; script-friendly)]' \
'--no-color[Disable ANSI colors (or set NO_COLOR)]' \
'--refresh[Bypass store read, still write response to store]' \
'--no-cache[Disable entity store entirely (no read, no write)]' \
'--cache-only[Only serve from local store; never make API requests]' \
'-q[Suppress informational stderr output (keep only fatal errors)]' \
'--quiet[Suppress informational stderr output (keep only fatal errors)]' \
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
'-u+[Username for multi-user token selection (maps to xurl -u)]:USERNAME:_default' \
'--username=[Username for multi-user token selection (maps to xurl -u)]:USERNAME:_default' \
'--output=[Error output format\: text (default for TTY), json (default for non-TTY)]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON, no color"))' \
'--plain[Plain output (no color, no hyperlinks; script-friendly)]' \
'--no-color[Disable ANSI colors (or set NO_COLOR)]' \
'--refresh[Bypass store read, still write response to store]' \
'--no-cache[Disable entity store entirely (no read, no write)]' \
'--cache-only[Only serve from local store; never make API requests]' \
'-q[Suppress informational stderr output (keep only fatal errors)]' \
'--quiet[Suppress informational stderr output (keep only fatal errors)]' \
'-h[Print help (see more with '\''--help'\'')]' \
'--help[Print help (see more with '\''--help'\'')]' \
&& ret=0
;;
(me)
_arguments "${_arguments_options[@]}" : \
'-u+[Username for multi-user token selection (maps to xurl -u)]:USERNAME:_default' \
'--username=[Username for multi-user token selection (maps to xurl -u)]:USERNAME:_default' \
'--output=[Error output format\: text (default for TTY), json (default for non-TTY)]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON, no color"))' \
'--pretty[Human-readable output]' \
'--plain[Plain output (no color, no hyperlinks; script-friendly)]' \
'--no-color[Disable ANSI colors (or set NO_COLOR)]' \
'--refresh[Bypass store read, still write response to store]' \
'--no-cache[Disable entity store entirely (no read, no write)]' \
'--cache-only[Only serve from local store; never make API requests]' \
'-q[Suppress informational stderr output (keep only fatal errors)]' \
'--quiet[Suppress informational stderr output (keep only fatal errors)]' \
'-h[Print help (see more with '\''--help'\'')]' \
'--help[Print help (see more with '\''--help'\'')]' \
&& ret=0
;;
(get)
_arguments "${_arguments_options[@]}" : \
'*-p+[]:KEY=VALUE:_default' \
'*--param=[]:KEY=VALUE:_default' \
'*--query=[]:KEY=VALUE:_default' \
'-u+[Username for multi-user token selection (maps to xurl -u)]:USERNAME:_default' \
'--username=[Username for multi-user token selection (maps to xurl -u)]:USERNAME:_default' \
'--output=[Error output format\: text (default for TTY), json (default for non-TTY)]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON, no color"))' \
'--pretty[]' \
'--plain[Plain output (no color, no hyperlinks; script-friendly)]' \
'--no-color[Disable ANSI colors (or set NO_COLOR)]' \
'--refresh[Bypass store read, still write response to store]' \
'--no-cache[Disable entity store entirely (no read, no write)]' \
'--cache-only[Only serve from local store; never make API requests]' \
'-q[Suppress informational stderr output (keep only fatal errors)]' \
'--quiet[Suppress informational stderr output (keep only fatal errors)]' \
'-h[Print help (see more with '\''--help'\'')]' \
'--help[Print help (see more with '\''--help'\'')]' \
':path:_default' \
&& ret=0
;;
(post)
_arguments "${_arguments_options[@]}" : \
'*-p+[]:KEY=VALUE:_default' \
'*--param=[]:KEY=VALUE:_default' \
'*--query=[]:KEY=VALUE:_default' \
'--body=[]:JSON:_default' \
'-u+[Username for multi-user token selection (maps to xurl -u)]:USERNAME:_default' \
'--username=[Username for multi-user token selection (maps to xurl -u)]:USERNAME:_default' \
'--output=[Error output format\: text (default for TTY), json (default for non-TTY)]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON, no color"))' \
'--pretty[]' \
'--plain[Plain output (no color, no hyperlinks; script-friendly)]' \
'--no-color[Disable ANSI colors (or set NO_COLOR)]' \
'--refresh[Bypass store read, still write response to store]' \
'--no-cache[Disable entity store entirely (no read, no write)]' \
'--cache-only[Only serve from local store; never make API requests]' \
'-q[Suppress informational stderr output (keep only fatal errors)]' \
'--quiet[Suppress informational stderr output (keep only fatal errors)]' \
'-h[Print help (see more with '\''--help'\'')]' \
'--help[Print help (see more with '\''--help'\'')]' \
':path:_default' \
&& ret=0
;;
(put)
_arguments "${_arguments_options[@]}" : \
'*-p+[]:KEY=VALUE:_default' \
'*--param=[]:KEY=VALUE:_default' \
'*--query=[]:KEY=VALUE:_default' \
'--body=[]:JSON:_default' \
'-u+[Username for multi-user token selection (maps to xurl -u)]:USERNAME:_default' \
'--username=[Username for multi-user token selection (maps to xurl -u)]:USERNAME:_default' \
'--output=[Error output format\: text (default for TTY), json (default for non-TTY)]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON, no color"))' \
'--pretty[]' \
'--plain[Plain output (no color, no hyperlinks; script-friendly)]' \
'--no-color[Disable ANSI colors (or set NO_COLOR)]' \
'--refresh[Bypass store read, still write response to store]' \
'--no-cache[Disable entity store entirely (no read, no write)]' \
'--cache-only[Only serve from local store; never make API requests]' \
'-q[Suppress informational stderr output (keep only fatal errors)]' \
'--quiet[Suppress informational stderr output (keep only fatal errors)]' \
'-h[Print help (see more with '\''--help'\'')]' \
'--help[Print help (see more with '\''--help'\'')]' \
':path:_default' \
&& ret=0
;;
(bookmarks)
_arguments "${_arguments_options[@]}" : \
'-u+[Username for multi-user token selection (maps to xurl -u)]:USERNAME:_default' \
'--username=[Username for multi-user token selection (maps to xurl -u)]:USERNAME:_default' \
'--output=[Error output format\: text (default for TTY), json (default for non-TTY)]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON, no color"))' \
'--pretty[]' \
'--plain[Plain output (no color, no hyperlinks; script-friendly)]' \
'--no-color[Disable ANSI colors (or set NO_COLOR)]' \
'--refresh[Bypass store read, still write response to store]' \
'--no-cache[Disable entity store entirely (no read, no write)]' \
'--cache-only[Only serve from local store; never make API requests]' \
'-q[Suppress informational stderr output (keep only fatal errors)]' \
'--quiet[Suppress informational stderr output (keep only fatal errors)]' \
'-h[Print help (see more with '\''--help'\'')]' \
'--help[Print help (see more with '\''--help'\'')]' \
&& ret=0
;;
(profile)
_arguments "${_arguments_options[@]}" : \
'--output=[Error output format\: text (default for TTY), json (default for non-TTY)]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON, no color"))' \
'--pretty[Pretty-print JSON output]' \
'--plain[Plain output (no color, no hyperlinks; script-friendly)]' \
'--no-color[Disable ANSI colors (or set NO_COLOR)]' \
'--refresh[Bypass store read, still write response to store]' \
'--no-cache[Disable entity store entirely (no read, no write)]' \
'--cache-only[Only serve from local store; never make API requests]' \
'-q[Suppress informational stderr output (keep only fatal errors)]' \
'--quiet[Suppress informational stderr output (keep only fatal errors)]' \
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
'-u+[Username for multi-user token selection (maps to xurl -u)]:USERNAME:_default' \
'--username=[Username for multi-user token selection (maps to xurl -u)]:USERNAME:_default' \
'--output=[Error output format\: text (default for TTY), json (default for non-TTY)]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON, no color"))' \
'--pretty[Pretty-print JSON output]' \
'--plain[Plain output (no color, no hyperlinks; script-friendly)]' \
'--no-color[Disable ANSI colors (or set NO_COLOR)]' \
'--refresh[Bypass store read, still write response to store]' \
'--no-cache[Disable entity store entirely (no read, no write)]' \
'--cache-only[Only serve from local store; never make API requests]' \
'-q[Suppress informational stderr output (keep only fatal errors)]' \
'--quiet[Suppress informational stderr output (keep only fatal errors)]' \
'-h[Print help (see more with '\''--help'\'')]' \
'--help[Print help (see more with '\''--help'\'')]' \
':query -- Search query (X API search syntax):_default' \
&& ret=0
;;
(thread)
_arguments "${_arguments_options[@]}" : \
'--max-pages=[Maximum number of search result pages (default\: 10, max\: 25)]:MAX_PAGES:_default' \
'-u+[Username for multi-user token selection (maps to xurl -u)]:USERNAME:_default' \
'--username=[Username for multi-user token selection (maps to xurl -u)]:USERNAME:_default' \
'--output=[Error output format\: text (default for TTY), json (default for non-TTY)]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON, no color"))' \
'--pretty[Pretty-print JSON output]' \
'--plain[Plain output (no color, no hyperlinks; script-friendly)]' \
'--no-color[Disable ANSI colors (or set NO_COLOR)]' \
'--refresh[Bypass store read, still write response to store]' \
'--no-cache[Disable entity store entirely (no read, no write)]' \
'--cache-only[Only serve from local store; never make API requests]' \
'-q[Suppress informational stderr output (keep only fatal errors)]' \
'--quiet[Suppress informational stderr output (keep only fatal errors)]' \
'-h[Print help (see more with '\''--help'\'')]' \
'--help[Print help (see more with '\''--help'\'')]' \
':tweet_id -- Tweet ID (root tweet or any reply in the thread):_default' \
&& ret=0
;;
(delete)
_arguments "${_arguments_options[@]}" : \
'*-p+[]:KEY=VALUE:_default' \
'*--param=[]:KEY=VALUE:_default' \
'*--query=[]:KEY=VALUE:_default' \
'-u+[Username for multi-user token selection (maps to xurl -u)]:USERNAME:_default' \
'--username=[Username for multi-user token selection (maps to xurl -u)]:USERNAME:_default' \
'--output=[Error output format\: text (default for TTY), json (default for non-TTY)]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON, no color"))' \
'--pretty[]' \
'--plain[Plain output (no color, no hyperlinks; script-friendly)]' \
'--no-color[Disable ANSI colors (or set NO_COLOR)]' \
'--refresh[Bypass store read, still write response to store]' \
'--no-cache[Disable entity store entirely (no read, no write)]' \
'--cache-only[Only serve from local store; never make API requests]' \
'-q[Suppress informational stderr output (keep only fatal errors)]' \
'--quiet[Suppress informational stderr output (keep only fatal errors)]' \
'-h[Print help (see more with '\''--help'\'')]' \
'--help[Print help (see more with '\''--help'\'')]' \
':path:_default' \
&& ret=0
;;
(watchlist)
_arguments "${_arguments_options[@]}" : \
'-u+[Username for multi-user token selection (maps to xurl -u)]:USERNAME:_default' \
'--username=[Username for multi-user token selection (maps to xurl -u)]:USERNAME:_default' \
'--output=[Error output format\: text (default for TTY), json (default for non-TTY)]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON, no color"))' \
'--pretty[Pretty-print JSON output]' \
'--plain[Plain output (no color, no hyperlinks; script-friendly)]' \
'--no-color[Disable ANSI colors (or set NO_COLOR)]' \
'--refresh[Bypass store read, still write response to store]' \
'--no-cache[Disable entity store entirely (no read, no write)]' \
'--cache-only[Only serve from local store; never make API requests]' \
'-q[Suppress informational stderr output (keep only fatal errors)]' \
'--quiet[Suppress informational stderr output (keep only fatal errors)]' \
'-h[Print help (see more with '\''--help'\'')]' \
'--help[Print help (see more with '\''--help'\'')]' \
":: :_bird__watchlist_commands" \
"*::: :->watchlist" \
&& ret=0

    case $state in
    (watchlist)
        words=($line[1] "${words[@]}")
        (( CURRENT += 1 ))
        curcontext="${curcontext%:*:*}:bird-watchlist-command-$line[1]:"
        case $line[1] in
            (check)
_arguments "${_arguments_options[@]}" : \
'-u+[Username for multi-user token selection (maps to xurl -u)]:USERNAME:_default' \
'--username=[Username for multi-user token selection (maps to xurl -u)]:USERNAME:_default' \
'--output=[Error output format\: text (default for TTY), json (default for non-TTY)]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON, no color"))' \
'--plain[Plain output (no color, no hyperlinks; script-friendly)]' \
'--no-color[Disable ANSI colors (or set NO_COLOR)]' \
'--refresh[Bypass store read, still write response to store]' \
'--no-cache[Disable entity store entirely (no read, no write)]' \
'--cache-only[Only serve from local store; never make API requests]' \
'-q[Suppress informational stderr output (keep only fatal errors)]' \
'--quiet[Suppress informational stderr output (keep only fatal errors)]' \
'-h[Print help (see more with '\''--help'\'')]' \
'--help[Print help (see more with '\''--help'\'')]' \
&& ret=0
;;
(add)
_arguments "${_arguments_options[@]}" : \
'--output=[Error output format\: text (default for TTY), json (default for non-TTY)]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON, no color"))' \
'--plain[Plain output (no color, no hyperlinks; script-friendly)]' \
'--no-color[Disable ANSI colors (or set NO_COLOR)]' \
'--refresh[Bypass store read, still write response to store]' \
'--no-cache[Disable entity store entirely (no read, no write)]' \
'--cache-only[Only serve from local store; never make API requests]' \
'-q[Suppress informational stderr output (keep only fatal errors)]' \
'--quiet[Suppress informational stderr output (keep only fatal errors)]' \
'-h[Print help (see more with '\''--help'\'')]' \
'--help[Print help (see more with '\''--help'\'')]' \
':username -- X/Twitter username (with or without @):_default' \
&& ret=0
;;
(remove)
_arguments "${_arguments_options[@]}" : \
'--output=[Error output format\: text (default for TTY), json (default for non-TTY)]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON, no color"))' \
'--plain[Plain output (no color, no hyperlinks; script-friendly)]' \
'--no-color[Disable ANSI colors (or set NO_COLOR)]' \
'--refresh[Bypass store read, still write response to store]' \
'--no-cache[Disable entity store entirely (no read, no write)]' \
'--cache-only[Only serve from local store; never make API requests]' \
'-q[Suppress informational stderr output (keep only fatal errors)]' \
'--quiet[Suppress informational stderr output (keep only fatal errors)]' \
'-h[Print help (see more with '\''--help'\'')]' \
'--help[Print help (see more with '\''--help'\'')]' \
':username -- X/Twitter username to remove:_default' \
&& ret=0
;;
(list)
_arguments "${_arguments_options[@]}" : \
'-u+[Username for multi-user token selection (maps to xurl -u)]:USERNAME:_default' \
'--username=[Username for multi-user token selection (maps to xurl -u)]:USERNAME:_default' \
'--output=[Error output format\: text (default for TTY), json (default for non-TTY)]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON, no color"))' \
'--plain[Plain output (no color, no hyperlinks; script-friendly)]' \
'--no-color[Disable ANSI colors (or set NO_COLOR)]' \
'--refresh[Bypass store read, still write response to store]' \
'--no-cache[Disable entity store entirely (no read, no write)]' \
'--cache-only[Only serve from local store; never make API requests]' \
'-q[Suppress informational stderr output (keep only fatal errors)]' \
'--quiet[Suppress informational stderr output (keep only fatal errors)]' \
'-h[Print help (see more with '\''--help'\'')]' \
'--help[Print help (see more with '\''--help'\'')]' \
&& ret=0
;;
(help)
_arguments "${_arguments_options[@]}" : \
":: :_bird__watchlist__help_commands" \
"*::: :->help" \
&& ret=0

    case $state in
    (help)
        words=($line[1] "${words[@]}")
        (( CURRENT += 1 ))
        curcontext="${curcontext%:*:*}:bird-watchlist-help-command-$line[1]:"
        case $line[1] in
            (check)
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
(usage)
_arguments "${_arguments_options[@]}" : \
'--since=[Show usage since this date (YYYY-MM-DD; default\: 30 days ago)]:SINCE:_default' \
'-u+[Username for multi-user token selection (maps to xurl -u)]:USERNAME:_default' \
'--username=[Username for multi-user token selection (maps to xurl -u)]:USERNAME:_default' \
'--output=[Error output format\: text (default for TTY), json (default for non-TTY)]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON, no color"))' \
'--sync[Sync actual usage from X API (requires Bearer token via xurl)]' \
'--pretty[Pretty-print output]' \
'--plain[Plain output (no color, no hyperlinks; script-friendly)]' \
'--no-color[Disable ANSI colors (or set NO_COLOR)]' \
'--refresh[Bypass store read, still write response to store]' \
'--no-cache[Disable entity store entirely (no read, no write)]' \
'--cache-only[Only serve from local store; never make API requests]' \
'-q[Suppress informational stderr output (keep only fatal errors)]' \
'--quiet[Suppress informational stderr output (keep only fatal errors)]' \
'-h[Print help (see more with '\''--help'\'')]' \
'--help[Print help (see more with '\''--help'\'')]' \
&& ret=0
;;
(tweet)
_arguments "${_arguments_options[@]}" : \
'--media-id=[Media ID to attach]:MEDIA_ID:_default' \
'-u+[Username for multi-user token selection (maps to xurl -u)]:USERNAME:_default' \
'--username=[Username for multi-user token selection (maps to xurl -u)]:USERNAME:_default' \
'--output=[Error output format\: text (default for TTY), json (default for non-TTY)]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON, no color"))' \
'--plain[Plain output (no color, no hyperlinks; script-friendly)]' \
'--no-color[Disable ANSI colors (or set NO_COLOR)]' \
'--refresh[Bypass store read, still write response to store]' \
'--no-cache[Disable entity store entirely (no read, no write)]' \
'--cache-only[Only serve from local store; never make API requests]' \
'-q[Suppress informational stderr output (keep only fatal errors)]' \
'--quiet[Suppress informational stderr output (keep only fatal errors)]' \
'-h[Print help (see more with '\''--help'\'')]' \
'--help[Print help (see more with '\''--help'\'')]' \
':text -- Tweet text:_default' \
&& ret=0
;;
(reply)
_arguments "${_arguments_options[@]}" : \
'-u+[Username for multi-user token selection (maps to xurl -u)]:USERNAME:_default' \
'--username=[Username for multi-user token selection (maps to xurl -u)]:USERNAME:_default' \
'--output=[Error output format\: text (default for TTY), json (default for non-TTY)]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON, no color"))' \
'--plain[Plain output (no color, no hyperlinks; script-friendly)]' \
'--no-color[Disable ANSI colors (or set NO_COLOR)]' \
'--refresh[Bypass store read, still write response to store]' \
'--no-cache[Disable entity store entirely (no read, no write)]' \
'--cache-only[Only serve from local store; never make API requests]' \
'-q[Suppress informational stderr output (keep only fatal errors)]' \
'--quiet[Suppress informational stderr output (keep only fatal errors)]' \
'-h[Print help (see more with '\''--help'\'')]' \
'--help[Print help (see more with '\''--help'\'')]' \
':tweet_id -- Tweet ID to reply to:_default' \
':text -- Reply text:_default' \
&& ret=0
;;
(like)
_arguments "${_arguments_options[@]}" : \
'-u+[Username for multi-user token selection (maps to xurl -u)]:USERNAME:_default' \
'--username=[Username for multi-user token selection (maps to xurl -u)]:USERNAME:_default' \
'--output=[Error output format\: text (default for TTY), json (default for non-TTY)]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON, no color"))' \
'--plain[Plain output (no color, no hyperlinks; script-friendly)]' \
'--no-color[Disable ANSI colors (or set NO_COLOR)]' \
'--refresh[Bypass store read, still write response to store]' \
'--no-cache[Disable entity store entirely (no read, no write)]' \
'--cache-only[Only serve from local store; never make API requests]' \
'-q[Suppress informational stderr output (keep only fatal errors)]' \
'--quiet[Suppress informational stderr output (keep only fatal errors)]' \
'-h[Print help (see more with '\''--help'\'')]' \
'--help[Print help (see more with '\''--help'\'')]' \
':tweet_id -- Tweet ID to like:_default' \
&& ret=0
;;
(unlike)
_arguments "${_arguments_options[@]}" : \
'-u+[Username for multi-user token selection (maps to xurl -u)]:USERNAME:_default' \
'--username=[Username for multi-user token selection (maps to xurl -u)]:USERNAME:_default' \
'--output=[Error output format\: text (default for TTY), json (default for non-TTY)]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON, no color"))' \
'--plain[Plain output (no color, no hyperlinks; script-friendly)]' \
'--no-color[Disable ANSI colors (or set NO_COLOR)]' \
'--refresh[Bypass store read, still write response to store]' \
'--no-cache[Disable entity store entirely (no read, no write)]' \
'--cache-only[Only serve from local store; never make API requests]' \
'-q[Suppress informational stderr output (keep only fatal errors)]' \
'--quiet[Suppress informational stderr output (keep only fatal errors)]' \
'-h[Print help (see more with '\''--help'\'')]' \
'--help[Print help (see more with '\''--help'\'')]' \
':tweet_id -- Tweet ID to unlike:_default' \
&& ret=0
;;
(repost)
_arguments "${_arguments_options[@]}" : \
'-u+[Username for multi-user token selection (maps to xurl -u)]:USERNAME:_default' \
'--username=[Username for multi-user token selection (maps to xurl -u)]:USERNAME:_default' \
'--output=[Error output format\: text (default for TTY), json (default for non-TTY)]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON, no color"))' \
'--plain[Plain output (no color, no hyperlinks; script-friendly)]' \
'--no-color[Disable ANSI colors (or set NO_COLOR)]' \
'--refresh[Bypass store read, still write response to store]' \
'--no-cache[Disable entity store entirely (no read, no write)]' \
'--cache-only[Only serve from local store; never make API requests]' \
'-q[Suppress informational stderr output (keep only fatal errors)]' \
'--quiet[Suppress informational stderr output (keep only fatal errors)]' \
'-h[Print help (see more with '\''--help'\'')]' \
'--help[Print help (see more with '\''--help'\'')]' \
':tweet_id -- Tweet ID to repost:_default' \
&& ret=0
;;
(unrepost)
_arguments "${_arguments_options[@]}" : \
'-u+[Username for multi-user token selection (maps to xurl -u)]:USERNAME:_default' \
'--username=[Username for multi-user token selection (maps to xurl -u)]:USERNAME:_default' \
'--output=[Error output format\: text (default for TTY), json (default for non-TTY)]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON, no color"))' \
'--plain[Plain output (no color, no hyperlinks; script-friendly)]' \
'--no-color[Disable ANSI colors (or set NO_COLOR)]' \
'--refresh[Bypass store read, still write response to store]' \
'--no-cache[Disable entity store entirely (no read, no write)]' \
'--cache-only[Only serve from local store; never make API requests]' \
'-q[Suppress informational stderr output (keep only fatal errors)]' \
'--quiet[Suppress informational stderr output (keep only fatal errors)]' \
'-h[Print help (see more with '\''--help'\'')]' \
'--help[Print help (see more with '\''--help'\'')]' \
':tweet_id -- Tweet ID to unrepost:_default' \
&& ret=0
;;
(follow)
_arguments "${_arguments_options[@]}" : \
'--output=[Error output format\: text (default for TTY), json (default for non-TTY)]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON, no color"))' \
'--plain[Plain output (no color, no hyperlinks; script-friendly)]' \
'--no-color[Disable ANSI colors (or set NO_COLOR)]' \
'--refresh[Bypass store read, still write response to store]' \
'--no-cache[Disable entity store entirely (no read, no write)]' \
'--cache-only[Only serve from local store; never make API requests]' \
'-q[Suppress informational stderr output (keep only fatal errors)]' \
'--quiet[Suppress informational stderr output (keep only fatal errors)]' \
'-h[Print help (see more with '\''--help'\'')]' \
'--help[Print help (see more with '\''--help'\'')]' \
':username -- Username to follow:_default' \
&& ret=0
;;
(unfollow)
_arguments "${_arguments_options[@]}" : \
'--output=[Error output format\: text (default for TTY), json (default for non-TTY)]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON, no color"))' \
'--plain[Plain output (no color, no hyperlinks; script-friendly)]' \
'--no-color[Disable ANSI colors (or set NO_COLOR)]' \
'--refresh[Bypass store read, still write response to store]' \
'--no-cache[Disable entity store entirely (no read, no write)]' \
'--cache-only[Only serve from local store; never make API requests]' \
'-q[Suppress informational stderr output (keep only fatal errors)]' \
'--quiet[Suppress informational stderr output (keep only fatal errors)]' \
'-h[Print help (see more with '\''--help'\'')]' \
'--help[Print help (see more with '\''--help'\'')]' \
':username -- Username to unfollow:_default' \
&& ret=0
;;
(dm)
_arguments "${_arguments_options[@]}" : \
'--output=[Error output format\: text (default for TTY), json (default for non-TTY)]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON, no color"))' \
'--plain[Plain output (no color, no hyperlinks; script-friendly)]' \
'--no-color[Disable ANSI colors (or set NO_COLOR)]' \
'--refresh[Bypass store read, still write response to store]' \
'--no-cache[Disable entity store entirely (no read, no write)]' \
'--cache-only[Only serve from local store; never make API requests]' \
'-q[Suppress informational stderr output (keep only fatal errors)]' \
'--quiet[Suppress informational stderr output (keep only fatal errors)]' \
'-h[Print help (see more with '\''--help'\'')]' \
'--help[Print help (see more with '\''--help'\'')]' \
':username -- Username to message:_default' \
':text -- Message text:_default' \
&& ret=0
;;
(block)
_arguments "${_arguments_options[@]}" : \
'--output=[Error output format\: text (default for TTY), json (default for non-TTY)]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON, no color"))' \
'--plain[Plain output (no color, no hyperlinks; script-friendly)]' \
'--no-color[Disable ANSI colors (or set NO_COLOR)]' \
'--refresh[Bypass store read, still write response to store]' \
'--no-cache[Disable entity store entirely (no read, no write)]' \
'--cache-only[Only serve from local store; never make API requests]' \
'-q[Suppress informational stderr output (keep only fatal errors)]' \
'--quiet[Suppress informational stderr output (keep only fatal errors)]' \
'-h[Print help (see more with '\''--help'\'')]' \
'--help[Print help (see more with '\''--help'\'')]' \
':username -- Username to block:_default' \
&& ret=0
;;
(unblock)
_arguments "${_arguments_options[@]}" : \
'--output=[Error output format\: text (default for TTY), json (default for non-TTY)]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON, no color"))' \
'--plain[Plain output (no color, no hyperlinks; script-friendly)]' \
'--no-color[Disable ANSI colors (or set NO_COLOR)]' \
'--refresh[Bypass store read, still write response to store]' \
'--no-cache[Disable entity store entirely (no read, no write)]' \
'--cache-only[Only serve from local store; never make API requests]' \
'-q[Suppress informational stderr output (keep only fatal errors)]' \
'--quiet[Suppress informational stderr output (keep only fatal errors)]' \
'-h[Print help (see more with '\''--help'\'')]' \
'--help[Print help (see more with '\''--help'\'')]' \
':username -- Username to unblock:_default' \
&& ret=0
;;
(mute)
_arguments "${_arguments_options[@]}" : \
'--output=[Error output format\: text (default for TTY), json (default for non-TTY)]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON, no color"))' \
'--plain[Plain output (no color, no hyperlinks; script-friendly)]' \
'--no-color[Disable ANSI colors (or set NO_COLOR)]' \
'--refresh[Bypass store read, still write response to store]' \
'--no-cache[Disable entity store entirely (no read, no write)]' \
'--cache-only[Only serve from local store; never make API requests]' \
'-q[Suppress informational stderr output (keep only fatal errors)]' \
'--quiet[Suppress informational stderr output (keep only fatal errors)]' \
'-h[Print help (see more with '\''--help'\'')]' \
'--help[Print help (see more with '\''--help'\'')]' \
':username -- Username to mute:_default' \
&& ret=0
;;
(unmute)
_arguments "${_arguments_options[@]}" : \
'--output=[Error output format\: text (default for TTY), json (default for non-TTY)]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON, no color"))' \
'--plain[Plain output (no color, no hyperlinks; script-friendly)]' \
'--no-color[Disable ANSI colors (or set NO_COLOR)]' \
'--refresh[Bypass store read, still write response to store]' \
'--no-cache[Disable entity store entirely (no read, no write)]' \
'--cache-only[Only serve from local store; never make API requests]' \
'-q[Suppress informational stderr output (keep only fatal errors)]' \
'--quiet[Suppress informational stderr output (keep only fatal errors)]' \
'-h[Print help (see more with '\''--help'\'')]' \
'--help[Print help (see more with '\''--help'\'')]' \
':username -- Username to unmute:_default' \
&& ret=0
;;
(doctor)
_arguments "${_arguments_options[@]}" : \
'-u+[Username for multi-user token selection (maps to xurl -u)]:USERNAME:_default' \
'--username=[Username for multi-user token selection (maps to xurl -u)]:USERNAME:_default' \
'--output=[Error output format\: text (default for TTY), json (default for non-TTY)]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON, no color"))' \
'--pretty[]' \
'--plain[Plain output (no color, no hyperlinks; script-friendly)]' \
'--no-color[Disable ANSI colors (or set NO_COLOR)]' \
'--refresh[Bypass store read, still write response to store]' \
'--no-cache[Disable entity store entirely (no read, no write)]' \
'--cache-only[Only serve from local store; never make API requests]' \
'-q[Suppress informational stderr output (keep only fatal errors)]' \
'--quiet[Suppress informational stderr output (keep only fatal errors)]' \
'-h[Print help (see more with '\''--help'\'')]' \
'--help[Print help (see more with '\''--help'\'')]' \
'::command -- Scope report to this command only (e.g. me, bookmarks, get):_default' \
&& ret=0
;;
(cache)
_arguments "${_arguments_options[@]}" : \
'-u+[Username for multi-user token selection (maps to xurl -u)]:USERNAME:_default' \
'--username=[Username for multi-user token selection (maps to xurl -u)]:USERNAME:_default' \
'--output=[Error output format\: text (default for TTY), json (default for non-TTY)]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON, no color"))' \
'--plain[Plain output (no color, no hyperlinks; script-friendly)]' \
'--no-color[Disable ANSI colors (or set NO_COLOR)]' \
'--refresh[Bypass store read, still write response to store]' \
'--no-cache[Disable entity store entirely (no read, no write)]' \
'--cache-only[Only serve from local store; never make API requests]' \
'-q[Suppress informational stderr output (keep only fatal errors)]' \
'--quiet[Suppress informational stderr output (keep only fatal errors)]' \
'-h[Print help (see more with '\''--help'\'')]' \
'--help[Print help (see more with '\''--help'\'')]' \
":: :_bird__cache_commands" \
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
'-u+[Username for multi-user token selection (maps to xurl -u)]:USERNAME:_default' \
'--username=[Username for multi-user token selection (maps to xurl -u)]:USERNAME:_default' \
'--output=[Error output format\: text (default for TTY), json (default for non-TTY)]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON, no color"))' \
'--plain[Plain output (no color, no hyperlinks; script-friendly)]' \
'--no-color[Disable ANSI colors (or set NO_COLOR)]' \
'--refresh[Bypass store read, still write response to store]' \
'--no-cache[Disable entity store entirely (no read, no write)]' \
'--cache-only[Only serve from local store; never make API requests]' \
'-q[Suppress informational stderr output (keep only fatal errors)]' \
'--quiet[Suppress informational stderr output (keep only fatal errors)]' \
'-h[Print help (see more with '\''--help'\'')]' \
'--help[Print help (see more with '\''--help'\'')]' \
&& ret=0
;;
(stats)
_arguments "${_arguments_options[@]}" : \
'-u+[Username for multi-user token selection (maps to xurl -u)]:USERNAME:_default' \
'--username=[Username for multi-user token selection (maps to xurl -u)]:USERNAME:_default' \
'--output=[Error output format\: text (default for TTY), json (default for non-TTY)]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON, no color"))' \
'--pretty[]' \
'--plain[Plain output (no color, no hyperlinks; script-friendly)]' \
'--no-color[Disable ANSI colors (or set NO_COLOR)]' \
'--refresh[Bypass store read, still write response to store]' \
'--no-cache[Disable entity store entirely (no read, no write)]' \
'--cache-only[Only serve from local store; never make API requests]' \
'-q[Suppress informational stderr output (keep only fatal errors)]' \
'--quiet[Suppress informational stderr output (keep only fatal errors)]' \
'-h[Print help (see more with '\''--help'\'')]' \
'--help[Print help (see more with '\''--help'\'')]' \
&& ret=0
;;
(help)
_arguments "${_arguments_options[@]}" : \
":: :_bird__cache__help_commands" \
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
'-u+[Username for multi-user token selection (maps to xurl -u)]:USERNAME:_default' \
'--username=[Username for multi-user token selection (maps to xurl -u)]:USERNAME:_default' \
'--output=[Error output format\: text (default for TTY), json (default for non-TTY)]:OUTPUT:((text\:"Default\: colored, human-readable"
json\:"Machine-readable JSON, no color"))' \
'--plain[Plain output (no color, no hyperlinks; script-friendly)]' \
'--no-color[Disable ANSI colors (or set NO_COLOR)]' \
'--refresh[Bypass store read, still write response to store]' \
'--no-cache[Disable entity store entirely (no read, no write)]' \
'--cache-only[Only serve from local store; never make API requests]' \
'-q[Suppress informational stderr output (keep only fatal errors)]' \
'--quiet[Suppress informational stderr output (keep only fatal errors)]' \
'-h[Print help (see more with '\''--help'\'')]' \
'--help[Print help (see more with '\''--help'\'')]' \
':shell -- Shell to generate completions for:(bash elvish fish powershell zsh)' \
&& ret=0
;;
(help)
_arguments "${_arguments_options[@]}" : \
":: :_bird__help_commands" \
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
(get)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
(post)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
(put)
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
(delete)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
(watchlist)
_arguments "${_arguments_options[@]}" : \
":: :_bird__help__watchlist_commands" \
"*::: :->watchlist" \
&& ret=0

    case $state in
    (watchlist)
        words=($line[1] "${words[@]}")
        (( CURRENT += 1 ))
        curcontext="${curcontext%:*:*}:bird-help-watchlist-command-$line[1]:"
        case $line[1] in
            (check)
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
(usage)
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
":: :_bird__help__cache_commands" \
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
'get:GET request to path (e.g. /2/users/me or /2/users/{id}/bookmarks with -p id=123)' \
'post:POST request to path' \
'put:PUT request to path' \
'bookmarks:List bookmarks for the current user (paginated, max_results=100)' \
'profile:Look up a user profile by username' \
'search:Search recent tweets (GET /2/tweets/search/recent)' \
'thread:Reconstruct a conversation thread from a tweet' \
'delete:DELETE request to path' \
'watchlist:Monitor users\: check recent activity, manage watchlist' \
'usage:View API usage and costs' \
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
'doctor:Show what is available\: xurl status, commands, and entity store health' \
'cache:Manage the HTTP response cache' \
'completions:Generate shell completions' \
'help:Print this message or the help of the given subcommand(s)' \
    )
    _describe -t commands 'bird commands' commands "$@"
}
(( $+functions[_bird__block_commands] )) ||
_bird__block_commands() {
    local commands; commands=()
    _describe -t commands 'bird block commands' commands "$@"
}
(( $+functions[_bird__bookmarks_commands] )) ||
_bird__bookmarks_commands() {
    local commands; commands=()
    _describe -t commands 'bird bookmarks commands' commands "$@"
}
(( $+functions[_bird__cache_commands] )) ||
_bird__cache_commands() {
    local commands; commands=(
'clear:Delete all cache entries' \
'stats:Show cache status (JSON default, --pretty for human-readable)' \
'help:Print this message or the help of the given subcommand(s)' \
    )
    _describe -t commands 'bird cache commands' commands "$@"
}
(( $+functions[_bird__cache__clear_commands] )) ||
_bird__cache__clear_commands() {
    local commands; commands=()
    _describe -t commands 'bird cache clear commands' commands "$@"
}
(( $+functions[_bird__cache__help_commands] )) ||
_bird__cache__help_commands() {
    local commands; commands=(
'clear:Delete all cache entries' \
'stats:Show cache status (JSON default, --pretty for human-readable)' \
'help:Print this message or the help of the given subcommand(s)' \
    )
    _describe -t commands 'bird cache help commands' commands "$@"
}
(( $+functions[_bird__cache__help__clear_commands] )) ||
_bird__cache__help__clear_commands() {
    local commands; commands=()
    _describe -t commands 'bird cache help clear commands' commands "$@"
}
(( $+functions[_bird__cache__help__help_commands] )) ||
_bird__cache__help__help_commands() {
    local commands; commands=()
    _describe -t commands 'bird cache help help commands' commands "$@"
}
(( $+functions[_bird__cache__help__stats_commands] )) ||
_bird__cache__help__stats_commands() {
    local commands; commands=()
    _describe -t commands 'bird cache help stats commands' commands "$@"
}
(( $+functions[_bird__cache__stats_commands] )) ||
_bird__cache__stats_commands() {
    local commands; commands=()
    _describe -t commands 'bird cache stats commands' commands "$@"
}
(( $+functions[_bird__completions_commands] )) ||
_bird__completions_commands() {
    local commands; commands=()
    _describe -t commands 'bird completions commands' commands "$@"
}
(( $+functions[_bird__delete_commands] )) ||
_bird__delete_commands() {
    local commands; commands=()
    _describe -t commands 'bird delete commands' commands "$@"
}
(( $+functions[_bird__dm_commands] )) ||
_bird__dm_commands() {
    local commands; commands=()
    _describe -t commands 'bird dm commands' commands "$@"
}
(( $+functions[_bird__doctor_commands] )) ||
_bird__doctor_commands() {
    local commands; commands=()
    _describe -t commands 'bird doctor commands' commands "$@"
}
(( $+functions[_bird__follow_commands] )) ||
_bird__follow_commands() {
    local commands; commands=()
    _describe -t commands 'bird follow commands' commands "$@"
}
(( $+functions[_bird__get_commands] )) ||
_bird__get_commands() {
    local commands; commands=()
    _describe -t commands 'bird get commands' commands "$@"
}
(( $+functions[_bird__help_commands] )) ||
_bird__help_commands() {
    local commands; commands=(
'login:Authenticate via xurl (OAuth2 PKCE browser flow)' \
'me:Show current user (GET /2/users/me)' \
'get:GET request to path (e.g. /2/users/me or /2/users/{id}/bookmarks with -p id=123)' \
'post:POST request to path' \
'put:PUT request to path' \
'bookmarks:List bookmarks for the current user (paginated, max_results=100)' \
'profile:Look up a user profile by username' \
'search:Search recent tweets (GET /2/tweets/search/recent)' \
'thread:Reconstruct a conversation thread from a tweet' \
'delete:DELETE request to path' \
'watchlist:Monitor users\: check recent activity, manage watchlist' \
'usage:View API usage and costs' \
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
'doctor:Show what is available\: xurl status, commands, and entity store health' \
'cache:Manage the HTTP response cache' \
'completions:Generate shell completions' \
'help:Print this message or the help of the given subcommand(s)' \
    )
    _describe -t commands 'bird help commands' commands "$@"
}
(( $+functions[_bird__help__block_commands] )) ||
_bird__help__block_commands() {
    local commands; commands=()
    _describe -t commands 'bird help block commands' commands "$@"
}
(( $+functions[_bird__help__bookmarks_commands] )) ||
_bird__help__bookmarks_commands() {
    local commands; commands=()
    _describe -t commands 'bird help bookmarks commands' commands "$@"
}
(( $+functions[_bird__help__cache_commands] )) ||
_bird__help__cache_commands() {
    local commands; commands=(
'clear:Delete all cache entries' \
'stats:Show cache status (JSON default, --pretty for human-readable)' \
    )
    _describe -t commands 'bird help cache commands' commands "$@"
}
(( $+functions[_bird__help__cache__clear_commands] )) ||
_bird__help__cache__clear_commands() {
    local commands; commands=()
    _describe -t commands 'bird help cache clear commands' commands "$@"
}
(( $+functions[_bird__help__cache__stats_commands] )) ||
_bird__help__cache__stats_commands() {
    local commands; commands=()
    _describe -t commands 'bird help cache stats commands' commands "$@"
}
(( $+functions[_bird__help__completions_commands] )) ||
_bird__help__completions_commands() {
    local commands; commands=()
    _describe -t commands 'bird help completions commands' commands "$@"
}
(( $+functions[_bird__help__delete_commands] )) ||
_bird__help__delete_commands() {
    local commands; commands=()
    _describe -t commands 'bird help delete commands' commands "$@"
}
(( $+functions[_bird__help__dm_commands] )) ||
_bird__help__dm_commands() {
    local commands; commands=()
    _describe -t commands 'bird help dm commands' commands "$@"
}
(( $+functions[_bird__help__doctor_commands] )) ||
_bird__help__doctor_commands() {
    local commands; commands=()
    _describe -t commands 'bird help doctor commands' commands "$@"
}
(( $+functions[_bird__help__follow_commands] )) ||
_bird__help__follow_commands() {
    local commands; commands=()
    _describe -t commands 'bird help follow commands' commands "$@"
}
(( $+functions[_bird__help__get_commands] )) ||
_bird__help__get_commands() {
    local commands; commands=()
    _describe -t commands 'bird help get commands' commands "$@"
}
(( $+functions[_bird__help__help_commands] )) ||
_bird__help__help_commands() {
    local commands; commands=()
    _describe -t commands 'bird help help commands' commands "$@"
}
(( $+functions[_bird__help__like_commands] )) ||
_bird__help__like_commands() {
    local commands; commands=()
    _describe -t commands 'bird help like commands' commands "$@"
}
(( $+functions[_bird__help__login_commands] )) ||
_bird__help__login_commands() {
    local commands; commands=()
    _describe -t commands 'bird help login commands' commands "$@"
}
(( $+functions[_bird__help__me_commands] )) ||
_bird__help__me_commands() {
    local commands; commands=()
    _describe -t commands 'bird help me commands' commands "$@"
}
(( $+functions[_bird__help__mute_commands] )) ||
_bird__help__mute_commands() {
    local commands; commands=()
    _describe -t commands 'bird help mute commands' commands "$@"
}
(( $+functions[_bird__help__post_commands] )) ||
_bird__help__post_commands() {
    local commands; commands=()
    _describe -t commands 'bird help post commands' commands "$@"
}
(( $+functions[_bird__help__profile_commands] )) ||
_bird__help__profile_commands() {
    local commands; commands=()
    _describe -t commands 'bird help profile commands' commands "$@"
}
(( $+functions[_bird__help__put_commands] )) ||
_bird__help__put_commands() {
    local commands; commands=()
    _describe -t commands 'bird help put commands' commands "$@"
}
(( $+functions[_bird__help__reply_commands] )) ||
_bird__help__reply_commands() {
    local commands; commands=()
    _describe -t commands 'bird help reply commands' commands "$@"
}
(( $+functions[_bird__help__repost_commands] )) ||
_bird__help__repost_commands() {
    local commands; commands=()
    _describe -t commands 'bird help repost commands' commands "$@"
}
(( $+functions[_bird__help__search_commands] )) ||
_bird__help__search_commands() {
    local commands; commands=()
    _describe -t commands 'bird help search commands' commands "$@"
}
(( $+functions[_bird__help__thread_commands] )) ||
_bird__help__thread_commands() {
    local commands; commands=()
    _describe -t commands 'bird help thread commands' commands "$@"
}
(( $+functions[_bird__help__tweet_commands] )) ||
_bird__help__tweet_commands() {
    local commands; commands=()
    _describe -t commands 'bird help tweet commands' commands "$@"
}
(( $+functions[_bird__help__unblock_commands] )) ||
_bird__help__unblock_commands() {
    local commands; commands=()
    _describe -t commands 'bird help unblock commands' commands "$@"
}
(( $+functions[_bird__help__unfollow_commands] )) ||
_bird__help__unfollow_commands() {
    local commands; commands=()
    _describe -t commands 'bird help unfollow commands' commands "$@"
}
(( $+functions[_bird__help__unlike_commands] )) ||
_bird__help__unlike_commands() {
    local commands; commands=()
    _describe -t commands 'bird help unlike commands' commands "$@"
}
(( $+functions[_bird__help__unmute_commands] )) ||
_bird__help__unmute_commands() {
    local commands; commands=()
    _describe -t commands 'bird help unmute commands' commands "$@"
}
(( $+functions[_bird__help__unrepost_commands] )) ||
_bird__help__unrepost_commands() {
    local commands; commands=()
    _describe -t commands 'bird help unrepost commands' commands "$@"
}
(( $+functions[_bird__help__usage_commands] )) ||
_bird__help__usage_commands() {
    local commands; commands=()
    _describe -t commands 'bird help usage commands' commands "$@"
}
(( $+functions[_bird__help__watchlist_commands] )) ||
_bird__help__watchlist_commands() {
    local commands; commands=(
'check:Check recent activity for all watched users' \
'add:Add a user to the watchlist' \
'remove:Remove a user from the watchlist' \
'list:Show the current watchlist' \
    )
    _describe -t commands 'bird help watchlist commands' commands "$@"
}
(( $+functions[_bird__help__watchlist__add_commands] )) ||
_bird__help__watchlist__add_commands() {
    local commands; commands=()
    _describe -t commands 'bird help watchlist add commands' commands "$@"
}
(( $+functions[_bird__help__watchlist__check_commands] )) ||
_bird__help__watchlist__check_commands() {
    local commands; commands=()
    _describe -t commands 'bird help watchlist check commands' commands "$@"
}
(( $+functions[_bird__help__watchlist__list_commands] )) ||
_bird__help__watchlist__list_commands() {
    local commands; commands=()
    _describe -t commands 'bird help watchlist list commands' commands "$@"
}
(( $+functions[_bird__help__watchlist__remove_commands] )) ||
_bird__help__watchlist__remove_commands() {
    local commands; commands=()
    _describe -t commands 'bird help watchlist remove commands' commands "$@"
}
(( $+functions[_bird__like_commands] )) ||
_bird__like_commands() {
    local commands; commands=()
    _describe -t commands 'bird like commands' commands "$@"
}
(( $+functions[_bird__login_commands] )) ||
_bird__login_commands() {
    local commands; commands=()
    _describe -t commands 'bird login commands' commands "$@"
}
(( $+functions[_bird__me_commands] )) ||
_bird__me_commands() {
    local commands; commands=()
    _describe -t commands 'bird me commands' commands "$@"
}
(( $+functions[_bird__mute_commands] )) ||
_bird__mute_commands() {
    local commands; commands=()
    _describe -t commands 'bird mute commands' commands "$@"
}
(( $+functions[_bird__post_commands] )) ||
_bird__post_commands() {
    local commands; commands=()
    _describe -t commands 'bird post commands' commands "$@"
}
(( $+functions[_bird__profile_commands] )) ||
_bird__profile_commands() {
    local commands; commands=()
    _describe -t commands 'bird profile commands' commands "$@"
}
(( $+functions[_bird__put_commands] )) ||
_bird__put_commands() {
    local commands; commands=()
    _describe -t commands 'bird put commands' commands "$@"
}
(( $+functions[_bird__reply_commands] )) ||
_bird__reply_commands() {
    local commands; commands=()
    _describe -t commands 'bird reply commands' commands "$@"
}
(( $+functions[_bird__repost_commands] )) ||
_bird__repost_commands() {
    local commands; commands=()
    _describe -t commands 'bird repost commands' commands "$@"
}
(( $+functions[_bird__search_commands] )) ||
_bird__search_commands() {
    local commands; commands=()
    _describe -t commands 'bird search commands' commands "$@"
}
(( $+functions[_bird__thread_commands] )) ||
_bird__thread_commands() {
    local commands; commands=()
    _describe -t commands 'bird thread commands' commands "$@"
}
(( $+functions[_bird__tweet_commands] )) ||
_bird__tweet_commands() {
    local commands; commands=()
    _describe -t commands 'bird tweet commands' commands "$@"
}
(( $+functions[_bird__unblock_commands] )) ||
_bird__unblock_commands() {
    local commands; commands=()
    _describe -t commands 'bird unblock commands' commands "$@"
}
(( $+functions[_bird__unfollow_commands] )) ||
_bird__unfollow_commands() {
    local commands; commands=()
    _describe -t commands 'bird unfollow commands' commands "$@"
}
(( $+functions[_bird__unlike_commands] )) ||
_bird__unlike_commands() {
    local commands; commands=()
    _describe -t commands 'bird unlike commands' commands "$@"
}
(( $+functions[_bird__unmute_commands] )) ||
_bird__unmute_commands() {
    local commands; commands=()
    _describe -t commands 'bird unmute commands' commands "$@"
}
(( $+functions[_bird__unrepost_commands] )) ||
_bird__unrepost_commands() {
    local commands; commands=()
    _describe -t commands 'bird unrepost commands' commands "$@"
}
(( $+functions[_bird__usage_commands] )) ||
_bird__usage_commands() {
    local commands; commands=()
    _describe -t commands 'bird usage commands' commands "$@"
}
(( $+functions[_bird__watchlist_commands] )) ||
_bird__watchlist_commands() {
    local commands; commands=(
'check:Check recent activity for all watched users' \
'add:Add a user to the watchlist' \
'remove:Remove a user from the watchlist' \
'list:Show the current watchlist' \
'help:Print this message or the help of the given subcommand(s)' \
    )
    _describe -t commands 'bird watchlist commands' commands "$@"
}
(( $+functions[_bird__watchlist__add_commands] )) ||
_bird__watchlist__add_commands() {
    local commands; commands=()
    _describe -t commands 'bird watchlist add commands' commands "$@"
}
(( $+functions[_bird__watchlist__check_commands] )) ||
_bird__watchlist__check_commands() {
    local commands; commands=()
    _describe -t commands 'bird watchlist check commands' commands "$@"
}
(( $+functions[_bird__watchlist__help_commands] )) ||
_bird__watchlist__help_commands() {
    local commands; commands=(
'check:Check recent activity for all watched users' \
'add:Add a user to the watchlist' \
'remove:Remove a user from the watchlist' \
'list:Show the current watchlist' \
'help:Print this message or the help of the given subcommand(s)' \
    )
    _describe -t commands 'bird watchlist help commands' commands "$@"
}
(( $+functions[_bird__watchlist__help__add_commands] )) ||
_bird__watchlist__help__add_commands() {
    local commands; commands=()
    _describe -t commands 'bird watchlist help add commands' commands "$@"
}
(( $+functions[_bird__watchlist__help__check_commands] )) ||
_bird__watchlist__help__check_commands() {
    local commands; commands=()
    _describe -t commands 'bird watchlist help check commands' commands "$@"
}
(( $+functions[_bird__watchlist__help__help_commands] )) ||
_bird__watchlist__help__help_commands() {
    local commands; commands=()
    _describe -t commands 'bird watchlist help help commands' commands "$@"
}
(( $+functions[_bird__watchlist__help__list_commands] )) ||
_bird__watchlist__help__list_commands() {
    local commands; commands=()
    _describe -t commands 'bird watchlist help list commands' commands "$@"
}
(( $+functions[_bird__watchlist__help__remove_commands] )) ||
_bird__watchlist__help__remove_commands() {
    local commands; commands=()
    _describe -t commands 'bird watchlist help remove commands' commands "$@"
}
(( $+functions[_bird__watchlist__list_commands] )) ||
_bird__watchlist__list_commands() {
    local commands; commands=()
    _describe -t commands 'bird watchlist list commands' commands "$@"
}
(( $+functions[_bird__watchlist__remove_commands] )) ||
_bird__watchlist__remove_commands() {
    local commands; commands=()
    _describe -t commands 'bird watchlist remove commands' commands "$@"
}

if [ "$funcstack[1]" = "_bird" ]; then
    _bird "$@"
else
    compdef _bird bird
fi
