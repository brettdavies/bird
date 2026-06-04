# Print an optspec for argparse to handle cmd's options that are independent of any subcommand.
function __fish_bird_global_optspecs
	string join \n u/username= o/output= json jsonl color= plain no-color q/quiet v/verbose timeout= no-interactive raw examples refresh no-cache cache-only limit= cursor= h/help V/version
end

function __fish_bird_needs_command
	# Figure out if the current invocation already has a command.
	set -l cmd (commandline -opc)
	set -e cmd[1]
	argparse -s (__fish_bird_global_optspecs) -- $cmd 2>/dev/null
	or return
	if set -q argv[1]
		# Also print the command, so this can be used to figure out what it is.
		echo $argv[1]
		return 1
	end
	return 0
end

function __fish_bird_using_subcommand
	set -l cmd (__fish_bird_needs_command)
	test -z "$cmd"
	and return 1
	contains -- $cmd[1] $argv
end

complete -c bird -n "__fish_bird_needs_command" -s u -l username -d 'Username for multi-user token selection (maps to xurl -u)' -r
complete -c bird -n "__fish_bird_needs_command" -s o -l output -d 'Output format (text, json, jsonl, ndjson). Defaults to json when piped' -r -f -a "text\t'Default: colored, human-readable'
json\t'Machine-readable JSON envelope, no color'
jsonl\t'Streaming line-delimited JSON (one object per line; no wrapper)'
ndjson\t'Newline-delimited JSON, accepted as an alias for jsonl'"
complete -c bird -n "__fish_bird_needs_command" -l color -d 'Color mode: auto (default), always, never' -r -f -a "auto\t'Auto-detect: color when stderr is a TTY and `NO_COLOR` is unset'
always\t'Always emit colors'
never\t'Never emit colors'"
complete -c bird -n "__fish_bird_needs_command" -l timeout -d 'Network timeout in seconds (default 30). Applies to xurl subprocesses' -r
complete -c bird -n "__fish_bird_needs_command" -l limit -d 'Maximum number of results to return on list-style commands (default 100, ceiling 1000)' -r
complete -c bird -n "__fish_bird_needs_command" -l cursor -d 'Pagination cursor token for list-style commands (X API `pagination_token`/`next_token`)' -r
complete -c bird -n "__fish_bird_needs_command" -l json -d 'Shorthand for `--output json`'
complete -c bird -n "__fish_bird_needs_command" -l jsonl -d 'Shorthand for `--output jsonl`'
complete -c bird -n "__fish_bird_needs_command" -l plain -d 'Deprecated alias for `--color never` (plain output, no color)'
complete -c bird -n "__fish_bird_needs_command" -l no-color -d 'Deprecated alias for `--color never`'
complete -c bird -n "__fish_bird_needs_command" -s q -l quiet -d 'Suppress informational stderr output (keep only fatal errors)'
complete -c bird -n "__fish_bird_needs_command" -s v -l verbose -d 'Increase verbosity (repeatable: -v info, -vv debug, -vvv trace)'
complete -c bird -n "__fish_bird_needs_command" -l no-interactive -d 'Disable interactive prompts (refuse anything that would block on stdin)'
complete -c bird -n "__fish_bird_needs_command" -l raw -d 'Emit pipe-safe, undecorated text. Ignored in JSON modes'
complete -c bird -n "__fish_bird_needs_command" -l examples -d 'Print curated examples block and exit'
complete -c bird -n "__fish_bird_needs_command" -l refresh -d 'Bypass store read, still write response to store'
complete -c bird -n "__fish_bird_needs_command" -l no-cache -d 'Disable entity store entirely (no read, no write)'
complete -c bird -n "__fish_bird_needs_command" -l cache-only -d 'Only serve from local store; never make API requests'
complete -c bird -n "__fish_bird_needs_command" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c bird -n "__fish_bird_needs_command" -s V -l version -d 'Print version'
complete -c bird -n "__fish_bird_needs_command" -f -a "login" -d 'Authenticate via xurl (OAuth2 PKCE browser flow)'
complete -c bird -n "__fish_bird_needs_command" -f -a "me" -d 'Show current user (GET /2/users/me)'
complete -c bird -n "__fish_bird_needs_command" -f -a "get" -d 'GET request to path (e.g. /2/users/me or /2/users/{id}/bookmarks with -p id=123)'
complete -c bird -n "__fish_bird_needs_command" -f -a "post" -d 'POST request to path'
complete -c bird -n "__fish_bird_needs_command" -f -a "put" -d 'PUT request to path'
complete -c bird -n "__fish_bird_needs_command" -f -a "bookmarks" -d 'List bookmarks for the current user (paginated, max_results=100)'
complete -c bird -n "__fish_bird_needs_command" -f -a "profile" -d 'Look up a user profile by username'
complete -c bird -n "__fish_bird_needs_command" -f -a "search" -d 'Search recent tweets (GET /2/tweets/search/recent)'
complete -c bird -n "__fish_bird_needs_command" -f -a "thread" -d 'Reconstruct a conversation thread from a tweet'
complete -c bird -n "__fish_bird_needs_command" -f -a "delete" -d 'DELETE request to path'
complete -c bird -n "__fish_bird_needs_command" -f -a "watchlist" -d 'Monitor users: check recent activity, manage watchlist'
complete -c bird -n "__fish_bird_needs_command" -f -a "usage" -d 'View API usage and costs'
complete -c bird -n "__fish_bird_needs_command" -f -a "tweet" -d 'Post a tweet (via xurl)'
complete -c bird -n "__fish_bird_needs_command" -f -a "reply" -d 'Reply to a tweet (via xurl)'
complete -c bird -n "__fish_bird_needs_command" -f -a "like" -d 'Like a tweet (via xurl)'
complete -c bird -n "__fish_bird_needs_command" -f -a "unlike" -d 'Unlike a tweet (via xurl)'
complete -c bird -n "__fish_bird_needs_command" -f -a "repost" -d 'Repost (retweet) a tweet (via xurl)'
complete -c bird -n "__fish_bird_needs_command" -f -a "unrepost" -d 'Undo a repost (via xurl)'
complete -c bird -n "__fish_bird_needs_command" -f -a "follow" -d 'Follow a user (via xurl)'
complete -c bird -n "__fish_bird_needs_command" -f -a "unfollow" -d 'Unfollow a user (via xurl)'
complete -c bird -n "__fish_bird_needs_command" -f -a "dm" -d 'Send a direct message (via xurl)'
complete -c bird -n "__fish_bird_needs_command" -f -a "block" -d 'Block a user (via xurl)'
complete -c bird -n "__fish_bird_needs_command" -f -a "unblock" -d 'Unblock a user (via xurl)'
complete -c bird -n "__fish_bird_needs_command" -f -a "mute" -d 'Mute a user (via xurl)'
complete -c bird -n "__fish_bird_needs_command" -f -a "unmute" -d 'Unmute a user (via xurl)'
complete -c bird -n "__fish_bird_needs_command" -f -a "doctor" -d 'Show what is available: xurl status, commands, and entity store health'
complete -c bird -n "__fish_bird_needs_command" -f -a "cache" -d 'Manage the HTTP response cache'
complete -c bird -n "__fish_bird_needs_command" -f -a "completions" -d 'Generate shell completions'
complete -c bird -n "__fish_bird_needs_command" -f -a "skill" -d 'Manage the bird agent-skill bundle (install for Claude Code, etc.)'
complete -c bird -n "__fish_bird_needs_command" -f -a "schema" -d 'Print a JSON Schema document for one of bird\'s output shapes'
complete -c bird -n "__fish_bird_needs_command" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c bird -n "__fish_bird_using_subcommand login" -s u -l username -d 'Username for multi-user token selection (maps to xurl -u)' -r
complete -c bird -n "__fish_bird_using_subcommand login" -s o -l output -d 'Output format (text, json, jsonl, ndjson). Defaults to json when piped' -r -f -a "text\t'Default: colored, human-readable'
json\t'Machine-readable JSON envelope, no color'
jsonl\t'Streaming line-delimited JSON (one object per line; no wrapper)'
ndjson\t'Newline-delimited JSON, accepted as an alias for jsonl'"
complete -c bird -n "__fish_bird_using_subcommand login" -l color -d 'Color mode: auto (default), always, never' -r -f -a "auto\t'Auto-detect: color when stderr is a TTY and `NO_COLOR` is unset'
always\t'Always emit colors'
never\t'Never emit colors'"
complete -c bird -n "__fish_bird_using_subcommand login" -l timeout -d 'Network timeout in seconds (default 30). Applies to xurl subprocesses' -r
complete -c bird -n "__fish_bird_using_subcommand login" -l limit -d 'Maximum number of results to return on list-style commands (default 100, ceiling 1000)' -r
complete -c bird -n "__fish_bird_using_subcommand login" -l cursor -d 'Pagination cursor token for list-style commands (X API `pagination_token`/`next_token`)' -r
complete -c bird -n "__fish_bird_using_subcommand login" -l no-browser -d 'Print the authorization URL on stdout and read the redirect URL back from stdin. No browser is launched'
complete -c bird -n "__fish_bird_using_subcommand login" -l json -d 'Shorthand for `--output json`'
complete -c bird -n "__fish_bird_using_subcommand login" -l jsonl -d 'Shorthand for `--output jsonl`'
complete -c bird -n "__fish_bird_using_subcommand login" -l plain -d 'Deprecated alias for `--color never` (plain output, no color)'
complete -c bird -n "__fish_bird_using_subcommand login" -l no-color -d 'Deprecated alias for `--color never`'
complete -c bird -n "__fish_bird_using_subcommand login" -s q -l quiet -d 'Suppress informational stderr output (keep only fatal errors)'
complete -c bird -n "__fish_bird_using_subcommand login" -s v -l verbose -d 'Increase verbosity (repeatable: -v info, -vv debug, -vvv trace)'
complete -c bird -n "__fish_bird_using_subcommand login" -l no-interactive -d 'Disable interactive prompts (refuse anything that would block on stdin)'
complete -c bird -n "__fish_bird_using_subcommand login" -l raw -d 'Emit pipe-safe, undecorated text. Ignored in JSON modes'
complete -c bird -n "__fish_bird_using_subcommand login" -l examples -d 'Print curated examples block and exit'
complete -c bird -n "__fish_bird_using_subcommand login" -l refresh -d 'Bypass store read, still write response to store'
complete -c bird -n "__fish_bird_using_subcommand login" -l no-cache -d 'Disable entity store entirely (no read, no write)'
complete -c bird -n "__fish_bird_using_subcommand login" -l cache-only -d 'Only serve from local store; never make API requests'
complete -c bird -n "__fish_bird_using_subcommand login" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c bird -n "__fish_bird_using_subcommand me" -s u -l username -d 'Username for multi-user token selection (maps to xurl -u)' -r
complete -c bird -n "__fish_bird_using_subcommand me" -s o -l output -d 'Output format (text, json, jsonl, ndjson). Defaults to json when piped' -r -f -a "text\t'Default: colored, human-readable'
json\t'Machine-readable JSON envelope, no color'
jsonl\t'Streaming line-delimited JSON (one object per line; no wrapper)'
ndjson\t'Newline-delimited JSON, accepted as an alias for jsonl'"
complete -c bird -n "__fish_bird_using_subcommand me" -l color -d 'Color mode: auto (default), always, never' -r -f -a "auto\t'Auto-detect: color when stderr is a TTY and `NO_COLOR` is unset'
always\t'Always emit colors'
never\t'Never emit colors'"
complete -c bird -n "__fish_bird_using_subcommand me" -l timeout -d 'Network timeout in seconds (default 30). Applies to xurl subprocesses' -r
complete -c bird -n "__fish_bird_using_subcommand me" -l limit -d 'Maximum number of results to return on list-style commands (default 100, ceiling 1000)' -r
complete -c bird -n "__fish_bird_using_subcommand me" -l cursor -d 'Pagination cursor token for list-style commands (X API `pagination_token`/`next_token`)' -r
complete -c bird -n "__fish_bird_using_subcommand me" -l pretty -d 'Human-readable output'
complete -c bird -n "__fish_bird_using_subcommand me" -l json -d 'Shorthand for `--output json`'
complete -c bird -n "__fish_bird_using_subcommand me" -l jsonl -d 'Shorthand for `--output jsonl`'
complete -c bird -n "__fish_bird_using_subcommand me" -l plain -d 'Deprecated alias for `--color never` (plain output, no color)'
complete -c bird -n "__fish_bird_using_subcommand me" -l no-color -d 'Deprecated alias for `--color never`'
complete -c bird -n "__fish_bird_using_subcommand me" -s q -l quiet -d 'Suppress informational stderr output (keep only fatal errors)'
complete -c bird -n "__fish_bird_using_subcommand me" -s v -l verbose -d 'Increase verbosity (repeatable: -v info, -vv debug, -vvv trace)'
complete -c bird -n "__fish_bird_using_subcommand me" -l no-interactive -d 'Disable interactive prompts (refuse anything that would block on stdin)'
complete -c bird -n "__fish_bird_using_subcommand me" -l raw -d 'Emit pipe-safe, undecorated text. Ignored in JSON modes'
complete -c bird -n "__fish_bird_using_subcommand me" -l examples -d 'Print curated examples block and exit'
complete -c bird -n "__fish_bird_using_subcommand me" -l refresh -d 'Bypass store read, still write response to store'
complete -c bird -n "__fish_bird_using_subcommand me" -l no-cache -d 'Disable entity store entirely (no read, no write)'
complete -c bird -n "__fish_bird_using_subcommand me" -l cache-only -d 'Only serve from local store; never make API requests'
complete -c bird -n "__fish_bird_using_subcommand me" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c bird -n "__fish_bird_using_subcommand get" -s p -l param -r
complete -c bird -n "__fish_bird_using_subcommand get" -l query -r
complete -c bird -n "__fish_bird_using_subcommand get" -s u -l username -d 'Username for multi-user token selection (maps to xurl -u)' -r
complete -c bird -n "__fish_bird_using_subcommand get" -s o -l output -d 'Output format (text, json, jsonl, ndjson). Defaults to json when piped' -r -f -a "text\t'Default: colored, human-readable'
json\t'Machine-readable JSON envelope, no color'
jsonl\t'Streaming line-delimited JSON (one object per line; no wrapper)'
ndjson\t'Newline-delimited JSON, accepted as an alias for jsonl'"
complete -c bird -n "__fish_bird_using_subcommand get" -l color -d 'Color mode: auto (default), always, never' -r -f -a "auto\t'Auto-detect: color when stderr is a TTY and `NO_COLOR` is unset'
always\t'Always emit colors'
never\t'Never emit colors'"
complete -c bird -n "__fish_bird_using_subcommand get" -l timeout -d 'Network timeout in seconds (default 30). Applies to xurl subprocesses' -r
complete -c bird -n "__fish_bird_using_subcommand get" -l limit -d 'Maximum number of results to return on list-style commands (default 100, ceiling 1000)' -r
complete -c bird -n "__fish_bird_using_subcommand get" -l cursor -d 'Pagination cursor token for list-style commands (X API `pagination_token`/`next_token`)' -r
complete -c bird -n "__fish_bird_using_subcommand get" -l pretty
complete -c bird -n "__fish_bird_using_subcommand get" -l json -d 'Shorthand for `--output json`'
complete -c bird -n "__fish_bird_using_subcommand get" -l jsonl -d 'Shorthand for `--output jsonl`'
complete -c bird -n "__fish_bird_using_subcommand get" -l plain -d 'Deprecated alias for `--color never` (plain output, no color)'
complete -c bird -n "__fish_bird_using_subcommand get" -l no-color -d 'Deprecated alias for `--color never`'
complete -c bird -n "__fish_bird_using_subcommand get" -s q -l quiet -d 'Suppress informational stderr output (keep only fatal errors)'
complete -c bird -n "__fish_bird_using_subcommand get" -s v -l verbose -d 'Increase verbosity (repeatable: -v info, -vv debug, -vvv trace)'
complete -c bird -n "__fish_bird_using_subcommand get" -l no-interactive -d 'Disable interactive prompts (refuse anything that would block on stdin)'
complete -c bird -n "__fish_bird_using_subcommand get" -l raw -d 'Emit pipe-safe, undecorated text. Ignored in JSON modes'
complete -c bird -n "__fish_bird_using_subcommand get" -l examples -d 'Print curated examples block and exit'
complete -c bird -n "__fish_bird_using_subcommand get" -l refresh -d 'Bypass store read, still write response to store'
complete -c bird -n "__fish_bird_using_subcommand get" -l no-cache -d 'Disable entity store entirely (no read, no write)'
complete -c bird -n "__fish_bird_using_subcommand get" -l cache-only -d 'Only serve from local store; never make API requests'
complete -c bird -n "__fish_bird_using_subcommand get" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c bird -n "__fish_bird_using_subcommand post" -s p -l param -r
complete -c bird -n "__fish_bird_using_subcommand post" -l query -r
complete -c bird -n "__fish_bird_using_subcommand post" -l body -r
complete -c bird -n "__fish_bird_using_subcommand post" -s u -l username -d 'Username for multi-user token selection (maps to xurl -u)' -r
complete -c bird -n "__fish_bird_using_subcommand post" -s o -l output -d 'Output format (text, json, jsonl, ndjson). Defaults to json when piped' -r -f -a "text\t'Default: colored, human-readable'
json\t'Machine-readable JSON envelope, no color'
jsonl\t'Streaming line-delimited JSON (one object per line; no wrapper)'
ndjson\t'Newline-delimited JSON, accepted as an alias for jsonl'"
complete -c bird -n "__fish_bird_using_subcommand post" -l color -d 'Color mode: auto (default), always, never' -r -f -a "auto\t'Auto-detect: color when stderr is a TTY and `NO_COLOR` is unset'
always\t'Always emit colors'
never\t'Never emit colors'"
complete -c bird -n "__fish_bird_using_subcommand post" -l timeout -d 'Network timeout in seconds (default 30). Applies to xurl subprocesses' -r
complete -c bird -n "__fish_bird_using_subcommand post" -l limit -d 'Maximum number of results to return on list-style commands (default 100, ceiling 1000)' -r
complete -c bird -n "__fish_bird_using_subcommand post" -l cursor -d 'Pagination cursor token for list-style commands (X API `pagination_token`/`next_token`)' -r
complete -c bird -n "__fish_bird_using_subcommand post" -l pretty
complete -c bird -n "__fish_bird_using_subcommand post" -s f -l force -d 'Skip the interactive confirmation prompt (alias: --yes)'
complete -c bird -n "__fish_bird_using_subcommand post" -l dry-run -d 'Validate inputs and print the would-be request, then exit without calling the API'
complete -c bird -n "__fish_bird_using_subcommand post" -l json -d 'Shorthand for `--output json`'
complete -c bird -n "__fish_bird_using_subcommand post" -l jsonl -d 'Shorthand for `--output jsonl`'
complete -c bird -n "__fish_bird_using_subcommand post" -l plain -d 'Deprecated alias for `--color never` (plain output, no color)'
complete -c bird -n "__fish_bird_using_subcommand post" -l no-color -d 'Deprecated alias for `--color never`'
complete -c bird -n "__fish_bird_using_subcommand post" -s q -l quiet -d 'Suppress informational stderr output (keep only fatal errors)'
complete -c bird -n "__fish_bird_using_subcommand post" -s v -l verbose -d 'Increase verbosity (repeatable: -v info, -vv debug, -vvv trace)'
complete -c bird -n "__fish_bird_using_subcommand post" -l no-interactive -d 'Disable interactive prompts (refuse anything that would block on stdin)'
complete -c bird -n "__fish_bird_using_subcommand post" -l raw -d 'Emit pipe-safe, undecorated text. Ignored in JSON modes'
complete -c bird -n "__fish_bird_using_subcommand post" -l examples -d 'Print curated examples block and exit'
complete -c bird -n "__fish_bird_using_subcommand post" -l refresh -d 'Bypass store read, still write response to store'
complete -c bird -n "__fish_bird_using_subcommand post" -l no-cache -d 'Disable entity store entirely (no read, no write)'
complete -c bird -n "__fish_bird_using_subcommand post" -l cache-only -d 'Only serve from local store; never make API requests'
complete -c bird -n "__fish_bird_using_subcommand post" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c bird -n "__fish_bird_using_subcommand put" -s p -l param -r
complete -c bird -n "__fish_bird_using_subcommand put" -l query -r
complete -c bird -n "__fish_bird_using_subcommand put" -l body -r
complete -c bird -n "__fish_bird_using_subcommand put" -s u -l username -d 'Username for multi-user token selection (maps to xurl -u)' -r
complete -c bird -n "__fish_bird_using_subcommand put" -s o -l output -d 'Output format (text, json, jsonl, ndjson). Defaults to json when piped' -r -f -a "text\t'Default: colored, human-readable'
json\t'Machine-readable JSON envelope, no color'
jsonl\t'Streaming line-delimited JSON (one object per line; no wrapper)'
ndjson\t'Newline-delimited JSON, accepted as an alias for jsonl'"
complete -c bird -n "__fish_bird_using_subcommand put" -l color -d 'Color mode: auto (default), always, never' -r -f -a "auto\t'Auto-detect: color when stderr is a TTY and `NO_COLOR` is unset'
always\t'Always emit colors'
never\t'Never emit colors'"
complete -c bird -n "__fish_bird_using_subcommand put" -l timeout -d 'Network timeout in seconds (default 30). Applies to xurl subprocesses' -r
complete -c bird -n "__fish_bird_using_subcommand put" -l limit -d 'Maximum number of results to return on list-style commands (default 100, ceiling 1000)' -r
complete -c bird -n "__fish_bird_using_subcommand put" -l cursor -d 'Pagination cursor token for list-style commands (X API `pagination_token`/`next_token`)' -r
complete -c bird -n "__fish_bird_using_subcommand put" -l pretty
complete -c bird -n "__fish_bird_using_subcommand put" -s f -l force -d 'Skip the interactive confirmation prompt (alias: --yes)'
complete -c bird -n "__fish_bird_using_subcommand put" -l dry-run -d 'Validate inputs and print the would-be request, then exit without calling the API'
complete -c bird -n "__fish_bird_using_subcommand put" -l json -d 'Shorthand for `--output json`'
complete -c bird -n "__fish_bird_using_subcommand put" -l jsonl -d 'Shorthand for `--output jsonl`'
complete -c bird -n "__fish_bird_using_subcommand put" -l plain -d 'Deprecated alias for `--color never` (plain output, no color)'
complete -c bird -n "__fish_bird_using_subcommand put" -l no-color -d 'Deprecated alias for `--color never`'
complete -c bird -n "__fish_bird_using_subcommand put" -s q -l quiet -d 'Suppress informational stderr output (keep only fatal errors)'
complete -c bird -n "__fish_bird_using_subcommand put" -s v -l verbose -d 'Increase verbosity (repeatable: -v info, -vv debug, -vvv trace)'
complete -c bird -n "__fish_bird_using_subcommand put" -l no-interactive -d 'Disable interactive prompts (refuse anything that would block on stdin)'
complete -c bird -n "__fish_bird_using_subcommand put" -l raw -d 'Emit pipe-safe, undecorated text. Ignored in JSON modes'
complete -c bird -n "__fish_bird_using_subcommand put" -l examples -d 'Print curated examples block and exit'
complete -c bird -n "__fish_bird_using_subcommand put" -l refresh -d 'Bypass store read, still write response to store'
complete -c bird -n "__fish_bird_using_subcommand put" -l no-cache -d 'Disable entity store entirely (no read, no write)'
complete -c bird -n "__fish_bird_using_subcommand put" -l cache-only -d 'Only serve from local store; never make API requests'
complete -c bird -n "__fish_bird_using_subcommand put" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c bird -n "__fish_bird_using_subcommand bookmarks" -s u -l username -d 'Username for multi-user token selection (maps to xurl -u)' -r
complete -c bird -n "__fish_bird_using_subcommand bookmarks" -s o -l output -d 'Output format (text, json, jsonl, ndjson). Defaults to json when piped' -r -f -a "text\t'Default: colored, human-readable'
json\t'Machine-readable JSON envelope, no color'
jsonl\t'Streaming line-delimited JSON (one object per line; no wrapper)'
ndjson\t'Newline-delimited JSON, accepted as an alias for jsonl'"
complete -c bird -n "__fish_bird_using_subcommand bookmarks" -l color -d 'Color mode: auto (default), always, never' -r -f -a "auto\t'Auto-detect: color when stderr is a TTY and `NO_COLOR` is unset'
always\t'Always emit colors'
never\t'Never emit colors'"
complete -c bird -n "__fish_bird_using_subcommand bookmarks" -l timeout -d 'Network timeout in seconds (default 30). Applies to xurl subprocesses' -r
complete -c bird -n "__fish_bird_using_subcommand bookmarks" -l limit -d 'Maximum number of results to return on list-style commands (default 100, ceiling 1000)' -r
complete -c bird -n "__fish_bird_using_subcommand bookmarks" -l cursor -d 'Pagination cursor token for list-style commands (X API `pagination_token`/`next_token`)' -r
complete -c bird -n "__fish_bird_using_subcommand bookmarks" -l pretty
complete -c bird -n "__fish_bird_using_subcommand bookmarks" -l json -d 'Shorthand for `--output json`'
complete -c bird -n "__fish_bird_using_subcommand bookmarks" -l jsonl -d 'Shorthand for `--output jsonl`'
complete -c bird -n "__fish_bird_using_subcommand bookmarks" -l plain -d 'Deprecated alias for `--color never` (plain output, no color)'
complete -c bird -n "__fish_bird_using_subcommand bookmarks" -l no-color -d 'Deprecated alias for `--color never`'
complete -c bird -n "__fish_bird_using_subcommand bookmarks" -s q -l quiet -d 'Suppress informational stderr output (keep only fatal errors)'
complete -c bird -n "__fish_bird_using_subcommand bookmarks" -s v -l verbose -d 'Increase verbosity (repeatable: -v info, -vv debug, -vvv trace)'
complete -c bird -n "__fish_bird_using_subcommand bookmarks" -l no-interactive -d 'Disable interactive prompts (refuse anything that would block on stdin)'
complete -c bird -n "__fish_bird_using_subcommand bookmarks" -l raw -d 'Emit pipe-safe, undecorated text. Ignored in JSON modes'
complete -c bird -n "__fish_bird_using_subcommand bookmarks" -l examples -d 'Print curated examples block and exit'
complete -c bird -n "__fish_bird_using_subcommand bookmarks" -l refresh -d 'Bypass store read, still write response to store'
complete -c bird -n "__fish_bird_using_subcommand bookmarks" -l no-cache -d 'Disable entity store entirely (no read, no write)'
complete -c bird -n "__fish_bird_using_subcommand bookmarks" -l cache-only -d 'Only serve from local store; never make API requests'
complete -c bird -n "__fish_bird_using_subcommand bookmarks" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c bird -n "__fish_bird_using_subcommand profile" -s o -l output -d 'Output format (text, json, jsonl, ndjson). Defaults to json when piped' -r -f -a "text\t'Default: colored, human-readable'
json\t'Machine-readable JSON envelope, no color'
jsonl\t'Streaming line-delimited JSON (one object per line; no wrapper)'
ndjson\t'Newline-delimited JSON, accepted as an alias for jsonl'"
complete -c bird -n "__fish_bird_using_subcommand profile" -l color -d 'Color mode: auto (default), always, never' -r -f -a "auto\t'Auto-detect: color when stderr is a TTY and `NO_COLOR` is unset'
always\t'Always emit colors'
never\t'Never emit colors'"
complete -c bird -n "__fish_bird_using_subcommand profile" -l timeout -d 'Network timeout in seconds (default 30). Applies to xurl subprocesses' -r
complete -c bird -n "__fish_bird_using_subcommand profile" -l limit -d 'Maximum number of results to return on list-style commands (default 100, ceiling 1000)' -r
complete -c bird -n "__fish_bird_using_subcommand profile" -l cursor -d 'Pagination cursor token for list-style commands (X API `pagination_token`/`next_token`)' -r
complete -c bird -n "__fish_bird_using_subcommand profile" -l pretty -d 'Pretty-print JSON output'
complete -c bird -n "__fish_bird_using_subcommand profile" -l json -d 'Shorthand for `--output json`'
complete -c bird -n "__fish_bird_using_subcommand profile" -l jsonl -d 'Shorthand for `--output jsonl`'
complete -c bird -n "__fish_bird_using_subcommand profile" -l plain -d 'Deprecated alias for `--color never` (plain output, no color)'
complete -c bird -n "__fish_bird_using_subcommand profile" -l no-color -d 'Deprecated alias for `--color never`'
complete -c bird -n "__fish_bird_using_subcommand profile" -s q -l quiet -d 'Suppress informational stderr output (keep only fatal errors)'
complete -c bird -n "__fish_bird_using_subcommand profile" -s v -l verbose -d 'Increase verbosity (repeatable: -v info, -vv debug, -vvv trace)'
complete -c bird -n "__fish_bird_using_subcommand profile" -l no-interactive -d 'Disable interactive prompts (refuse anything that would block on stdin)'
complete -c bird -n "__fish_bird_using_subcommand profile" -l raw -d 'Emit pipe-safe, undecorated text. Ignored in JSON modes'
complete -c bird -n "__fish_bird_using_subcommand profile" -l examples -d 'Print curated examples block and exit'
complete -c bird -n "__fish_bird_using_subcommand profile" -l refresh -d 'Bypass store read, still write response to store'
complete -c bird -n "__fish_bird_using_subcommand profile" -l no-cache -d 'Disable entity store entirely (no read, no write)'
complete -c bird -n "__fish_bird_using_subcommand profile" -l cache-only -d 'Only serve from local store; never make API requests'
complete -c bird -n "__fish_bird_using_subcommand profile" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c bird -n "__fish_bird_using_subcommand search" -l sort -d 'Sort results: recent (default), likes' -r
complete -c bird -n "__fish_bird_using_subcommand search" -l min-likes -d 'Minimum like count threshold' -r
complete -c bird -n "__fish_bird_using_subcommand search" -l max-results -d 'Maximum results per page (10-100, default: 100)' -r
complete -c bird -n "__fish_bird_using_subcommand search" -l pages -d 'Number of pages to fetch (1-10, default: 1)' -r
complete -c bird -n "__fish_bird_using_subcommand search" -s u -l username -d 'Username for multi-user token selection (maps to xurl -u)' -r
complete -c bird -n "__fish_bird_using_subcommand search" -s o -l output -d 'Output format (text, json, jsonl, ndjson). Defaults to json when piped' -r -f -a "text\t'Default: colored, human-readable'
json\t'Machine-readable JSON envelope, no color'
jsonl\t'Streaming line-delimited JSON (one object per line; no wrapper)'
ndjson\t'Newline-delimited JSON, accepted as an alias for jsonl'"
complete -c bird -n "__fish_bird_using_subcommand search" -l color -d 'Color mode: auto (default), always, never' -r -f -a "auto\t'Auto-detect: color when stderr is a TTY and `NO_COLOR` is unset'
always\t'Always emit colors'
never\t'Never emit colors'"
complete -c bird -n "__fish_bird_using_subcommand search" -l timeout -d 'Network timeout in seconds (default 30). Applies to xurl subprocesses' -r
complete -c bird -n "__fish_bird_using_subcommand search" -l limit -d 'Maximum number of results to return on list-style commands (default 100, ceiling 1000)' -r
complete -c bird -n "__fish_bird_using_subcommand search" -l cursor -d 'Pagination cursor token for list-style commands (X API `pagination_token`/`next_token`)' -r
complete -c bird -n "__fish_bird_using_subcommand search" -l pretty -d 'Pretty-print JSON output'
complete -c bird -n "__fish_bird_using_subcommand search" -l json -d 'Shorthand for `--output json`'
complete -c bird -n "__fish_bird_using_subcommand search" -l jsonl -d 'Shorthand for `--output jsonl`'
complete -c bird -n "__fish_bird_using_subcommand search" -l plain -d 'Deprecated alias for `--color never` (plain output, no color)'
complete -c bird -n "__fish_bird_using_subcommand search" -l no-color -d 'Deprecated alias for `--color never`'
complete -c bird -n "__fish_bird_using_subcommand search" -s q -l quiet -d 'Suppress informational stderr output (keep only fatal errors)'
complete -c bird -n "__fish_bird_using_subcommand search" -s v -l verbose -d 'Increase verbosity (repeatable: -v info, -vv debug, -vvv trace)'
complete -c bird -n "__fish_bird_using_subcommand search" -l no-interactive -d 'Disable interactive prompts (refuse anything that would block on stdin)'
complete -c bird -n "__fish_bird_using_subcommand search" -l raw -d 'Emit pipe-safe, undecorated text. Ignored in JSON modes'
complete -c bird -n "__fish_bird_using_subcommand search" -l examples -d 'Print curated examples block and exit'
complete -c bird -n "__fish_bird_using_subcommand search" -l refresh -d 'Bypass store read, still write response to store'
complete -c bird -n "__fish_bird_using_subcommand search" -l no-cache -d 'Disable entity store entirely (no read, no write)'
complete -c bird -n "__fish_bird_using_subcommand search" -l cache-only -d 'Only serve from local store; never make API requests'
complete -c bird -n "__fish_bird_using_subcommand search" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c bird -n "__fish_bird_using_subcommand thread" -l max-pages -d 'Maximum number of search result pages (default: 10, max: 25)' -r
complete -c bird -n "__fish_bird_using_subcommand thread" -s u -l username -d 'Username for multi-user token selection (maps to xurl -u)' -r
complete -c bird -n "__fish_bird_using_subcommand thread" -s o -l output -d 'Output format (text, json, jsonl, ndjson). Defaults to json when piped' -r -f -a "text\t'Default: colored, human-readable'
json\t'Machine-readable JSON envelope, no color'
jsonl\t'Streaming line-delimited JSON (one object per line; no wrapper)'
ndjson\t'Newline-delimited JSON, accepted as an alias for jsonl'"
complete -c bird -n "__fish_bird_using_subcommand thread" -l color -d 'Color mode: auto (default), always, never' -r -f -a "auto\t'Auto-detect: color when stderr is a TTY and `NO_COLOR` is unset'
always\t'Always emit colors'
never\t'Never emit colors'"
complete -c bird -n "__fish_bird_using_subcommand thread" -l timeout -d 'Network timeout in seconds (default 30). Applies to xurl subprocesses' -r
complete -c bird -n "__fish_bird_using_subcommand thread" -l limit -d 'Maximum number of results to return on list-style commands (default 100, ceiling 1000)' -r
complete -c bird -n "__fish_bird_using_subcommand thread" -l cursor -d 'Pagination cursor token for list-style commands (X API `pagination_token`/`next_token`)' -r
complete -c bird -n "__fish_bird_using_subcommand thread" -l pretty -d 'Pretty-print JSON output'
complete -c bird -n "__fish_bird_using_subcommand thread" -l json -d 'Shorthand for `--output json`'
complete -c bird -n "__fish_bird_using_subcommand thread" -l jsonl -d 'Shorthand for `--output jsonl`'
complete -c bird -n "__fish_bird_using_subcommand thread" -l plain -d 'Deprecated alias for `--color never` (plain output, no color)'
complete -c bird -n "__fish_bird_using_subcommand thread" -l no-color -d 'Deprecated alias for `--color never`'
complete -c bird -n "__fish_bird_using_subcommand thread" -s q -l quiet -d 'Suppress informational stderr output (keep only fatal errors)'
complete -c bird -n "__fish_bird_using_subcommand thread" -s v -l verbose -d 'Increase verbosity (repeatable: -v info, -vv debug, -vvv trace)'
complete -c bird -n "__fish_bird_using_subcommand thread" -l no-interactive -d 'Disable interactive prompts (refuse anything that would block on stdin)'
complete -c bird -n "__fish_bird_using_subcommand thread" -l raw -d 'Emit pipe-safe, undecorated text. Ignored in JSON modes'
complete -c bird -n "__fish_bird_using_subcommand thread" -l examples -d 'Print curated examples block and exit'
complete -c bird -n "__fish_bird_using_subcommand thread" -l refresh -d 'Bypass store read, still write response to store'
complete -c bird -n "__fish_bird_using_subcommand thread" -l no-cache -d 'Disable entity store entirely (no read, no write)'
complete -c bird -n "__fish_bird_using_subcommand thread" -l cache-only -d 'Only serve from local store; never make API requests'
complete -c bird -n "__fish_bird_using_subcommand thread" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c bird -n "__fish_bird_using_subcommand delete" -s p -l param -r
complete -c bird -n "__fish_bird_using_subcommand delete" -l query -r
complete -c bird -n "__fish_bird_using_subcommand delete" -s u -l username -d 'Username for multi-user token selection (maps to xurl -u)' -r
complete -c bird -n "__fish_bird_using_subcommand delete" -s o -l output -d 'Output format (text, json, jsonl, ndjson). Defaults to json when piped' -r -f -a "text\t'Default: colored, human-readable'
json\t'Machine-readable JSON envelope, no color'
jsonl\t'Streaming line-delimited JSON (one object per line; no wrapper)'
ndjson\t'Newline-delimited JSON, accepted as an alias for jsonl'"
complete -c bird -n "__fish_bird_using_subcommand delete" -l color -d 'Color mode: auto (default), always, never' -r -f -a "auto\t'Auto-detect: color when stderr is a TTY and `NO_COLOR` is unset'
always\t'Always emit colors'
never\t'Never emit colors'"
complete -c bird -n "__fish_bird_using_subcommand delete" -l timeout -d 'Network timeout in seconds (default 30). Applies to xurl subprocesses' -r
complete -c bird -n "__fish_bird_using_subcommand delete" -l limit -d 'Maximum number of results to return on list-style commands (default 100, ceiling 1000)' -r
complete -c bird -n "__fish_bird_using_subcommand delete" -l cursor -d 'Pagination cursor token for list-style commands (X API `pagination_token`/`next_token`)' -r
complete -c bird -n "__fish_bird_using_subcommand delete" -l pretty
complete -c bird -n "__fish_bird_using_subcommand delete" -s f -l force -d 'Skip the interactive confirmation prompt (alias: --yes)'
complete -c bird -n "__fish_bird_using_subcommand delete" -l dry-run -d 'Validate inputs and print the would-be request, then exit without calling the API'
complete -c bird -n "__fish_bird_using_subcommand delete" -l json -d 'Shorthand for `--output json`'
complete -c bird -n "__fish_bird_using_subcommand delete" -l jsonl -d 'Shorthand for `--output jsonl`'
complete -c bird -n "__fish_bird_using_subcommand delete" -l plain -d 'Deprecated alias for `--color never` (plain output, no color)'
complete -c bird -n "__fish_bird_using_subcommand delete" -l no-color -d 'Deprecated alias for `--color never`'
complete -c bird -n "__fish_bird_using_subcommand delete" -s q -l quiet -d 'Suppress informational stderr output (keep only fatal errors)'
complete -c bird -n "__fish_bird_using_subcommand delete" -s v -l verbose -d 'Increase verbosity (repeatable: -v info, -vv debug, -vvv trace)'
complete -c bird -n "__fish_bird_using_subcommand delete" -l no-interactive -d 'Disable interactive prompts (refuse anything that would block on stdin)'
complete -c bird -n "__fish_bird_using_subcommand delete" -l raw -d 'Emit pipe-safe, undecorated text. Ignored in JSON modes'
complete -c bird -n "__fish_bird_using_subcommand delete" -l examples -d 'Print curated examples block and exit'
complete -c bird -n "__fish_bird_using_subcommand delete" -l refresh -d 'Bypass store read, still write response to store'
complete -c bird -n "__fish_bird_using_subcommand delete" -l no-cache -d 'Disable entity store entirely (no read, no write)'
complete -c bird -n "__fish_bird_using_subcommand delete" -l cache-only -d 'Only serve from local store; never make API requests'
complete -c bird -n "__fish_bird_using_subcommand delete" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and not __fish_seen_subcommand_from fetch add remove list help" -s u -l username -d 'Username for multi-user token selection (maps to xurl -u)' -r
complete -c bird -n "__fish_bird_using_subcommand watchlist; and not __fish_seen_subcommand_from fetch add remove list help" -s o -l output -d 'Output format (text, json, jsonl, ndjson). Defaults to json when piped' -r -f -a "text\t'Default: colored, human-readable'
json\t'Machine-readable JSON envelope, no color'
jsonl\t'Streaming line-delimited JSON (one object per line; no wrapper)'
ndjson\t'Newline-delimited JSON, accepted as an alias for jsonl'"
complete -c bird -n "__fish_bird_using_subcommand watchlist; and not __fish_seen_subcommand_from fetch add remove list help" -l color -d 'Color mode: auto (default), always, never' -r -f -a "auto\t'Auto-detect: color when stderr is a TTY and `NO_COLOR` is unset'
always\t'Always emit colors'
never\t'Never emit colors'"
complete -c bird -n "__fish_bird_using_subcommand watchlist; and not __fish_seen_subcommand_from fetch add remove list help" -l timeout -d 'Network timeout in seconds (default 30). Applies to xurl subprocesses' -r
complete -c bird -n "__fish_bird_using_subcommand watchlist; and not __fish_seen_subcommand_from fetch add remove list help" -l limit -d 'Maximum number of results to return on list-style commands (default 100, ceiling 1000)' -r
complete -c bird -n "__fish_bird_using_subcommand watchlist; and not __fish_seen_subcommand_from fetch add remove list help" -l cursor -d 'Pagination cursor token for list-style commands (X API `pagination_token`/`next_token`)' -r
complete -c bird -n "__fish_bird_using_subcommand watchlist; and not __fish_seen_subcommand_from fetch add remove list help" -l pretty -d 'Pretty-print JSON output'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and not __fish_seen_subcommand_from fetch add remove list help" -l json -d 'Shorthand for `--output json`'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and not __fish_seen_subcommand_from fetch add remove list help" -l jsonl -d 'Shorthand for `--output jsonl`'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and not __fish_seen_subcommand_from fetch add remove list help" -l plain -d 'Deprecated alias for `--color never` (plain output, no color)'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and not __fish_seen_subcommand_from fetch add remove list help" -l no-color -d 'Deprecated alias for `--color never`'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and not __fish_seen_subcommand_from fetch add remove list help" -s q -l quiet -d 'Suppress informational stderr output (keep only fatal errors)'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and not __fish_seen_subcommand_from fetch add remove list help" -s v -l verbose -d 'Increase verbosity (repeatable: -v info, -vv debug, -vvv trace)'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and not __fish_seen_subcommand_from fetch add remove list help" -l no-interactive -d 'Disable interactive prompts (refuse anything that would block on stdin)'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and not __fish_seen_subcommand_from fetch add remove list help" -l raw -d 'Emit pipe-safe, undecorated text. Ignored in JSON modes'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and not __fish_seen_subcommand_from fetch add remove list help" -l examples -d 'Print curated examples block and exit'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and not __fish_seen_subcommand_from fetch add remove list help" -l refresh -d 'Bypass store read, still write response to store'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and not __fish_seen_subcommand_from fetch add remove list help" -l no-cache -d 'Disable entity store entirely (no read, no write)'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and not __fish_seen_subcommand_from fetch add remove list help" -l cache-only -d 'Only serve from local store; never make API requests'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and not __fish_seen_subcommand_from fetch add remove list help" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and not __fish_seen_subcommand_from fetch add remove list help" -f -a "fetch" -d 'Fetch recent activity for all watched users'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and not __fish_seen_subcommand_from fetch add remove list help" -f -a "add" -d 'Add a user to the watchlist'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and not __fish_seen_subcommand_from fetch add remove list help" -f -a "remove" -d 'Remove a user from the watchlist'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and not __fish_seen_subcommand_from fetch add remove list help" -f -a "list" -d 'Show the current watchlist'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and not __fish_seen_subcommand_from fetch add remove list help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from fetch" -s u -l username -d 'Username for multi-user token selection (maps to xurl -u)' -r
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from fetch" -s o -l output -d 'Output format (text, json, jsonl, ndjson). Defaults to json when piped' -r -f -a "text\t'Default: colored, human-readable'
json\t'Machine-readable JSON envelope, no color'
jsonl\t'Streaming line-delimited JSON (one object per line; no wrapper)'
ndjson\t'Newline-delimited JSON, accepted as an alias for jsonl'"
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from fetch" -l color -d 'Color mode: auto (default), always, never' -r -f -a "auto\t'Auto-detect: color when stderr is a TTY and `NO_COLOR` is unset'
always\t'Always emit colors'
never\t'Never emit colors'"
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from fetch" -l timeout -d 'Network timeout in seconds (default 30). Applies to xurl subprocesses' -r
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from fetch" -l limit -d 'Maximum number of results to return on list-style commands (default 100, ceiling 1000)' -r
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from fetch" -l cursor -d 'Pagination cursor token for list-style commands (X API `pagination_token`/`next_token`)' -r
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from fetch" -l json -d 'Shorthand for `--output json`'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from fetch" -l jsonl -d 'Shorthand for `--output jsonl`'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from fetch" -l plain -d 'Deprecated alias for `--color never` (plain output, no color)'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from fetch" -l no-color -d 'Deprecated alias for `--color never`'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from fetch" -s q -l quiet -d 'Suppress informational stderr output (keep only fatal errors)'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from fetch" -s v -l verbose -d 'Increase verbosity (repeatable: -v info, -vv debug, -vvv trace)'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from fetch" -l no-interactive -d 'Disable interactive prompts (refuse anything that would block on stdin)'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from fetch" -l raw -d 'Emit pipe-safe, undecorated text. Ignored in JSON modes'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from fetch" -l examples -d 'Print curated examples block and exit'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from fetch" -l refresh -d 'Bypass store read, still write response to store'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from fetch" -l no-cache -d 'Disable entity store entirely (no read, no write)'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from fetch" -l cache-only -d 'Only serve from local store; never make API requests'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from fetch" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from add" -s o -l output -d 'Output format (text, json, jsonl, ndjson). Defaults to json when piped' -r -f -a "text\t'Default: colored, human-readable'
json\t'Machine-readable JSON envelope, no color'
jsonl\t'Streaming line-delimited JSON (one object per line; no wrapper)'
ndjson\t'Newline-delimited JSON, accepted as an alias for jsonl'"
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from add" -l color -d 'Color mode: auto (default), always, never' -r -f -a "auto\t'Auto-detect: color when stderr is a TTY and `NO_COLOR` is unset'
always\t'Always emit colors'
never\t'Never emit colors'"
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from add" -l timeout -d 'Network timeout in seconds (default 30). Applies to xurl subprocesses' -r
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from add" -l limit -d 'Maximum number of results to return on list-style commands (default 100, ceiling 1000)' -r
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from add" -l cursor -d 'Pagination cursor token for list-style commands (X API `pagination_token`/`next_token`)' -r
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from add" -l json -d 'Shorthand for `--output json`'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from add" -l jsonl -d 'Shorthand for `--output jsonl`'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from add" -l plain -d 'Deprecated alias for `--color never` (plain output, no color)'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from add" -l no-color -d 'Deprecated alias for `--color never`'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from add" -s q -l quiet -d 'Suppress informational stderr output (keep only fatal errors)'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from add" -s v -l verbose -d 'Increase verbosity (repeatable: -v info, -vv debug, -vvv trace)'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from add" -l no-interactive -d 'Disable interactive prompts (refuse anything that would block on stdin)'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from add" -l raw -d 'Emit pipe-safe, undecorated text. Ignored in JSON modes'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from add" -l examples -d 'Print curated examples block and exit'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from add" -l refresh -d 'Bypass store read, still write response to store'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from add" -l no-cache -d 'Disable entity store entirely (no read, no write)'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from add" -l cache-only -d 'Only serve from local store; never make API requests'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from add" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from remove" -s o -l output -d 'Output format (text, json, jsonl, ndjson). Defaults to json when piped' -r -f -a "text\t'Default: colored, human-readable'
json\t'Machine-readable JSON envelope, no color'
jsonl\t'Streaming line-delimited JSON (one object per line; no wrapper)'
ndjson\t'Newline-delimited JSON, accepted as an alias for jsonl'"
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from remove" -l color -d 'Color mode: auto (default), always, never' -r -f -a "auto\t'Auto-detect: color when stderr is a TTY and `NO_COLOR` is unset'
always\t'Always emit colors'
never\t'Never emit colors'"
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from remove" -l timeout -d 'Network timeout in seconds (default 30). Applies to xurl subprocesses' -r
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from remove" -l limit -d 'Maximum number of results to return on list-style commands (default 100, ceiling 1000)' -r
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from remove" -l cursor -d 'Pagination cursor token for list-style commands (X API `pagination_token`/`next_token`)' -r
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from remove" -s f -l force -d 'Skip the interactive confirmation prompt (alias: --yes)'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from remove" -l dry-run -d 'Validate inputs and print the would-be request, then exit without calling the API'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from remove" -l json -d 'Shorthand for `--output json`'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from remove" -l jsonl -d 'Shorthand for `--output jsonl`'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from remove" -l plain -d 'Deprecated alias for `--color never` (plain output, no color)'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from remove" -l no-color -d 'Deprecated alias for `--color never`'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from remove" -s q -l quiet -d 'Suppress informational stderr output (keep only fatal errors)'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from remove" -s v -l verbose -d 'Increase verbosity (repeatable: -v info, -vv debug, -vvv trace)'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from remove" -l no-interactive -d 'Disable interactive prompts (refuse anything that would block on stdin)'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from remove" -l raw -d 'Emit pipe-safe, undecorated text. Ignored in JSON modes'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from remove" -l examples -d 'Print curated examples block and exit'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from remove" -l refresh -d 'Bypass store read, still write response to store'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from remove" -l no-cache -d 'Disable entity store entirely (no read, no write)'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from remove" -l cache-only -d 'Only serve from local store; never make API requests'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from remove" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from list" -s u -l username -d 'Username for multi-user token selection (maps to xurl -u)' -r
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from list" -s o -l output -d 'Output format (text, json, jsonl, ndjson). Defaults to json when piped' -r -f -a "text\t'Default: colored, human-readable'
json\t'Machine-readable JSON envelope, no color'
jsonl\t'Streaming line-delimited JSON (one object per line; no wrapper)'
ndjson\t'Newline-delimited JSON, accepted as an alias for jsonl'"
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from list" -l color -d 'Color mode: auto (default), always, never' -r -f -a "auto\t'Auto-detect: color when stderr is a TTY and `NO_COLOR` is unset'
always\t'Always emit colors'
never\t'Never emit colors'"
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from list" -l timeout -d 'Network timeout in seconds (default 30). Applies to xurl subprocesses' -r
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from list" -l limit -d 'Maximum number of results to return on list-style commands (default 100, ceiling 1000)' -r
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from list" -l cursor -d 'Pagination cursor token for list-style commands (X API `pagination_token`/`next_token`)' -r
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from list" -l json -d 'Shorthand for `--output json`'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from list" -l jsonl -d 'Shorthand for `--output jsonl`'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from list" -l plain -d 'Deprecated alias for `--color never` (plain output, no color)'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from list" -l no-color -d 'Deprecated alias for `--color never`'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from list" -s q -l quiet -d 'Suppress informational stderr output (keep only fatal errors)'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from list" -s v -l verbose -d 'Increase verbosity (repeatable: -v info, -vv debug, -vvv trace)'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from list" -l no-interactive -d 'Disable interactive prompts (refuse anything that would block on stdin)'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from list" -l raw -d 'Emit pipe-safe, undecorated text. Ignored in JSON modes'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from list" -l examples -d 'Print curated examples block and exit'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from list" -l refresh -d 'Bypass store read, still write response to store'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from list" -l no-cache -d 'Disable entity store entirely (no read, no write)'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from list" -l cache-only -d 'Only serve from local store; never make API requests'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from list" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from help" -f -a "fetch" -d 'Fetch recent activity for all watched users'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from help" -f -a "add" -d 'Add a user to the watchlist'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from help" -f -a "remove" -d 'Remove a user from the watchlist'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from help" -f -a "list" -d 'Show the current watchlist'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c bird -n "__fish_bird_using_subcommand usage" -l since -d 'Show usage since this date (YYYY-MM-DD; default: 30 days ago)' -r
complete -c bird -n "__fish_bird_using_subcommand usage" -s u -l username -d 'Username for multi-user token selection (maps to xurl -u)' -r
complete -c bird -n "__fish_bird_using_subcommand usage" -s o -l output -d 'Output format (text, json, jsonl, ndjson). Defaults to json when piped' -r -f -a "text\t'Default: colored, human-readable'
json\t'Machine-readable JSON envelope, no color'
jsonl\t'Streaming line-delimited JSON (one object per line; no wrapper)'
ndjson\t'Newline-delimited JSON, accepted as an alias for jsonl'"
complete -c bird -n "__fish_bird_using_subcommand usage" -l color -d 'Color mode: auto (default), always, never' -r -f -a "auto\t'Auto-detect: color when stderr is a TTY and `NO_COLOR` is unset'
always\t'Always emit colors'
never\t'Never emit colors'"
complete -c bird -n "__fish_bird_using_subcommand usage" -l timeout -d 'Network timeout in seconds (default 30). Applies to xurl subprocesses' -r
complete -c bird -n "__fish_bird_using_subcommand usage" -l limit -d 'Maximum number of results to return on list-style commands (default 100, ceiling 1000)' -r
complete -c bird -n "__fish_bird_using_subcommand usage" -l cursor -d 'Pagination cursor token for list-style commands (X API `pagination_token`/`next_token`)' -r
complete -c bird -n "__fish_bird_using_subcommand usage" -l local -d 'Show only local estimates (skip API)'
complete -c bird -n "__fish_bird_using_subcommand usage" -l pretty -d 'Pretty-print output'
complete -c bird -n "__fish_bird_using_subcommand usage" -l json -d 'Shorthand for `--output json`'
complete -c bird -n "__fish_bird_using_subcommand usage" -l jsonl -d 'Shorthand for `--output jsonl`'
complete -c bird -n "__fish_bird_using_subcommand usage" -l plain -d 'Deprecated alias for `--color never` (plain output, no color)'
complete -c bird -n "__fish_bird_using_subcommand usage" -l no-color -d 'Deprecated alias for `--color never`'
complete -c bird -n "__fish_bird_using_subcommand usage" -s q -l quiet -d 'Suppress informational stderr output (keep only fatal errors)'
complete -c bird -n "__fish_bird_using_subcommand usage" -s v -l verbose -d 'Increase verbosity (repeatable: -v info, -vv debug, -vvv trace)'
complete -c bird -n "__fish_bird_using_subcommand usage" -l no-interactive -d 'Disable interactive prompts (refuse anything that would block on stdin)'
complete -c bird -n "__fish_bird_using_subcommand usage" -l raw -d 'Emit pipe-safe, undecorated text. Ignored in JSON modes'
complete -c bird -n "__fish_bird_using_subcommand usage" -l examples -d 'Print curated examples block and exit'
complete -c bird -n "__fish_bird_using_subcommand usage" -l refresh -d 'Bypass store read, still write response to store'
complete -c bird -n "__fish_bird_using_subcommand usage" -l no-cache -d 'Disable entity store entirely (no read, no write)'
complete -c bird -n "__fish_bird_using_subcommand usage" -l cache-only -d 'Only serve from local store; never make API requests'
complete -c bird -n "__fish_bird_using_subcommand usage" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c bird -n "__fish_bird_using_subcommand tweet" -l media-id -d 'Media ID to attach' -r
complete -c bird -n "__fish_bird_using_subcommand tweet" -s u -l username -d 'Username for multi-user token selection (maps to xurl -u)' -r
complete -c bird -n "__fish_bird_using_subcommand tweet" -s o -l output -d 'Output format (text, json, jsonl, ndjson). Defaults to json when piped' -r -f -a "text\t'Default: colored, human-readable'
json\t'Machine-readable JSON envelope, no color'
jsonl\t'Streaming line-delimited JSON (one object per line; no wrapper)'
ndjson\t'Newline-delimited JSON, accepted as an alias for jsonl'"
complete -c bird -n "__fish_bird_using_subcommand tweet" -l color -d 'Color mode: auto (default), always, never' -r -f -a "auto\t'Auto-detect: color when stderr is a TTY and `NO_COLOR` is unset'
always\t'Always emit colors'
never\t'Never emit colors'"
complete -c bird -n "__fish_bird_using_subcommand tweet" -l timeout -d 'Network timeout in seconds (default 30). Applies to xurl subprocesses' -r
complete -c bird -n "__fish_bird_using_subcommand tweet" -l limit -d 'Maximum number of results to return on list-style commands (default 100, ceiling 1000)' -r
complete -c bird -n "__fish_bird_using_subcommand tweet" -l cursor -d 'Pagination cursor token for list-style commands (X API `pagination_token`/`next_token`)' -r
complete -c bird -n "__fish_bird_using_subcommand tweet" -s f -l force -d 'Skip the interactive confirmation prompt (alias: --yes)'
complete -c bird -n "__fish_bird_using_subcommand tweet" -l dry-run -d 'Validate inputs and print the would-be request, then exit without calling the API'
complete -c bird -n "__fish_bird_using_subcommand tweet" -l json -d 'Shorthand for `--output json`'
complete -c bird -n "__fish_bird_using_subcommand tweet" -l jsonl -d 'Shorthand for `--output jsonl`'
complete -c bird -n "__fish_bird_using_subcommand tweet" -l plain -d 'Deprecated alias for `--color never` (plain output, no color)'
complete -c bird -n "__fish_bird_using_subcommand tweet" -l no-color -d 'Deprecated alias for `--color never`'
complete -c bird -n "__fish_bird_using_subcommand tweet" -s q -l quiet -d 'Suppress informational stderr output (keep only fatal errors)'
complete -c bird -n "__fish_bird_using_subcommand tweet" -s v -l verbose -d 'Increase verbosity (repeatable: -v info, -vv debug, -vvv trace)'
complete -c bird -n "__fish_bird_using_subcommand tweet" -l no-interactive -d 'Disable interactive prompts (refuse anything that would block on stdin)'
complete -c bird -n "__fish_bird_using_subcommand tweet" -l raw -d 'Emit pipe-safe, undecorated text. Ignored in JSON modes'
complete -c bird -n "__fish_bird_using_subcommand tweet" -l examples -d 'Print curated examples block and exit'
complete -c bird -n "__fish_bird_using_subcommand tweet" -l refresh -d 'Bypass store read, still write response to store'
complete -c bird -n "__fish_bird_using_subcommand tweet" -l no-cache -d 'Disable entity store entirely (no read, no write)'
complete -c bird -n "__fish_bird_using_subcommand tweet" -l cache-only -d 'Only serve from local store; never make API requests'
complete -c bird -n "__fish_bird_using_subcommand tweet" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c bird -n "__fish_bird_using_subcommand reply" -s u -l username -d 'Username for multi-user token selection (maps to xurl -u)' -r
complete -c bird -n "__fish_bird_using_subcommand reply" -s o -l output -d 'Output format (text, json, jsonl, ndjson). Defaults to json when piped' -r -f -a "text\t'Default: colored, human-readable'
json\t'Machine-readable JSON envelope, no color'
jsonl\t'Streaming line-delimited JSON (one object per line; no wrapper)'
ndjson\t'Newline-delimited JSON, accepted as an alias for jsonl'"
complete -c bird -n "__fish_bird_using_subcommand reply" -l color -d 'Color mode: auto (default), always, never' -r -f -a "auto\t'Auto-detect: color when stderr is a TTY and `NO_COLOR` is unset'
always\t'Always emit colors'
never\t'Never emit colors'"
complete -c bird -n "__fish_bird_using_subcommand reply" -l timeout -d 'Network timeout in seconds (default 30). Applies to xurl subprocesses' -r
complete -c bird -n "__fish_bird_using_subcommand reply" -l limit -d 'Maximum number of results to return on list-style commands (default 100, ceiling 1000)' -r
complete -c bird -n "__fish_bird_using_subcommand reply" -l cursor -d 'Pagination cursor token for list-style commands (X API `pagination_token`/`next_token`)' -r
complete -c bird -n "__fish_bird_using_subcommand reply" -s f -l force -d 'Skip the interactive confirmation prompt (alias: --yes)'
complete -c bird -n "__fish_bird_using_subcommand reply" -l dry-run -d 'Validate inputs and print the would-be request, then exit without calling the API'
complete -c bird -n "__fish_bird_using_subcommand reply" -l json -d 'Shorthand for `--output json`'
complete -c bird -n "__fish_bird_using_subcommand reply" -l jsonl -d 'Shorthand for `--output jsonl`'
complete -c bird -n "__fish_bird_using_subcommand reply" -l plain -d 'Deprecated alias for `--color never` (plain output, no color)'
complete -c bird -n "__fish_bird_using_subcommand reply" -l no-color -d 'Deprecated alias for `--color never`'
complete -c bird -n "__fish_bird_using_subcommand reply" -s q -l quiet -d 'Suppress informational stderr output (keep only fatal errors)'
complete -c bird -n "__fish_bird_using_subcommand reply" -s v -l verbose -d 'Increase verbosity (repeatable: -v info, -vv debug, -vvv trace)'
complete -c bird -n "__fish_bird_using_subcommand reply" -l no-interactive -d 'Disable interactive prompts (refuse anything that would block on stdin)'
complete -c bird -n "__fish_bird_using_subcommand reply" -l raw -d 'Emit pipe-safe, undecorated text. Ignored in JSON modes'
complete -c bird -n "__fish_bird_using_subcommand reply" -l examples -d 'Print curated examples block and exit'
complete -c bird -n "__fish_bird_using_subcommand reply" -l refresh -d 'Bypass store read, still write response to store'
complete -c bird -n "__fish_bird_using_subcommand reply" -l no-cache -d 'Disable entity store entirely (no read, no write)'
complete -c bird -n "__fish_bird_using_subcommand reply" -l cache-only -d 'Only serve from local store; never make API requests'
complete -c bird -n "__fish_bird_using_subcommand reply" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c bird -n "__fish_bird_using_subcommand like" -s u -l username -d 'Username for multi-user token selection (maps to xurl -u)' -r
complete -c bird -n "__fish_bird_using_subcommand like" -s o -l output -d 'Output format (text, json, jsonl, ndjson). Defaults to json when piped' -r -f -a "text\t'Default: colored, human-readable'
json\t'Machine-readable JSON envelope, no color'
jsonl\t'Streaming line-delimited JSON (one object per line; no wrapper)'
ndjson\t'Newline-delimited JSON, accepted as an alias for jsonl'"
complete -c bird -n "__fish_bird_using_subcommand like" -l color -d 'Color mode: auto (default), always, never' -r -f -a "auto\t'Auto-detect: color when stderr is a TTY and `NO_COLOR` is unset'
always\t'Always emit colors'
never\t'Never emit colors'"
complete -c bird -n "__fish_bird_using_subcommand like" -l timeout -d 'Network timeout in seconds (default 30). Applies to xurl subprocesses' -r
complete -c bird -n "__fish_bird_using_subcommand like" -l limit -d 'Maximum number of results to return on list-style commands (default 100, ceiling 1000)' -r
complete -c bird -n "__fish_bird_using_subcommand like" -l cursor -d 'Pagination cursor token for list-style commands (X API `pagination_token`/`next_token`)' -r
complete -c bird -n "__fish_bird_using_subcommand like" -s f -l force -d 'Skip the interactive confirmation prompt (alias: --yes)'
complete -c bird -n "__fish_bird_using_subcommand like" -l dry-run -d 'Validate inputs and print the would-be request, then exit without calling the API'
complete -c bird -n "__fish_bird_using_subcommand like" -l json -d 'Shorthand for `--output json`'
complete -c bird -n "__fish_bird_using_subcommand like" -l jsonl -d 'Shorthand for `--output jsonl`'
complete -c bird -n "__fish_bird_using_subcommand like" -l plain -d 'Deprecated alias for `--color never` (plain output, no color)'
complete -c bird -n "__fish_bird_using_subcommand like" -l no-color -d 'Deprecated alias for `--color never`'
complete -c bird -n "__fish_bird_using_subcommand like" -s q -l quiet -d 'Suppress informational stderr output (keep only fatal errors)'
complete -c bird -n "__fish_bird_using_subcommand like" -s v -l verbose -d 'Increase verbosity (repeatable: -v info, -vv debug, -vvv trace)'
complete -c bird -n "__fish_bird_using_subcommand like" -l no-interactive -d 'Disable interactive prompts (refuse anything that would block on stdin)'
complete -c bird -n "__fish_bird_using_subcommand like" -l raw -d 'Emit pipe-safe, undecorated text. Ignored in JSON modes'
complete -c bird -n "__fish_bird_using_subcommand like" -l examples -d 'Print curated examples block and exit'
complete -c bird -n "__fish_bird_using_subcommand like" -l refresh -d 'Bypass store read, still write response to store'
complete -c bird -n "__fish_bird_using_subcommand like" -l no-cache -d 'Disable entity store entirely (no read, no write)'
complete -c bird -n "__fish_bird_using_subcommand like" -l cache-only -d 'Only serve from local store; never make API requests'
complete -c bird -n "__fish_bird_using_subcommand like" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c bird -n "__fish_bird_using_subcommand unlike" -s u -l username -d 'Username for multi-user token selection (maps to xurl -u)' -r
complete -c bird -n "__fish_bird_using_subcommand unlike" -s o -l output -d 'Output format (text, json, jsonl, ndjson). Defaults to json when piped' -r -f -a "text\t'Default: colored, human-readable'
json\t'Machine-readable JSON envelope, no color'
jsonl\t'Streaming line-delimited JSON (one object per line; no wrapper)'
ndjson\t'Newline-delimited JSON, accepted as an alias for jsonl'"
complete -c bird -n "__fish_bird_using_subcommand unlike" -l color -d 'Color mode: auto (default), always, never' -r -f -a "auto\t'Auto-detect: color when stderr is a TTY and `NO_COLOR` is unset'
always\t'Always emit colors'
never\t'Never emit colors'"
complete -c bird -n "__fish_bird_using_subcommand unlike" -l timeout -d 'Network timeout in seconds (default 30). Applies to xurl subprocesses' -r
complete -c bird -n "__fish_bird_using_subcommand unlike" -l limit -d 'Maximum number of results to return on list-style commands (default 100, ceiling 1000)' -r
complete -c bird -n "__fish_bird_using_subcommand unlike" -l cursor -d 'Pagination cursor token for list-style commands (X API `pagination_token`/`next_token`)' -r
complete -c bird -n "__fish_bird_using_subcommand unlike" -s f -l force -d 'Skip the interactive confirmation prompt (alias: --yes)'
complete -c bird -n "__fish_bird_using_subcommand unlike" -l dry-run -d 'Validate inputs and print the would-be request, then exit without calling the API'
complete -c bird -n "__fish_bird_using_subcommand unlike" -l json -d 'Shorthand for `--output json`'
complete -c bird -n "__fish_bird_using_subcommand unlike" -l jsonl -d 'Shorthand for `--output jsonl`'
complete -c bird -n "__fish_bird_using_subcommand unlike" -l plain -d 'Deprecated alias for `--color never` (plain output, no color)'
complete -c bird -n "__fish_bird_using_subcommand unlike" -l no-color -d 'Deprecated alias for `--color never`'
complete -c bird -n "__fish_bird_using_subcommand unlike" -s q -l quiet -d 'Suppress informational stderr output (keep only fatal errors)'
complete -c bird -n "__fish_bird_using_subcommand unlike" -s v -l verbose -d 'Increase verbosity (repeatable: -v info, -vv debug, -vvv trace)'
complete -c bird -n "__fish_bird_using_subcommand unlike" -l no-interactive -d 'Disable interactive prompts (refuse anything that would block on stdin)'
complete -c bird -n "__fish_bird_using_subcommand unlike" -l raw -d 'Emit pipe-safe, undecorated text. Ignored in JSON modes'
complete -c bird -n "__fish_bird_using_subcommand unlike" -l examples -d 'Print curated examples block and exit'
complete -c bird -n "__fish_bird_using_subcommand unlike" -l refresh -d 'Bypass store read, still write response to store'
complete -c bird -n "__fish_bird_using_subcommand unlike" -l no-cache -d 'Disable entity store entirely (no read, no write)'
complete -c bird -n "__fish_bird_using_subcommand unlike" -l cache-only -d 'Only serve from local store; never make API requests'
complete -c bird -n "__fish_bird_using_subcommand unlike" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c bird -n "__fish_bird_using_subcommand repost" -s u -l username -d 'Username for multi-user token selection (maps to xurl -u)' -r
complete -c bird -n "__fish_bird_using_subcommand repost" -s o -l output -d 'Output format (text, json, jsonl, ndjson). Defaults to json when piped' -r -f -a "text\t'Default: colored, human-readable'
json\t'Machine-readable JSON envelope, no color'
jsonl\t'Streaming line-delimited JSON (one object per line; no wrapper)'
ndjson\t'Newline-delimited JSON, accepted as an alias for jsonl'"
complete -c bird -n "__fish_bird_using_subcommand repost" -l color -d 'Color mode: auto (default), always, never' -r -f -a "auto\t'Auto-detect: color when stderr is a TTY and `NO_COLOR` is unset'
always\t'Always emit colors'
never\t'Never emit colors'"
complete -c bird -n "__fish_bird_using_subcommand repost" -l timeout -d 'Network timeout in seconds (default 30). Applies to xurl subprocesses' -r
complete -c bird -n "__fish_bird_using_subcommand repost" -l limit -d 'Maximum number of results to return on list-style commands (default 100, ceiling 1000)' -r
complete -c bird -n "__fish_bird_using_subcommand repost" -l cursor -d 'Pagination cursor token for list-style commands (X API `pagination_token`/`next_token`)' -r
complete -c bird -n "__fish_bird_using_subcommand repost" -s f -l force -d 'Skip the interactive confirmation prompt (alias: --yes)'
complete -c bird -n "__fish_bird_using_subcommand repost" -l dry-run -d 'Validate inputs and print the would-be request, then exit without calling the API'
complete -c bird -n "__fish_bird_using_subcommand repost" -l json -d 'Shorthand for `--output json`'
complete -c bird -n "__fish_bird_using_subcommand repost" -l jsonl -d 'Shorthand for `--output jsonl`'
complete -c bird -n "__fish_bird_using_subcommand repost" -l plain -d 'Deprecated alias for `--color never` (plain output, no color)'
complete -c bird -n "__fish_bird_using_subcommand repost" -l no-color -d 'Deprecated alias for `--color never`'
complete -c bird -n "__fish_bird_using_subcommand repost" -s q -l quiet -d 'Suppress informational stderr output (keep only fatal errors)'
complete -c bird -n "__fish_bird_using_subcommand repost" -s v -l verbose -d 'Increase verbosity (repeatable: -v info, -vv debug, -vvv trace)'
complete -c bird -n "__fish_bird_using_subcommand repost" -l no-interactive -d 'Disable interactive prompts (refuse anything that would block on stdin)'
complete -c bird -n "__fish_bird_using_subcommand repost" -l raw -d 'Emit pipe-safe, undecorated text. Ignored in JSON modes'
complete -c bird -n "__fish_bird_using_subcommand repost" -l examples -d 'Print curated examples block and exit'
complete -c bird -n "__fish_bird_using_subcommand repost" -l refresh -d 'Bypass store read, still write response to store'
complete -c bird -n "__fish_bird_using_subcommand repost" -l no-cache -d 'Disable entity store entirely (no read, no write)'
complete -c bird -n "__fish_bird_using_subcommand repost" -l cache-only -d 'Only serve from local store; never make API requests'
complete -c bird -n "__fish_bird_using_subcommand repost" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c bird -n "__fish_bird_using_subcommand unrepost" -s u -l username -d 'Username for multi-user token selection (maps to xurl -u)' -r
complete -c bird -n "__fish_bird_using_subcommand unrepost" -s o -l output -d 'Output format (text, json, jsonl, ndjson). Defaults to json when piped' -r -f -a "text\t'Default: colored, human-readable'
json\t'Machine-readable JSON envelope, no color'
jsonl\t'Streaming line-delimited JSON (one object per line; no wrapper)'
ndjson\t'Newline-delimited JSON, accepted as an alias for jsonl'"
complete -c bird -n "__fish_bird_using_subcommand unrepost" -l color -d 'Color mode: auto (default), always, never' -r -f -a "auto\t'Auto-detect: color when stderr is a TTY and `NO_COLOR` is unset'
always\t'Always emit colors'
never\t'Never emit colors'"
complete -c bird -n "__fish_bird_using_subcommand unrepost" -l timeout -d 'Network timeout in seconds (default 30). Applies to xurl subprocesses' -r
complete -c bird -n "__fish_bird_using_subcommand unrepost" -l limit -d 'Maximum number of results to return on list-style commands (default 100, ceiling 1000)' -r
complete -c bird -n "__fish_bird_using_subcommand unrepost" -l cursor -d 'Pagination cursor token for list-style commands (X API `pagination_token`/`next_token`)' -r
complete -c bird -n "__fish_bird_using_subcommand unrepost" -s f -l force -d 'Skip the interactive confirmation prompt (alias: --yes)'
complete -c bird -n "__fish_bird_using_subcommand unrepost" -l dry-run -d 'Validate inputs and print the would-be request, then exit without calling the API'
complete -c bird -n "__fish_bird_using_subcommand unrepost" -l json -d 'Shorthand for `--output json`'
complete -c bird -n "__fish_bird_using_subcommand unrepost" -l jsonl -d 'Shorthand for `--output jsonl`'
complete -c bird -n "__fish_bird_using_subcommand unrepost" -l plain -d 'Deprecated alias for `--color never` (plain output, no color)'
complete -c bird -n "__fish_bird_using_subcommand unrepost" -l no-color -d 'Deprecated alias for `--color never`'
complete -c bird -n "__fish_bird_using_subcommand unrepost" -s q -l quiet -d 'Suppress informational stderr output (keep only fatal errors)'
complete -c bird -n "__fish_bird_using_subcommand unrepost" -s v -l verbose -d 'Increase verbosity (repeatable: -v info, -vv debug, -vvv trace)'
complete -c bird -n "__fish_bird_using_subcommand unrepost" -l no-interactive -d 'Disable interactive prompts (refuse anything that would block on stdin)'
complete -c bird -n "__fish_bird_using_subcommand unrepost" -l raw -d 'Emit pipe-safe, undecorated text. Ignored in JSON modes'
complete -c bird -n "__fish_bird_using_subcommand unrepost" -l examples -d 'Print curated examples block and exit'
complete -c bird -n "__fish_bird_using_subcommand unrepost" -l refresh -d 'Bypass store read, still write response to store'
complete -c bird -n "__fish_bird_using_subcommand unrepost" -l no-cache -d 'Disable entity store entirely (no read, no write)'
complete -c bird -n "__fish_bird_using_subcommand unrepost" -l cache-only -d 'Only serve from local store; never make API requests'
complete -c bird -n "__fish_bird_using_subcommand unrepost" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c bird -n "__fish_bird_using_subcommand follow" -s o -l output -d 'Output format (text, json, jsonl, ndjson). Defaults to json when piped' -r -f -a "text\t'Default: colored, human-readable'
json\t'Machine-readable JSON envelope, no color'
jsonl\t'Streaming line-delimited JSON (one object per line; no wrapper)'
ndjson\t'Newline-delimited JSON, accepted as an alias for jsonl'"
complete -c bird -n "__fish_bird_using_subcommand follow" -l color -d 'Color mode: auto (default), always, never' -r -f -a "auto\t'Auto-detect: color when stderr is a TTY and `NO_COLOR` is unset'
always\t'Always emit colors'
never\t'Never emit colors'"
complete -c bird -n "__fish_bird_using_subcommand follow" -l timeout -d 'Network timeout in seconds (default 30). Applies to xurl subprocesses' -r
complete -c bird -n "__fish_bird_using_subcommand follow" -l limit -d 'Maximum number of results to return on list-style commands (default 100, ceiling 1000)' -r
complete -c bird -n "__fish_bird_using_subcommand follow" -l cursor -d 'Pagination cursor token for list-style commands (X API `pagination_token`/`next_token`)' -r
complete -c bird -n "__fish_bird_using_subcommand follow" -s f -l force -d 'Skip the interactive confirmation prompt (alias: --yes)'
complete -c bird -n "__fish_bird_using_subcommand follow" -l dry-run -d 'Validate inputs and print the would-be request, then exit without calling the API'
complete -c bird -n "__fish_bird_using_subcommand follow" -l json -d 'Shorthand for `--output json`'
complete -c bird -n "__fish_bird_using_subcommand follow" -l jsonl -d 'Shorthand for `--output jsonl`'
complete -c bird -n "__fish_bird_using_subcommand follow" -l plain -d 'Deprecated alias for `--color never` (plain output, no color)'
complete -c bird -n "__fish_bird_using_subcommand follow" -l no-color -d 'Deprecated alias for `--color never`'
complete -c bird -n "__fish_bird_using_subcommand follow" -s q -l quiet -d 'Suppress informational stderr output (keep only fatal errors)'
complete -c bird -n "__fish_bird_using_subcommand follow" -s v -l verbose -d 'Increase verbosity (repeatable: -v info, -vv debug, -vvv trace)'
complete -c bird -n "__fish_bird_using_subcommand follow" -l no-interactive -d 'Disable interactive prompts (refuse anything that would block on stdin)'
complete -c bird -n "__fish_bird_using_subcommand follow" -l raw -d 'Emit pipe-safe, undecorated text. Ignored in JSON modes'
complete -c bird -n "__fish_bird_using_subcommand follow" -l examples -d 'Print curated examples block and exit'
complete -c bird -n "__fish_bird_using_subcommand follow" -l refresh -d 'Bypass store read, still write response to store'
complete -c bird -n "__fish_bird_using_subcommand follow" -l no-cache -d 'Disable entity store entirely (no read, no write)'
complete -c bird -n "__fish_bird_using_subcommand follow" -l cache-only -d 'Only serve from local store; never make API requests'
complete -c bird -n "__fish_bird_using_subcommand follow" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c bird -n "__fish_bird_using_subcommand unfollow" -s o -l output -d 'Output format (text, json, jsonl, ndjson). Defaults to json when piped' -r -f -a "text\t'Default: colored, human-readable'
json\t'Machine-readable JSON envelope, no color'
jsonl\t'Streaming line-delimited JSON (one object per line; no wrapper)'
ndjson\t'Newline-delimited JSON, accepted as an alias for jsonl'"
complete -c bird -n "__fish_bird_using_subcommand unfollow" -l color -d 'Color mode: auto (default), always, never' -r -f -a "auto\t'Auto-detect: color when stderr is a TTY and `NO_COLOR` is unset'
always\t'Always emit colors'
never\t'Never emit colors'"
complete -c bird -n "__fish_bird_using_subcommand unfollow" -l timeout -d 'Network timeout in seconds (default 30). Applies to xurl subprocesses' -r
complete -c bird -n "__fish_bird_using_subcommand unfollow" -l limit -d 'Maximum number of results to return on list-style commands (default 100, ceiling 1000)' -r
complete -c bird -n "__fish_bird_using_subcommand unfollow" -l cursor -d 'Pagination cursor token for list-style commands (X API `pagination_token`/`next_token`)' -r
complete -c bird -n "__fish_bird_using_subcommand unfollow" -s f -l force -d 'Skip the interactive confirmation prompt (alias: --yes)'
complete -c bird -n "__fish_bird_using_subcommand unfollow" -l dry-run -d 'Validate inputs and print the would-be request, then exit without calling the API'
complete -c bird -n "__fish_bird_using_subcommand unfollow" -l json -d 'Shorthand for `--output json`'
complete -c bird -n "__fish_bird_using_subcommand unfollow" -l jsonl -d 'Shorthand for `--output jsonl`'
complete -c bird -n "__fish_bird_using_subcommand unfollow" -l plain -d 'Deprecated alias for `--color never` (plain output, no color)'
complete -c bird -n "__fish_bird_using_subcommand unfollow" -l no-color -d 'Deprecated alias for `--color never`'
complete -c bird -n "__fish_bird_using_subcommand unfollow" -s q -l quiet -d 'Suppress informational stderr output (keep only fatal errors)'
complete -c bird -n "__fish_bird_using_subcommand unfollow" -s v -l verbose -d 'Increase verbosity (repeatable: -v info, -vv debug, -vvv trace)'
complete -c bird -n "__fish_bird_using_subcommand unfollow" -l no-interactive -d 'Disable interactive prompts (refuse anything that would block on stdin)'
complete -c bird -n "__fish_bird_using_subcommand unfollow" -l raw -d 'Emit pipe-safe, undecorated text. Ignored in JSON modes'
complete -c bird -n "__fish_bird_using_subcommand unfollow" -l examples -d 'Print curated examples block and exit'
complete -c bird -n "__fish_bird_using_subcommand unfollow" -l refresh -d 'Bypass store read, still write response to store'
complete -c bird -n "__fish_bird_using_subcommand unfollow" -l no-cache -d 'Disable entity store entirely (no read, no write)'
complete -c bird -n "__fish_bird_using_subcommand unfollow" -l cache-only -d 'Only serve from local store; never make API requests'
complete -c bird -n "__fish_bird_using_subcommand unfollow" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c bird -n "__fish_bird_using_subcommand dm" -s o -l output -d 'Output format (text, json, jsonl, ndjson). Defaults to json when piped' -r -f -a "text\t'Default: colored, human-readable'
json\t'Machine-readable JSON envelope, no color'
jsonl\t'Streaming line-delimited JSON (one object per line; no wrapper)'
ndjson\t'Newline-delimited JSON, accepted as an alias for jsonl'"
complete -c bird -n "__fish_bird_using_subcommand dm" -l color -d 'Color mode: auto (default), always, never' -r -f -a "auto\t'Auto-detect: color when stderr is a TTY and `NO_COLOR` is unset'
always\t'Always emit colors'
never\t'Never emit colors'"
complete -c bird -n "__fish_bird_using_subcommand dm" -l timeout -d 'Network timeout in seconds (default 30). Applies to xurl subprocesses' -r
complete -c bird -n "__fish_bird_using_subcommand dm" -l limit -d 'Maximum number of results to return on list-style commands (default 100, ceiling 1000)' -r
complete -c bird -n "__fish_bird_using_subcommand dm" -l cursor -d 'Pagination cursor token for list-style commands (X API `pagination_token`/`next_token`)' -r
complete -c bird -n "__fish_bird_using_subcommand dm" -s f -l force -d 'Skip the interactive confirmation prompt (alias: --yes)'
complete -c bird -n "__fish_bird_using_subcommand dm" -l dry-run -d 'Validate inputs and print the would-be request, then exit without calling the API'
complete -c bird -n "__fish_bird_using_subcommand dm" -l json -d 'Shorthand for `--output json`'
complete -c bird -n "__fish_bird_using_subcommand dm" -l jsonl -d 'Shorthand for `--output jsonl`'
complete -c bird -n "__fish_bird_using_subcommand dm" -l plain -d 'Deprecated alias for `--color never` (plain output, no color)'
complete -c bird -n "__fish_bird_using_subcommand dm" -l no-color -d 'Deprecated alias for `--color never`'
complete -c bird -n "__fish_bird_using_subcommand dm" -s q -l quiet -d 'Suppress informational stderr output (keep only fatal errors)'
complete -c bird -n "__fish_bird_using_subcommand dm" -s v -l verbose -d 'Increase verbosity (repeatable: -v info, -vv debug, -vvv trace)'
complete -c bird -n "__fish_bird_using_subcommand dm" -l no-interactive -d 'Disable interactive prompts (refuse anything that would block on stdin)'
complete -c bird -n "__fish_bird_using_subcommand dm" -l raw -d 'Emit pipe-safe, undecorated text. Ignored in JSON modes'
complete -c bird -n "__fish_bird_using_subcommand dm" -l examples -d 'Print curated examples block and exit'
complete -c bird -n "__fish_bird_using_subcommand dm" -l refresh -d 'Bypass store read, still write response to store'
complete -c bird -n "__fish_bird_using_subcommand dm" -l no-cache -d 'Disable entity store entirely (no read, no write)'
complete -c bird -n "__fish_bird_using_subcommand dm" -l cache-only -d 'Only serve from local store; never make API requests'
complete -c bird -n "__fish_bird_using_subcommand dm" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c bird -n "__fish_bird_using_subcommand block" -s o -l output -d 'Output format (text, json, jsonl, ndjson). Defaults to json when piped' -r -f -a "text\t'Default: colored, human-readable'
json\t'Machine-readable JSON envelope, no color'
jsonl\t'Streaming line-delimited JSON (one object per line; no wrapper)'
ndjson\t'Newline-delimited JSON, accepted as an alias for jsonl'"
complete -c bird -n "__fish_bird_using_subcommand block" -l color -d 'Color mode: auto (default), always, never' -r -f -a "auto\t'Auto-detect: color when stderr is a TTY and `NO_COLOR` is unset'
always\t'Always emit colors'
never\t'Never emit colors'"
complete -c bird -n "__fish_bird_using_subcommand block" -l timeout -d 'Network timeout in seconds (default 30). Applies to xurl subprocesses' -r
complete -c bird -n "__fish_bird_using_subcommand block" -l limit -d 'Maximum number of results to return on list-style commands (default 100, ceiling 1000)' -r
complete -c bird -n "__fish_bird_using_subcommand block" -l cursor -d 'Pagination cursor token for list-style commands (X API `pagination_token`/`next_token`)' -r
complete -c bird -n "__fish_bird_using_subcommand block" -s f -l force -d 'Skip the interactive confirmation prompt (alias: --yes)'
complete -c bird -n "__fish_bird_using_subcommand block" -l dry-run -d 'Validate inputs and print the would-be request, then exit without calling the API'
complete -c bird -n "__fish_bird_using_subcommand block" -l json -d 'Shorthand for `--output json`'
complete -c bird -n "__fish_bird_using_subcommand block" -l jsonl -d 'Shorthand for `--output jsonl`'
complete -c bird -n "__fish_bird_using_subcommand block" -l plain -d 'Deprecated alias for `--color never` (plain output, no color)'
complete -c bird -n "__fish_bird_using_subcommand block" -l no-color -d 'Deprecated alias for `--color never`'
complete -c bird -n "__fish_bird_using_subcommand block" -s q -l quiet -d 'Suppress informational stderr output (keep only fatal errors)'
complete -c bird -n "__fish_bird_using_subcommand block" -s v -l verbose -d 'Increase verbosity (repeatable: -v info, -vv debug, -vvv trace)'
complete -c bird -n "__fish_bird_using_subcommand block" -l no-interactive -d 'Disable interactive prompts (refuse anything that would block on stdin)'
complete -c bird -n "__fish_bird_using_subcommand block" -l raw -d 'Emit pipe-safe, undecorated text. Ignored in JSON modes'
complete -c bird -n "__fish_bird_using_subcommand block" -l examples -d 'Print curated examples block and exit'
complete -c bird -n "__fish_bird_using_subcommand block" -l refresh -d 'Bypass store read, still write response to store'
complete -c bird -n "__fish_bird_using_subcommand block" -l no-cache -d 'Disable entity store entirely (no read, no write)'
complete -c bird -n "__fish_bird_using_subcommand block" -l cache-only -d 'Only serve from local store; never make API requests'
complete -c bird -n "__fish_bird_using_subcommand block" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c bird -n "__fish_bird_using_subcommand unblock" -s o -l output -d 'Output format (text, json, jsonl, ndjson). Defaults to json when piped' -r -f -a "text\t'Default: colored, human-readable'
json\t'Machine-readable JSON envelope, no color'
jsonl\t'Streaming line-delimited JSON (one object per line; no wrapper)'
ndjson\t'Newline-delimited JSON, accepted as an alias for jsonl'"
complete -c bird -n "__fish_bird_using_subcommand unblock" -l color -d 'Color mode: auto (default), always, never' -r -f -a "auto\t'Auto-detect: color when stderr is a TTY and `NO_COLOR` is unset'
always\t'Always emit colors'
never\t'Never emit colors'"
complete -c bird -n "__fish_bird_using_subcommand unblock" -l timeout -d 'Network timeout in seconds (default 30). Applies to xurl subprocesses' -r
complete -c bird -n "__fish_bird_using_subcommand unblock" -l limit -d 'Maximum number of results to return on list-style commands (default 100, ceiling 1000)' -r
complete -c bird -n "__fish_bird_using_subcommand unblock" -l cursor -d 'Pagination cursor token for list-style commands (X API `pagination_token`/`next_token`)' -r
complete -c bird -n "__fish_bird_using_subcommand unblock" -s f -l force -d 'Skip the interactive confirmation prompt (alias: --yes)'
complete -c bird -n "__fish_bird_using_subcommand unblock" -l dry-run -d 'Validate inputs and print the would-be request, then exit without calling the API'
complete -c bird -n "__fish_bird_using_subcommand unblock" -l json -d 'Shorthand for `--output json`'
complete -c bird -n "__fish_bird_using_subcommand unblock" -l jsonl -d 'Shorthand for `--output jsonl`'
complete -c bird -n "__fish_bird_using_subcommand unblock" -l plain -d 'Deprecated alias for `--color never` (plain output, no color)'
complete -c bird -n "__fish_bird_using_subcommand unblock" -l no-color -d 'Deprecated alias for `--color never`'
complete -c bird -n "__fish_bird_using_subcommand unblock" -s q -l quiet -d 'Suppress informational stderr output (keep only fatal errors)'
complete -c bird -n "__fish_bird_using_subcommand unblock" -s v -l verbose -d 'Increase verbosity (repeatable: -v info, -vv debug, -vvv trace)'
complete -c bird -n "__fish_bird_using_subcommand unblock" -l no-interactive -d 'Disable interactive prompts (refuse anything that would block on stdin)'
complete -c bird -n "__fish_bird_using_subcommand unblock" -l raw -d 'Emit pipe-safe, undecorated text. Ignored in JSON modes'
complete -c bird -n "__fish_bird_using_subcommand unblock" -l examples -d 'Print curated examples block and exit'
complete -c bird -n "__fish_bird_using_subcommand unblock" -l refresh -d 'Bypass store read, still write response to store'
complete -c bird -n "__fish_bird_using_subcommand unblock" -l no-cache -d 'Disable entity store entirely (no read, no write)'
complete -c bird -n "__fish_bird_using_subcommand unblock" -l cache-only -d 'Only serve from local store; never make API requests'
complete -c bird -n "__fish_bird_using_subcommand unblock" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c bird -n "__fish_bird_using_subcommand mute" -s o -l output -d 'Output format (text, json, jsonl, ndjson). Defaults to json when piped' -r -f -a "text\t'Default: colored, human-readable'
json\t'Machine-readable JSON envelope, no color'
jsonl\t'Streaming line-delimited JSON (one object per line; no wrapper)'
ndjson\t'Newline-delimited JSON, accepted as an alias for jsonl'"
complete -c bird -n "__fish_bird_using_subcommand mute" -l color -d 'Color mode: auto (default), always, never' -r -f -a "auto\t'Auto-detect: color when stderr is a TTY and `NO_COLOR` is unset'
always\t'Always emit colors'
never\t'Never emit colors'"
complete -c bird -n "__fish_bird_using_subcommand mute" -l timeout -d 'Network timeout in seconds (default 30). Applies to xurl subprocesses' -r
complete -c bird -n "__fish_bird_using_subcommand mute" -l limit -d 'Maximum number of results to return on list-style commands (default 100, ceiling 1000)' -r
complete -c bird -n "__fish_bird_using_subcommand mute" -l cursor -d 'Pagination cursor token for list-style commands (X API `pagination_token`/`next_token`)' -r
complete -c bird -n "__fish_bird_using_subcommand mute" -s f -l force -d 'Skip the interactive confirmation prompt (alias: --yes)'
complete -c bird -n "__fish_bird_using_subcommand mute" -l dry-run -d 'Validate inputs and print the would-be request, then exit without calling the API'
complete -c bird -n "__fish_bird_using_subcommand mute" -l json -d 'Shorthand for `--output json`'
complete -c bird -n "__fish_bird_using_subcommand mute" -l jsonl -d 'Shorthand for `--output jsonl`'
complete -c bird -n "__fish_bird_using_subcommand mute" -l plain -d 'Deprecated alias for `--color never` (plain output, no color)'
complete -c bird -n "__fish_bird_using_subcommand mute" -l no-color -d 'Deprecated alias for `--color never`'
complete -c bird -n "__fish_bird_using_subcommand mute" -s q -l quiet -d 'Suppress informational stderr output (keep only fatal errors)'
complete -c bird -n "__fish_bird_using_subcommand mute" -s v -l verbose -d 'Increase verbosity (repeatable: -v info, -vv debug, -vvv trace)'
complete -c bird -n "__fish_bird_using_subcommand mute" -l no-interactive -d 'Disable interactive prompts (refuse anything that would block on stdin)'
complete -c bird -n "__fish_bird_using_subcommand mute" -l raw -d 'Emit pipe-safe, undecorated text. Ignored in JSON modes'
complete -c bird -n "__fish_bird_using_subcommand mute" -l examples -d 'Print curated examples block and exit'
complete -c bird -n "__fish_bird_using_subcommand mute" -l refresh -d 'Bypass store read, still write response to store'
complete -c bird -n "__fish_bird_using_subcommand mute" -l no-cache -d 'Disable entity store entirely (no read, no write)'
complete -c bird -n "__fish_bird_using_subcommand mute" -l cache-only -d 'Only serve from local store; never make API requests'
complete -c bird -n "__fish_bird_using_subcommand mute" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c bird -n "__fish_bird_using_subcommand unmute" -s o -l output -d 'Output format (text, json, jsonl, ndjson). Defaults to json when piped' -r -f -a "text\t'Default: colored, human-readable'
json\t'Machine-readable JSON envelope, no color'
jsonl\t'Streaming line-delimited JSON (one object per line; no wrapper)'
ndjson\t'Newline-delimited JSON, accepted as an alias for jsonl'"
complete -c bird -n "__fish_bird_using_subcommand unmute" -l color -d 'Color mode: auto (default), always, never' -r -f -a "auto\t'Auto-detect: color when stderr is a TTY and `NO_COLOR` is unset'
always\t'Always emit colors'
never\t'Never emit colors'"
complete -c bird -n "__fish_bird_using_subcommand unmute" -l timeout -d 'Network timeout in seconds (default 30). Applies to xurl subprocesses' -r
complete -c bird -n "__fish_bird_using_subcommand unmute" -l limit -d 'Maximum number of results to return on list-style commands (default 100, ceiling 1000)' -r
complete -c bird -n "__fish_bird_using_subcommand unmute" -l cursor -d 'Pagination cursor token for list-style commands (X API `pagination_token`/`next_token`)' -r
complete -c bird -n "__fish_bird_using_subcommand unmute" -s f -l force -d 'Skip the interactive confirmation prompt (alias: --yes)'
complete -c bird -n "__fish_bird_using_subcommand unmute" -l dry-run -d 'Validate inputs and print the would-be request, then exit without calling the API'
complete -c bird -n "__fish_bird_using_subcommand unmute" -l json -d 'Shorthand for `--output json`'
complete -c bird -n "__fish_bird_using_subcommand unmute" -l jsonl -d 'Shorthand for `--output jsonl`'
complete -c bird -n "__fish_bird_using_subcommand unmute" -l plain -d 'Deprecated alias for `--color never` (plain output, no color)'
complete -c bird -n "__fish_bird_using_subcommand unmute" -l no-color -d 'Deprecated alias for `--color never`'
complete -c bird -n "__fish_bird_using_subcommand unmute" -s q -l quiet -d 'Suppress informational stderr output (keep only fatal errors)'
complete -c bird -n "__fish_bird_using_subcommand unmute" -s v -l verbose -d 'Increase verbosity (repeatable: -v info, -vv debug, -vvv trace)'
complete -c bird -n "__fish_bird_using_subcommand unmute" -l no-interactive -d 'Disable interactive prompts (refuse anything that would block on stdin)'
complete -c bird -n "__fish_bird_using_subcommand unmute" -l raw -d 'Emit pipe-safe, undecorated text. Ignored in JSON modes'
complete -c bird -n "__fish_bird_using_subcommand unmute" -l examples -d 'Print curated examples block and exit'
complete -c bird -n "__fish_bird_using_subcommand unmute" -l refresh -d 'Bypass store read, still write response to store'
complete -c bird -n "__fish_bird_using_subcommand unmute" -l no-cache -d 'Disable entity store entirely (no read, no write)'
complete -c bird -n "__fish_bird_using_subcommand unmute" -l cache-only -d 'Only serve from local store; never make API requests'
complete -c bird -n "__fish_bird_using_subcommand unmute" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c bird -n "__fish_bird_using_subcommand doctor" -s u -l username -d 'Username for multi-user token selection (maps to xurl -u)' -r
complete -c bird -n "__fish_bird_using_subcommand doctor" -s o -l output -d 'Output format (text, json, jsonl, ndjson). Defaults to json when piped' -r -f -a "text\t'Default: colored, human-readable'
json\t'Machine-readable JSON envelope, no color'
jsonl\t'Streaming line-delimited JSON (one object per line; no wrapper)'
ndjson\t'Newline-delimited JSON, accepted as an alias for jsonl'"
complete -c bird -n "__fish_bird_using_subcommand doctor" -l color -d 'Color mode: auto (default), always, never' -r -f -a "auto\t'Auto-detect: color when stderr is a TTY and `NO_COLOR` is unset'
always\t'Always emit colors'
never\t'Never emit colors'"
complete -c bird -n "__fish_bird_using_subcommand doctor" -l timeout -d 'Network timeout in seconds (default 30). Applies to xurl subprocesses' -r
complete -c bird -n "__fish_bird_using_subcommand doctor" -l limit -d 'Maximum number of results to return on list-style commands (default 100, ceiling 1000)' -r
complete -c bird -n "__fish_bird_using_subcommand doctor" -l cursor -d 'Pagination cursor token for list-style commands (X API `pagination_token`/`next_token`)' -r
complete -c bird -n "__fish_bird_using_subcommand doctor" -l pretty
complete -c bird -n "__fish_bird_using_subcommand doctor" -l json -d 'Shorthand for `--output json`'
complete -c bird -n "__fish_bird_using_subcommand doctor" -l jsonl -d 'Shorthand for `--output jsonl`'
complete -c bird -n "__fish_bird_using_subcommand doctor" -l plain -d 'Deprecated alias for `--color never` (plain output, no color)'
complete -c bird -n "__fish_bird_using_subcommand doctor" -l no-color -d 'Deprecated alias for `--color never`'
complete -c bird -n "__fish_bird_using_subcommand doctor" -s q -l quiet -d 'Suppress informational stderr output (keep only fatal errors)'
complete -c bird -n "__fish_bird_using_subcommand doctor" -s v -l verbose -d 'Increase verbosity (repeatable: -v info, -vv debug, -vvv trace)'
complete -c bird -n "__fish_bird_using_subcommand doctor" -l no-interactive -d 'Disable interactive prompts (refuse anything that would block on stdin)'
complete -c bird -n "__fish_bird_using_subcommand doctor" -l raw -d 'Emit pipe-safe, undecorated text. Ignored in JSON modes'
complete -c bird -n "__fish_bird_using_subcommand doctor" -l examples -d 'Print curated examples block and exit'
complete -c bird -n "__fish_bird_using_subcommand doctor" -l refresh -d 'Bypass store read, still write response to store'
complete -c bird -n "__fish_bird_using_subcommand doctor" -l no-cache -d 'Disable entity store entirely (no read, no write)'
complete -c bird -n "__fish_bird_using_subcommand doctor" -l cache-only -d 'Only serve from local store; never make API requests'
complete -c bird -n "__fish_bird_using_subcommand doctor" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c bird -n "__fish_bird_using_subcommand cache; and not __fish_seen_subcommand_from clear stats help" -s u -l username -d 'Username for multi-user token selection (maps to xurl -u)' -r
complete -c bird -n "__fish_bird_using_subcommand cache; and not __fish_seen_subcommand_from clear stats help" -s o -l output -d 'Output format (text, json, jsonl, ndjson). Defaults to json when piped' -r -f -a "text\t'Default: colored, human-readable'
json\t'Machine-readable JSON envelope, no color'
jsonl\t'Streaming line-delimited JSON (one object per line; no wrapper)'
ndjson\t'Newline-delimited JSON, accepted as an alias for jsonl'"
complete -c bird -n "__fish_bird_using_subcommand cache; and not __fish_seen_subcommand_from clear stats help" -l color -d 'Color mode: auto (default), always, never' -r -f -a "auto\t'Auto-detect: color when stderr is a TTY and `NO_COLOR` is unset'
always\t'Always emit colors'
never\t'Never emit colors'"
complete -c bird -n "__fish_bird_using_subcommand cache; and not __fish_seen_subcommand_from clear stats help" -l timeout -d 'Network timeout in seconds (default 30). Applies to xurl subprocesses' -r
complete -c bird -n "__fish_bird_using_subcommand cache; and not __fish_seen_subcommand_from clear stats help" -l limit -d 'Maximum number of results to return on list-style commands (default 100, ceiling 1000)' -r
complete -c bird -n "__fish_bird_using_subcommand cache; and not __fish_seen_subcommand_from clear stats help" -l cursor -d 'Pagination cursor token for list-style commands (X API `pagination_token`/`next_token`)' -r
complete -c bird -n "__fish_bird_using_subcommand cache; and not __fish_seen_subcommand_from clear stats help" -l json -d 'Shorthand for `--output json`'
complete -c bird -n "__fish_bird_using_subcommand cache; and not __fish_seen_subcommand_from clear stats help" -l jsonl -d 'Shorthand for `--output jsonl`'
complete -c bird -n "__fish_bird_using_subcommand cache; and not __fish_seen_subcommand_from clear stats help" -l plain -d 'Deprecated alias for `--color never` (plain output, no color)'
complete -c bird -n "__fish_bird_using_subcommand cache; and not __fish_seen_subcommand_from clear stats help" -l no-color -d 'Deprecated alias for `--color never`'
complete -c bird -n "__fish_bird_using_subcommand cache; and not __fish_seen_subcommand_from clear stats help" -s q -l quiet -d 'Suppress informational stderr output (keep only fatal errors)'
complete -c bird -n "__fish_bird_using_subcommand cache; and not __fish_seen_subcommand_from clear stats help" -s v -l verbose -d 'Increase verbosity (repeatable: -v info, -vv debug, -vvv trace)'
complete -c bird -n "__fish_bird_using_subcommand cache; and not __fish_seen_subcommand_from clear stats help" -l no-interactive -d 'Disable interactive prompts (refuse anything that would block on stdin)'
complete -c bird -n "__fish_bird_using_subcommand cache; and not __fish_seen_subcommand_from clear stats help" -l raw -d 'Emit pipe-safe, undecorated text. Ignored in JSON modes'
complete -c bird -n "__fish_bird_using_subcommand cache; and not __fish_seen_subcommand_from clear stats help" -l examples -d 'Print curated examples block and exit'
complete -c bird -n "__fish_bird_using_subcommand cache; and not __fish_seen_subcommand_from clear stats help" -l refresh -d 'Bypass store read, still write response to store'
complete -c bird -n "__fish_bird_using_subcommand cache; and not __fish_seen_subcommand_from clear stats help" -l no-cache -d 'Disable entity store entirely (no read, no write)'
complete -c bird -n "__fish_bird_using_subcommand cache; and not __fish_seen_subcommand_from clear stats help" -l cache-only -d 'Only serve from local store; never make API requests'
complete -c bird -n "__fish_bird_using_subcommand cache; and not __fish_seen_subcommand_from clear stats help" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c bird -n "__fish_bird_using_subcommand cache; and not __fish_seen_subcommand_from clear stats help" -f -a "clear" -d 'Delete all cache entries'
complete -c bird -n "__fish_bird_using_subcommand cache; and not __fish_seen_subcommand_from clear stats help" -f -a "stats" -d 'Show cache status (JSON default, --pretty for human-readable)'
complete -c bird -n "__fish_bird_using_subcommand cache; and not __fish_seen_subcommand_from clear stats help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c bird -n "__fish_bird_using_subcommand cache; and __fish_seen_subcommand_from clear" -s u -l username -d 'Username for multi-user token selection (maps to xurl -u)' -r
complete -c bird -n "__fish_bird_using_subcommand cache; and __fish_seen_subcommand_from clear" -s o -l output -d 'Output format (text, json, jsonl, ndjson). Defaults to json when piped' -r -f -a "text\t'Default: colored, human-readable'
json\t'Machine-readable JSON envelope, no color'
jsonl\t'Streaming line-delimited JSON (one object per line; no wrapper)'
ndjson\t'Newline-delimited JSON, accepted as an alias for jsonl'"
complete -c bird -n "__fish_bird_using_subcommand cache; and __fish_seen_subcommand_from clear" -l color -d 'Color mode: auto (default), always, never' -r -f -a "auto\t'Auto-detect: color when stderr is a TTY and `NO_COLOR` is unset'
always\t'Always emit colors'
never\t'Never emit colors'"
complete -c bird -n "__fish_bird_using_subcommand cache; and __fish_seen_subcommand_from clear" -l timeout -d 'Network timeout in seconds (default 30). Applies to xurl subprocesses' -r
complete -c bird -n "__fish_bird_using_subcommand cache; and __fish_seen_subcommand_from clear" -l limit -d 'Maximum number of results to return on list-style commands (default 100, ceiling 1000)' -r
complete -c bird -n "__fish_bird_using_subcommand cache; and __fish_seen_subcommand_from clear" -l cursor -d 'Pagination cursor token for list-style commands (X API `pagination_token`/`next_token`)' -r
complete -c bird -n "__fish_bird_using_subcommand cache; and __fish_seen_subcommand_from clear" -s f -l force -d 'Skip the interactive confirmation prompt (alias: --yes)'
complete -c bird -n "__fish_bird_using_subcommand cache; and __fish_seen_subcommand_from clear" -l dry-run -d 'Validate inputs and print the would-be request, then exit without calling the API'
complete -c bird -n "__fish_bird_using_subcommand cache; and __fish_seen_subcommand_from clear" -l json -d 'Shorthand for `--output json`'
complete -c bird -n "__fish_bird_using_subcommand cache; and __fish_seen_subcommand_from clear" -l jsonl -d 'Shorthand for `--output jsonl`'
complete -c bird -n "__fish_bird_using_subcommand cache; and __fish_seen_subcommand_from clear" -l plain -d 'Deprecated alias for `--color never` (plain output, no color)'
complete -c bird -n "__fish_bird_using_subcommand cache; and __fish_seen_subcommand_from clear" -l no-color -d 'Deprecated alias for `--color never`'
complete -c bird -n "__fish_bird_using_subcommand cache; and __fish_seen_subcommand_from clear" -s q -l quiet -d 'Suppress informational stderr output (keep only fatal errors)'
complete -c bird -n "__fish_bird_using_subcommand cache; and __fish_seen_subcommand_from clear" -s v -l verbose -d 'Increase verbosity (repeatable: -v info, -vv debug, -vvv trace)'
complete -c bird -n "__fish_bird_using_subcommand cache; and __fish_seen_subcommand_from clear" -l no-interactive -d 'Disable interactive prompts (refuse anything that would block on stdin)'
complete -c bird -n "__fish_bird_using_subcommand cache; and __fish_seen_subcommand_from clear" -l raw -d 'Emit pipe-safe, undecorated text. Ignored in JSON modes'
complete -c bird -n "__fish_bird_using_subcommand cache; and __fish_seen_subcommand_from clear" -l examples -d 'Print curated examples block and exit'
complete -c bird -n "__fish_bird_using_subcommand cache; and __fish_seen_subcommand_from clear" -l refresh -d 'Bypass store read, still write response to store'
complete -c bird -n "__fish_bird_using_subcommand cache; and __fish_seen_subcommand_from clear" -l no-cache -d 'Disable entity store entirely (no read, no write)'
complete -c bird -n "__fish_bird_using_subcommand cache; and __fish_seen_subcommand_from clear" -l cache-only -d 'Only serve from local store; never make API requests'
complete -c bird -n "__fish_bird_using_subcommand cache; and __fish_seen_subcommand_from clear" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c bird -n "__fish_bird_using_subcommand cache; and __fish_seen_subcommand_from stats" -s u -l username -d 'Username for multi-user token selection (maps to xurl -u)' -r
complete -c bird -n "__fish_bird_using_subcommand cache; and __fish_seen_subcommand_from stats" -s o -l output -d 'Output format (text, json, jsonl, ndjson). Defaults to json when piped' -r -f -a "text\t'Default: colored, human-readable'
json\t'Machine-readable JSON envelope, no color'
jsonl\t'Streaming line-delimited JSON (one object per line; no wrapper)'
ndjson\t'Newline-delimited JSON, accepted as an alias for jsonl'"
complete -c bird -n "__fish_bird_using_subcommand cache; and __fish_seen_subcommand_from stats" -l color -d 'Color mode: auto (default), always, never' -r -f -a "auto\t'Auto-detect: color when stderr is a TTY and `NO_COLOR` is unset'
always\t'Always emit colors'
never\t'Never emit colors'"
complete -c bird -n "__fish_bird_using_subcommand cache; and __fish_seen_subcommand_from stats" -l timeout -d 'Network timeout in seconds (default 30). Applies to xurl subprocesses' -r
complete -c bird -n "__fish_bird_using_subcommand cache; and __fish_seen_subcommand_from stats" -l limit -d 'Maximum number of results to return on list-style commands (default 100, ceiling 1000)' -r
complete -c bird -n "__fish_bird_using_subcommand cache; and __fish_seen_subcommand_from stats" -l cursor -d 'Pagination cursor token for list-style commands (X API `pagination_token`/`next_token`)' -r
complete -c bird -n "__fish_bird_using_subcommand cache; and __fish_seen_subcommand_from stats" -l pretty
complete -c bird -n "__fish_bird_using_subcommand cache; and __fish_seen_subcommand_from stats" -l json -d 'Shorthand for `--output json`'
complete -c bird -n "__fish_bird_using_subcommand cache; and __fish_seen_subcommand_from stats" -l jsonl -d 'Shorthand for `--output jsonl`'
complete -c bird -n "__fish_bird_using_subcommand cache; and __fish_seen_subcommand_from stats" -l plain -d 'Deprecated alias for `--color never` (plain output, no color)'
complete -c bird -n "__fish_bird_using_subcommand cache; and __fish_seen_subcommand_from stats" -l no-color -d 'Deprecated alias for `--color never`'
complete -c bird -n "__fish_bird_using_subcommand cache; and __fish_seen_subcommand_from stats" -s q -l quiet -d 'Suppress informational stderr output (keep only fatal errors)'
complete -c bird -n "__fish_bird_using_subcommand cache; and __fish_seen_subcommand_from stats" -s v -l verbose -d 'Increase verbosity (repeatable: -v info, -vv debug, -vvv trace)'
complete -c bird -n "__fish_bird_using_subcommand cache; and __fish_seen_subcommand_from stats" -l no-interactive -d 'Disable interactive prompts (refuse anything that would block on stdin)'
complete -c bird -n "__fish_bird_using_subcommand cache; and __fish_seen_subcommand_from stats" -l raw -d 'Emit pipe-safe, undecorated text. Ignored in JSON modes'
complete -c bird -n "__fish_bird_using_subcommand cache; and __fish_seen_subcommand_from stats" -l examples -d 'Print curated examples block and exit'
complete -c bird -n "__fish_bird_using_subcommand cache; and __fish_seen_subcommand_from stats" -l refresh -d 'Bypass store read, still write response to store'
complete -c bird -n "__fish_bird_using_subcommand cache; and __fish_seen_subcommand_from stats" -l no-cache -d 'Disable entity store entirely (no read, no write)'
complete -c bird -n "__fish_bird_using_subcommand cache; and __fish_seen_subcommand_from stats" -l cache-only -d 'Only serve from local store; never make API requests'
complete -c bird -n "__fish_bird_using_subcommand cache; and __fish_seen_subcommand_from stats" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c bird -n "__fish_bird_using_subcommand cache; and __fish_seen_subcommand_from help" -f -a "clear" -d 'Delete all cache entries'
complete -c bird -n "__fish_bird_using_subcommand cache; and __fish_seen_subcommand_from help" -f -a "stats" -d 'Show cache status (JSON default, --pretty for human-readable)'
complete -c bird -n "__fish_bird_using_subcommand cache; and __fish_seen_subcommand_from help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c bird -n "__fish_bird_using_subcommand completions" -s u -l username -d 'Username for multi-user token selection (maps to xurl -u)' -r
complete -c bird -n "__fish_bird_using_subcommand completions" -s o -l output -d 'Output format (text, json, jsonl, ndjson). Defaults to json when piped' -r -f -a "text\t'Default: colored, human-readable'
json\t'Machine-readable JSON envelope, no color'
jsonl\t'Streaming line-delimited JSON (one object per line; no wrapper)'
ndjson\t'Newline-delimited JSON, accepted as an alias for jsonl'"
complete -c bird -n "__fish_bird_using_subcommand completions" -l color -d 'Color mode: auto (default), always, never' -r -f -a "auto\t'Auto-detect: color when stderr is a TTY and `NO_COLOR` is unset'
always\t'Always emit colors'
never\t'Never emit colors'"
complete -c bird -n "__fish_bird_using_subcommand completions" -l timeout -d 'Network timeout in seconds (default 30). Applies to xurl subprocesses' -r
complete -c bird -n "__fish_bird_using_subcommand completions" -l limit -d 'Maximum number of results to return on list-style commands (default 100, ceiling 1000)' -r
complete -c bird -n "__fish_bird_using_subcommand completions" -l cursor -d 'Pagination cursor token for list-style commands (X API `pagination_token`/`next_token`)' -r
complete -c bird -n "__fish_bird_using_subcommand completions" -l json -d 'Shorthand for `--output json`'
complete -c bird -n "__fish_bird_using_subcommand completions" -l jsonl -d 'Shorthand for `--output jsonl`'
complete -c bird -n "__fish_bird_using_subcommand completions" -l plain -d 'Deprecated alias for `--color never` (plain output, no color)'
complete -c bird -n "__fish_bird_using_subcommand completions" -l no-color -d 'Deprecated alias for `--color never`'
complete -c bird -n "__fish_bird_using_subcommand completions" -s q -l quiet -d 'Suppress informational stderr output (keep only fatal errors)'
complete -c bird -n "__fish_bird_using_subcommand completions" -s v -l verbose -d 'Increase verbosity (repeatable: -v info, -vv debug, -vvv trace)'
complete -c bird -n "__fish_bird_using_subcommand completions" -l no-interactive -d 'Disable interactive prompts (refuse anything that would block on stdin)'
complete -c bird -n "__fish_bird_using_subcommand completions" -l raw -d 'Emit pipe-safe, undecorated text. Ignored in JSON modes'
complete -c bird -n "__fish_bird_using_subcommand completions" -l examples -d 'Print curated examples block and exit'
complete -c bird -n "__fish_bird_using_subcommand completions" -l refresh -d 'Bypass store read, still write response to store'
complete -c bird -n "__fish_bird_using_subcommand completions" -l no-cache -d 'Disable entity store entirely (no read, no write)'
complete -c bird -n "__fish_bird_using_subcommand completions" -l cache-only -d 'Only serve from local store; never make API requests'
complete -c bird -n "__fish_bird_using_subcommand completions" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c bird -n "__fish_bird_using_subcommand skill; and not __fish_seen_subcommand_from install update help" -s u -l username -d 'Username for multi-user token selection (maps to xurl -u)' -r
complete -c bird -n "__fish_bird_using_subcommand skill; and not __fish_seen_subcommand_from install update help" -s o -l output -d 'Output format (text, json, jsonl, ndjson). Defaults to json when piped' -r -f -a "text\t'Default: colored, human-readable'
json\t'Machine-readable JSON envelope, no color'
jsonl\t'Streaming line-delimited JSON (one object per line; no wrapper)'
ndjson\t'Newline-delimited JSON, accepted as an alias for jsonl'"
complete -c bird -n "__fish_bird_using_subcommand skill; and not __fish_seen_subcommand_from install update help" -l color -d 'Color mode: auto (default), always, never' -r -f -a "auto\t'Auto-detect: color when stderr is a TTY and `NO_COLOR` is unset'
always\t'Always emit colors'
never\t'Never emit colors'"
complete -c bird -n "__fish_bird_using_subcommand skill; and not __fish_seen_subcommand_from install update help" -l timeout -d 'Network timeout in seconds (default 30). Applies to xurl subprocesses' -r
complete -c bird -n "__fish_bird_using_subcommand skill; and not __fish_seen_subcommand_from install update help" -l limit -d 'Maximum number of results to return on list-style commands (default 100, ceiling 1000)' -r
complete -c bird -n "__fish_bird_using_subcommand skill; and not __fish_seen_subcommand_from install update help" -l cursor -d 'Pagination cursor token for list-style commands (X API `pagination_token`/`next_token`)' -r
complete -c bird -n "__fish_bird_using_subcommand skill; and not __fish_seen_subcommand_from install update help" -l json -d 'Shorthand for `--output json`'
complete -c bird -n "__fish_bird_using_subcommand skill; and not __fish_seen_subcommand_from install update help" -l jsonl -d 'Shorthand for `--output jsonl`'
complete -c bird -n "__fish_bird_using_subcommand skill; and not __fish_seen_subcommand_from install update help" -l plain -d 'Deprecated alias for `--color never` (plain output, no color)'
complete -c bird -n "__fish_bird_using_subcommand skill; and not __fish_seen_subcommand_from install update help" -l no-color -d 'Deprecated alias for `--color never`'
complete -c bird -n "__fish_bird_using_subcommand skill; and not __fish_seen_subcommand_from install update help" -s q -l quiet -d 'Suppress informational stderr output (keep only fatal errors)'
complete -c bird -n "__fish_bird_using_subcommand skill; and not __fish_seen_subcommand_from install update help" -s v -l verbose -d 'Increase verbosity (repeatable: -v info, -vv debug, -vvv trace)'
complete -c bird -n "__fish_bird_using_subcommand skill; and not __fish_seen_subcommand_from install update help" -l no-interactive -d 'Disable interactive prompts (refuse anything that would block on stdin)'
complete -c bird -n "__fish_bird_using_subcommand skill; and not __fish_seen_subcommand_from install update help" -l raw -d 'Emit pipe-safe, undecorated text. Ignored in JSON modes'
complete -c bird -n "__fish_bird_using_subcommand skill; and not __fish_seen_subcommand_from install update help" -l examples -d 'Print curated examples block and exit'
complete -c bird -n "__fish_bird_using_subcommand skill; and not __fish_seen_subcommand_from install update help" -l refresh -d 'Bypass store read, still write response to store'
complete -c bird -n "__fish_bird_using_subcommand skill; and not __fish_seen_subcommand_from install update help" -l no-cache -d 'Disable entity store entirely (no read, no write)'
complete -c bird -n "__fish_bird_using_subcommand skill; and not __fish_seen_subcommand_from install update help" -l cache-only -d 'Only serve from local store; never make API requests'
complete -c bird -n "__fish_bird_using_subcommand skill; and not __fish_seen_subcommand_from install update help" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c bird -n "__fish_bird_using_subcommand skill; and not __fish_seen_subcommand_from install update help" -f -a "install" -d 'Install the bird skill bundle into a host\'s canonical skills directory'
complete -c bird -n "__fish_bird_using_subcommand skill; and not __fish_seen_subcommand_from install update help" -f -a "update" -d 'Update the installed bird skill bundle to the embedded version'
complete -c bird -n "__fish_bird_using_subcommand skill; and not __fish_seen_subcommand_from install update help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c bird -n "__fish_bird_using_subcommand skill; and __fish_seen_subcommand_from install" -l host -d 'Target host (default: claude-code). Mutually exclusive with --all' -r -f -a "claude-code\t'Claude Code (`~/.claude/skills/bird/`)'"
complete -c bird -n "__fish_bird_using_subcommand skill; and __fish_seen_subcommand_from install" -s u -l username -d 'Username for multi-user token selection (maps to xurl -u)' -r
complete -c bird -n "__fish_bird_using_subcommand skill; and __fish_seen_subcommand_from install" -s o -l output -d 'Output format (text, json, jsonl, ndjson). Defaults to json when piped' -r -f -a "text\t'Default: colored, human-readable'
json\t'Machine-readable JSON envelope, no color'
jsonl\t'Streaming line-delimited JSON (one object per line; no wrapper)'
ndjson\t'Newline-delimited JSON, accepted as an alias for jsonl'"
complete -c bird -n "__fish_bird_using_subcommand skill; and __fish_seen_subcommand_from install" -l color -d 'Color mode: auto (default), always, never' -r -f -a "auto\t'Auto-detect: color when stderr is a TTY and `NO_COLOR` is unset'
always\t'Always emit colors'
never\t'Never emit colors'"
complete -c bird -n "__fish_bird_using_subcommand skill; and __fish_seen_subcommand_from install" -l timeout -d 'Network timeout in seconds (default 30). Applies to xurl subprocesses' -r
complete -c bird -n "__fish_bird_using_subcommand skill; and __fish_seen_subcommand_from install" -l limit -d 'Maximum number of results to return on list-style commands (default 100, ceiling 1000)' -r
complete -c bird -n "__fish_bird_using_subcommand skill; and __fish_seen_subcommand_from install" -l cursor -d 'Pagination cursor token for list-style commands (X API `pagination_token`/`next_token`)' -r
complete -c bird -n "__fish_bird_using_subcommand skill; and __fish_seen_subcommand_from install" -l all -d 'Install into every supported host in one invocation'
complete -c bird -n "__fish_bird_using_subcommand skill; and __fish_seen_subcommand_from install" -l dry-run -d 'Print the planned destination without writing'
complete -c bird -n "__fish_bird_using_subcommand skill; and __fish_seen_subcommand_from install" -l json -d 'Shorthand for `--output json`'
complete -c bird -n "__fish_bird_using_subcommand skill; and __fish_seen_subcommand_from install" -l jsonl -d 'Shorthand for `--output jsonl`'
complete -c bird -n "__fish_bird_using_subcommand skill; and __fish_seen_subcommand_from install" -l plain -d 'Deprecated alias for `--color never` (plain output, no color)'
complete -c bird -n "__fish_bird_using_subcommand skill; and __fish_seen_subcommand_from install" -l no-color -d 'Deprecated alias for `--color never`'
complete -c bird -n "__fish_bird_using_subcommand skill; and __fish_seen_subcommand_from install" -s q -l quiet -d 'Suppress informational stderr output (keep only fatal errors)'
complete -c bird -n "__fish_bird_using_subcommand skill; and __fish_seen_subcommand_from install" -s v -l verbose -d 'Increase verbosity (repeatable: -v info, -vv debug, -vvv trace)'
complete -c bird -n "__fish_bird_using_subcommand skill; and __fish_seen_subcommand_from install" -l no-interactive -d 'Disable interactive prompts (refuse anything that would block on stdin)'
complete -c bird -n "__fish_bird_using_subcommand skill; and __fish_seen_subcommand_from install" -l raw -d 'Emit pipe-safe, undecorated text. Ignored in JSON modes'
complete -c bird -n "__fish_bird_using_subcommand skill; and __fish_seen_subcommand_from install" -l examples -d 'Print curated examples block and exit'
complete -c bird -n "__fish_bird_using_subcommand skill; and __fish_seen_subcommand_from install" -l refresh -d 'Bypass store read, still write response to store'
complete -c bird -n "__fish_bird_using_subcommand skill; and __fish_seen_subcommand_from install" -l no-cache -d 'Disable entity store entirely (no read, no write)'
complete -c bird -n "__fish_bird_using_subcommand skill; and __fish_seen_subcommand_from install" -l cache-only -d 'Only serve from local store; never make API requests'
complete -c bird -n "__fish_bird_using_subcommand skill; and __fish_seen_subcommand_from install" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c bird -n "__fish_bird_using_subcommand skill; and __fish_seen_subcommand_from update" -l host -d 'Target host (default: claude-code). Mutually exclusive with --all' -r -f -a "claude-code\t'Claude Code (`~/.claude/skills/bird/`)'"
complete -c bird -n "__fish_bird_using_subcommand skill; and __fish_seen_subcommand_from update" -s u -l username -d 'Username for multi-user token selection (maps to xurl -u)' -r
complete -c bird -n "__fish_bird_using_subcommand skill; and __fish_seen_subcommand_from update" -s o -l output -d 'Output format (text, json, jsonl, ndjson). Defaults to json when piped' -r -f -a "text\t'Default: colored, human-readable'
json\t'Machine-readable JSON envelope, no color'
jsonl\t'Streaming line-delimited JSON (one object per line; no wrapper)'
ndjson\t'Newline-delimited JSON, accepted as an alias for jsonl'"
complete -c bird -n "__fish_bird_using_subcommand skill; and __fish_seen_subcommand_from update" -l color -d 'Color mode: auto (default), always, never' -r -f -a "auto\t'Auto-detect: color when stderr is a TTY and `NO_COLOR` is unset'
always\t'Always emit colors'
never\t'Never emit colors'"
complete -c bird -n "__fish_bird_using_subcommand skill; and __fish_seen_subcommand_from update" -l timeout -d 'Network timeout in seconds (default 30). Applies to xurl subprocesses' -r
complete -c bird -n "__fish_bird_using_subcommand skill; and __fish_seen_subcommand_from update" -l limit -d 'Maximum number of results to return on list-style commands (default 100, ceiling 1000)' -r
complete -c bird -n "__fish_bird_using_subcommand skill; and __fish_seen_subcommand_from update" -l cursor -d 'Pagination cursor token for list-style commands (X API `pagination_token`/`next_token`)' -r
complete -c bird -n "__fish_bird_using_subcommand skill; and __fish_seen_subcommand_from update" -l all -d 'Update every supported host in one invocation'
complete -c bird -n "__fish_bird_using_subcommand skill; and __fish_seen_subcommand_from update" -l dry-run -d 'Print the planned destination without writing'
complete -c bird -n "__fish_bird_using_subcommand skill; and __fish_seen_subcommand_from update" -l json -d 'Shorthand for `--output json`'
complete -c bird -n "__fish_bird_using_subcommand skill; and __fish_seen_subcommand_from update" -l jsonl -d 'Shorthand for `--output jsonl`'
complete -c bird -n "__fish_bird_using_subcommand skill; and __fish_seen_subcommand_from update" -l plain -d 'Deprecated alias for `--color never` (plain output, no color)'
complete -c bird -n "__fish_bird_using_subcommand skill; and __fish_seen_subcommand_from update" -l no-color -d 'Deprecated alias for `--color never`'
complete -c bird -n "__fish_bird_using_subcommand skill; and __fish_seen_subcommand_from update" -s q -l quiet -d 'Suppress informational stderr output (keep only fatal errors)'
complete -c bird -n "__fish_bird_using_subcommand skill; and __fish_seen_subcommand_from update" -s v -l verbose -d 'Increase verbosity (repeatable: -v info, -vv debug, -vvv trace)'
complete -c bird -n "__fish_bird_using_subcommand skill; and __fish_seen_subcommand_from update" -l no-interactive -d 'Disable interactive prompts (refuse anything that would block on stdin)'
complete -c bird -n "__fish_bird_using_subcommand skill; and __fish_seen_subcommand_from update" -l raw -d 'Emit pipe-safe, undecorated text. Ignored in JSON modes'
complete -c bird -n "__fish_bird_using_subcommand skill; and __fish_seen_subcommand_from update" -l examples -d 'Print curated examples block and exit'
complete -c bird -n "__fish_bird_using_subcommand skill; and __fish_seen_subcommand_from update" -l refresh -d 'Bypass store read, still write response to store'
complete -c bird -n "__fish_bird_using_subcommand skill; and __fish_seen_subcommand_from update" -l no-cache -d 'Disable entity store entirely (no read, no write)'
complete -c bird -n "__fish_bird_using_subcommand skill; and __fish_seen_subcommand_from update" -l cache-only -d 'Only serve from local store; never make API requests'
complete -c bird -n "__fish_bird_using_subcommand skill; and __fish_seen_subcommand_from update" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c bird -n "__fish_bird_using_subcommand skill; and __fish_seen_subcommand_from help" -f -a "install" -d 'Install the bird skill bundle into a host\'s canonical skills directory'
complete -c bird -n "__fish_bird_using_subcommand skill; and __fish_seen_subcommand_from help" -f -a "update" -d 'Update the installed bird skill bundle to the embedded version'
complete -c bird -n "__fish_bird_using_subcommand skill; and __fish_seen_subcommand_from help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c bird -n "__fish_bird_using_subcommand schema" -s u -l username -d 'Username for multi-user token selection (maps to xurl -u)' -r
complete -c bird -n "__fish_bird_using_subcommand schema" -s o -l output -d 'Output format (text, json, jsonl, ndjson). Defaults to json when piped' -r -f -a "text\t'Default: colored, human-readable'
json\t'Machine-readable JSON envelope, no color'
jsonl\t'Streaming line-delimited JSON (one object per line; no wrapper)'
ndjson\t'Newline-delimited JSON, accepted as an alias for jsonl'"
complete -c bird -n "__fish_bird_using_subcommand schema" -l color -d 'Color mode: auto (default), always, never' -r -f -a "auto\t'Auto-detect: color when stderr is a TTY and `NO_COLOR` is unset'
always\t'Always emit colors'
never\t'Never emit colors'"
complete -c bird -n "__fish_bird_using_subcommand schema" -l timeout -d 'Network timeout in seconds (default 30). Applies to xurl subprocesses' -r
complete -c bird -n "__fish_bird_using_subcommand schema" -l limit -d 'Maximum number of results to return on list-style commands (default 100, ceiling 1000)' -r
complete -c bird -n "__fish_bird_using_subcommand schema" -l cursor -d 'Pagination cursor token for list-style commands (X API `pagination_token`/`next_token`)' -r
complete -c bird -n "__fish_bird_using_subcommand schema" -l list -d 'List all available schema names instead of printing a schema'
complete -c bird -n "__fish_bird_using_subcommand schema" -l json -d 'Shorthand for `--output json`'
complete -c bird -n "__fish_bird_using_subcommand schema" -l jsonl -d 'Shorthand for `--output jsonl`'
complete -c bird -n "__fish_bird_using_subcommand schema" -l plain -d 'Deprecated alias for `--color never` (plain output, no color)'
complete -c bird -n "__fish_bird_using_subcommand schema" -l no-color -d 'Deprecated alias for `--color never`'
complete -c bird -n "__fish_bird_using_subcommand schema" -s q -l quiet -d 'Suppress informational stderr output (keep only fatal errors)'
complete -c bird -n "__fish_bird_using_subcommand schema" -s v -l verbose -d 'Increase verbosity (repeatable: -v info, -vv debug, -vvv trace)'
complete -c bird -n "__fish_bird_using_subcommand schema" -l no-interactive -d 'Disable interactive prompts (refuse anything that would block on stdin)'
complete -c bird -n "__fish_bird_using_subcommand schema" -l raw -d 'Emit pipe-safe, undecorated text. Ignored in JSON modes'
complete -c bird -n "__fish_bird_using_subcommand schema" -l examples -d 'Print curated examples block and exit'
complete -c bird -n "__fish_bird_using_subcommand schema" -l refresh -d 'Bypass store read, still write response to store'
complete -c bird -n "__fish_bird_using_subcommand schema" -l no-cache -d 'Disable entity store entirely (no read, no write)'
complete -c bird -n "__fish_bird_using_subcommand schema" -l cache-only -d 'Only serve from local store; never make API requests'
complete -c bird -n "__fish_bird_using_subcommand schema" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c bird -n "__fish_bird_using_subcommand help; and not __fish_seen_subcommand_from login me get post put bookmarks profile search thread delete watchlist usage tweet reply like unlike repost unrepost follow unfollow dm block unblock mute unmute doctor cache completions skill schema help" -f -a "login" -d 'Authenticate via xurl (OAuth2 PKCE browser flow)'
complete -c bird -n "__fish_bird_using_subcommand help; and not __fish_seen_subcommand_from login me get post put bookmarks profile search thread delete watchlist usage tweet reply like unlike repost unrepost follow unfollow dm block unblock mute unmute doctor cache completions skill schema help" -f -a "me" -d 'Show current user (GET /2/users/me)'
complete -c bird -n "__fish_bird_using_subcommand help; and not __fish_seen_subcommand_from login me get post put bookmarks profile search thread delete watchlist usage tweet reply like unlike repost unrepost follow unfollow dm block unblock mute unmute doctor cache completions skill schema help" -f -a "get" -d 'GET request to path (e.g. /2/users/me or /2/users/{id}/bookmarks with -p id=123)'
complete -c bird -n "__fish_bird_using_subcommand help; and not __fish_seen_subcommand_from login me get post put bookmarks profile search thread delete watchlist usage tweet reply like unlike repost unrepost follow unfollow dm block unblock mute unmute doctor cache completions skill schema help" -f -a "post" -d 'POST request to path'
complete -c bird -n "__fish_bird_using_subcommand help; and not __fish_seen_subcommand_from login me get post put bookmarks profile search thread delete watchlist usage tweet reply like unlike repost unrepost follow unfollow dm block unblock mute unmute doctor cache completions skill schema help" -f -a "put" -d 'PUT request to path'
complete -c bird -n "__fish_bird_using_subcommand help; and not __fish_seen_subcommand_from login me get post put bookmarks profile search thread delete watchlist usage tweet reply like unlike repost unrepost follow unfollow dm block unblock mute unmute doctor cache completions skill schema help" -f -a "bookmarks" -d 'List bookmarks for the current user (paginated, max_results=100)'
complete -c bird -n "__fish_bird_using_subcommand help; and not __fish_seen_subcommand_from login me get post put bookmarks profile search thread delete watchlist usage tweet reply like unlike repost unrepost follow unfollow dm block unblock mute unmute doctor cache completions skill schema help" -f -a "profile" -d 'Look up a user profile by username'
complete -c bird -n "__fish_bird_using_subcommand help; and not __fish_seen_subcommand_from login me get post put bookmarks profile search thread delete watchlist usage tweet reply like unlike repost unrepost follow unfollow dm block unblock mute unmute doctor cache completions skill schema help" -f -a "search" -d 'Search recent tweets (GET /2/tweets/search/recent)'
complete -c bird -n "__fish_bird_using_subcommand help; and not __fish_seen_subcommand_from login me get post put bookmarks profile search thread delete watchlist usage tweet reply like unlike repost unrepost follow unfollow dm block unblock mute unmute doctor cache completions skill schema help" -f -a "thread" -d 'Reconstruct a conversation thread from a tweet'
complete -c bird -n "__fish_bird_using_subcommand help; and not __fish_seen_subcommand_from login me get post put bookmarks profile search thread delete watchlist usage tweet reply like unlike repost unrepost follow unfollow dm block unblock mute unmute doctor cache completions skill schema help" -f -a "delete" -d 'DELETE request to path'
complete -c bird -n "__fish_bird_using_subcommand help; and not __fish_seen_subcommand_from login me get post put bookmarks profile search thread delete watchlist usage tweet reply like unlike repost unrepost follow unfollow dm block unblock mute unmute doctor cache completions skill schema help" -f -a "watchlist" -d 'Monitor users: check recent activity, manage watchlist'
complete -c bird -n "__fish_bird_using_subcommand help; and not __fish_seen_subcommand_from login me get post put bookmarks profile search thread delete watchlist usage tweet reply like unlike repost unrepost follow unfollow dm block unblock mute unmute doctor cache completions skill schema help" -f -a "usage" -d 'View API usage and costs'
complete -c bird -n "__fish_bird_using_subcommand help; and not __fish_seen_subcommand_from login me get post put bookmarks profile search thread delete watchlist usage tweet reply like unlike repost unrepost follow unfollow dm block unblock mute unmute doctor cache completions skill schema help" -f -a "tweet" -d 'Post a tweet (via xurl)'
complete -c bird -n "__fish_bird_using_subcommand help; and not __fish_seen_subcommand_from login me get post put bookmarks profile search thread delete watchlist usage tweet reply like unlike repost unrepost follow unfollow dm block unblock mute unmute doctor cache completions skill schema help" -f -a "reply" -d 'Reply to a tweet (via xurl)'
complete -c bird -n "__fish_bird_using_subcommand help; and not __fish_seen_subcommand_from login me get post put bookmarks profile search thread delete watchlist usage tweet reply like unlike repost unrepost follow unfollow dm block unblock mute unmute doctor cache completions skill schema help" -f -a "like" -d 'Like a tweet (via xurl)'
complete -c bird -n "__fish_bird_using_subcommand help; and not __fish_seen_subcommand_from login me get post put bookmarks profile search thread delete watchlist usage tweet reply like unlike repost unrepost follow unfollow dm block unblock mute unmute doctor cache completions skill schema help" -f -a "unlike" -d 'Unlike a tweet (via xurl)'
complete -c bird -n "__fish_bird_using_subcommand help; and not __fish_seen_subcommand_from login me get post put bookmarks profile search thread delete watchlist usage tweet reply like unlike repost unrepost follow unfollow dm block unblock mute unmute doctor cache completions skill schema help" -f -a "repost" -d 'Repost (retweet) a tweet (via xurl)'
complete -c bird -n "__fish_bird_using_subcommand help; and not __fish_seen_subcommand_from login me get post put bookmarks profile search thread delete watchlist usage tweet reply like unlike repost unrepost follow unfollow dm block unblock mute unmute doctor cache completions skill schema help" -f -a "unrepost" -d 'Undo a repost (via xurl)'
complete -c bird -n "__fish_bird_using_subcommand help; and not __fish_seen_subcommand_from login me get post put bookmarks profile search thread delete watchlist usage tweet reply like unlike repost unrepost follow unfollow dm block unblock mute unmute doctor cache completions skill schema help" -f -a "follow" -d 'Follow a user (via xurl)'
complete -c bird -n "__fish_bird_using_subcommand help; and not __fish_seen_subcommand_from login me get post put bookmarks profile search thread delete watchlist usage tweet reply like unlike repost unrepost follow unfollow dm block unblock mute unmute doctor cache completions skill schema help" -f -a "unfollow" -d 'Unfollow a user (via xurl)'
complete -c bird -n "__fish_bird_using_subcommand help; and not __fish_seen_subcommand_from login me get post put bookmarks profile search thread delete watchlist usage tweet reply like unlike repost unrepost follow unfollow dm block unblock mute unmute doctor cache completions skill schema help" -f -a "dm" -d 'Send a direct message (via xurl)'
complete -c bird -n "__fish_bird_using_subcommand help; and not __fish_seen_subcommand_from login me get post put bookmarks profile search thread delete watchlist usage tweet reply like unlike repost unrepost follow unfollow dm block unblock mute unmute doctor cache completions skill schema help" -f -a "block" -d 'Block a user (via xurl)'
complete -c bird -n "__fish_bird_using_subcommand help; and not __fish_seen_subcommand_from login me get post put bookmarks profile search thread delete watchlist usage tweet reply like unlike repost unrepost follow unfollow dm block unblock mute unmute doctor cache completions skill schema help" -f -a "unblock" -d 'Unblock a user (via xurl)'
complete -c bird -n "__fish_bird_using_subcommand help; and not __fish_seen_subcommand_from login me get post put bookmarks profile search thread delete watchlist usage tweet reply like unlike repost unrepost follow unfollow dm block unblock mute unmute doctor cache completions skill schema help" -f -a "mute" -d 'Mute a user (via xurl)'
complete -c bird -n "__fish_bird_using_subcommand help; and not __fish_seen_subcommand_from login me get post put bookmarks profile search thread delete watchlist usage tweet reply like unlike repost unrepost follow unfollow dm block unblock mute unmute doctor cache completions skill schema help" -f -a "unmute" -d 'Unmute a user (via xurl)'
complete -c bird -n "__fish_bird_using_subcommand help; and not __fish_seen_subcommand_from login me get post put bookmarks profile search thread delete watchlist usage tweet reply like unlike repost unrepost follow unfollow dm block unblock mute unmute doctor cache completions skill schema help" -f -a "doctor" -d 'Show what is available: xurl status, commands, and entity store health'
complete -c bird -n "__fish_bird_using_subcommand help; and not __fish_seen_subcommand_from login me get post put bookmarks profile search thread delete watchlist usage tweet reply like unlike repost unrepost follow unfollow dm block unblock mute unmute doctor cache completions skill schema help" -f -a "cache" -d 'Manage the HTTP response cache'
complete -c bird -n "__fish_bird_using_subcommand help; and not __fish_seen_subcommand_from login me get post put bookmarks profile search thread delete watchlist usage tweet reply like unlike repost unrepost follow unfollow dm block unblock mute unmute doctor cache completions skill schema help" -f -a "completions" -d 'Generate shell completions'
complete -c bird -n "__fish_bird_using_subcommand help; and not __fish_seen_subcommand_from login me get post put bookmarks profile search thread delete watchlist usage tweet reply like unlike repost unrepost follow unfollow dm block unblock mute unmute doctor cache completions skill schema help" -f -a "skill" -d 'Manage the bird agent-skill bundle (install for Claude Code, etc.)'
complete -c bird -n "__fish_bird_using_subcommand help; and not __fish_seen_subcommand_from login me get post put bookmarks profile search thread delete watchlist usage tweet reply like unlike repost unrepost follow unfollow dm block unblock mute unmute doctor cache completions skill schema help" -f -a "schema" -d 'Print a JSON Schema document for one of bird\'s output shapes'
complete -c bird -n "__fish_bird_using_subcommand help; and not __fish_seen_subcommand_from login me get post put bookmarks profile search thread delete watchlist usage tweet reply like unlike repost unrepost follow unfollow dm block unblock mute unmute doctor cache completions skill schema help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c bird -n "__fish_bird_using_subcommand help; and __fish_seen_subcommand_from watchlist" -f -a "fetch" -d 'Fetch recent activity for all watched users'
complete -c bird -n "__fish_bird_using_subcommand help; and __fish_seen_subcommand_from watchlist" -f -a "add" -d 'Add a user to the watchlist'
complete -c bird -n "__fish_bird_using_subcommand help; and __fish_seen_subcommand_from watchlist" -f -a "remove" -d 'Remove a user from the watchlist'
complete -c bird -n "__fish_bird_using_subcommand help; and __fish_seen_subcommand_from watchlist" -f -a "list" -d 'Show the current watchlist'
complete -c bird -n "__fish_bird_using_subcommand help; and __fish_seen_subcommand_from cache" -f -a "clear" -d 'Delete all cache entries'
complete -c bird -n "__fish_bird_using_subcommand help; and __fish_seen_subcommand_from cache" -f -a "stats" -d 'Show cache status (JSON default, --pretty for human-readable)'
complete -c bird -n "__fish_bird_using_subcommand help; and __fish_seen_subcommand_from skill" -f -a "install" -d 'Install the bird skill bundle into a host\'s canonical skills directory'
complete -c bird -n "__fish_bird_using_subcommand help; and __fish_seen_subcommand_from skill" -f -a "update" -d 'Update the installed bird skill bundle to the embedded version'
