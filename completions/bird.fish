# Print an optspec for argparse to handle cmd's options that are independent of any subcommand.
function __fish_bird_global_optspecs
	string join \n u/username= plain no-color refresh no-cache cache-only q/quiet output= h/help V/version
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
complete -c bird -n "__fish_bird_needs_command" -l output -d 'Error output format: text (default for TTY), json (default for non-TTY)' -r -f -a "text\t'Default: colored, human-readable'
json\t'Machine-readable JSON, no color'"
complete -c bird -n "__fish_bird_needs_command" -l plain -d 'Plain output (no color, no hyperlinks; script-friendly)'
complete -c bird -n "__fish_bird_needs_command" -l no-color -d 'Disable ANSI colors (or set NO_COLOR)'
complete -c bird -n "__fish_bird_needs_command" -l refresh -d 'Bypass store read, still write response to store'
complete -c bird -n "__fish_bird_needs_command" -l no-cache -d 'Disable entity store entirely (no read, no write)'
complete -c bird -n "__fish_bird_needs_command" -l cache-only -d 'Only serve from local store; never make API requests'
complete -c bird -n "__fish_bird_needs_command" -s q -l quiet -d 'Suppress informational stderr output (keep only fatal errors)'
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
complete -c bird -n "__fish_bird_needs_command" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c bird -n "__fish_bird_using_subcommand login" -s u -l username -d 'Username for multi-user token selection (maps to xurl -u)' -r
complete -c bird -n "__fish_bird_using_subcommand login" -l output -d 'Error output format: text (default for TTY), json (default for non-TTY)' -r -f -a "text\t'Default: colored, human-readable'
json\t'Machine-readable JSON, no color'"
complete -c bird -n "__fish_bird_using_subcommand login" -l plain -d 'Plain output (no color, no hyperlinks; script-friendly)'
complete -c bird -n "__fish_bird_using_subcommand login" -l no-color -d 'Disable ANSI colors (or set NO_COLOR)'
complete -c bird -n "__fish_bird_using_subcommand login" -l refresh -d 'Bypass store read, still write response to store'
complete -c bird -n "__fish_bird_using_subcommand login" -l no-cache -d 'Disable entity store entirely (no read, no write)'
complete -c bird -n "__fish_bird_using_subcommand login" -l cache-only -d 'Only serve from local store; never make API requests'
complete -c bird -n "__fish_bird_using_subcommand login" -s q -l quiet -d 'Suppress informational stderr output (keep only fatal errors)'
complete -c bird -n "__fish_bird_using_subcommand login" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c bird -n "__fish_bird_using_subcommand me" -s u -l username -d 'Username for multi-user token selection (maps to xurl -u)' -r
complete -c bird -n "__fish_bird_using_subcommand me" -l output -d 'Error output format: text (default for TTY), json (default for non-TTY)' -r -f -a "text\t'Default: colored, human-readable'
json\t'Machine-readable JSON, no color'"
complete -c bird -n "__fish_bird_using_subcommand me" -l pretty -d 'Human-readable output'
complete -c bird -n "__fish_bird_using_subcommand me" -l plain -d 'Plain output (no color, no hyperlinks; script-friendly)'
complete -c bird -n "__fish_bird_using_subcommand me" -l no-color -d 'Disable ANSI colors (or set NO_COLOR)'
complete -c bird -n "__fish_bird_using_subcommand me" -l refresh -d 'Bypass store read, still write response to store'
complete -c bird -n "__fish_bird_using_subcommand me" -l no-cache -d 'Disable entity store entirely (no read, no write)'
complete -c bird -n "__fish_bird_using_subcommand me" -l cache-only -d 'Only serve from local store; never make API requests'
complete -c bird -n "__fish_bird_using_subcommand me" -s q -l quiet -d 'Suppress informational stderr output (keep only fatal errors)'
complete -c bird -n "__fish_bird_using_subcommand me" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c bird -n "__fish_bird_using_subcommand get" -s p -l param -r
complete -c bird -n "__fish_bird_using_subcommand get" -l query -r
complete -c bird -n "__fish_bird_using_subcommand get" -s u -l username -d 'Username for multi-user token selection (maps to xurl -u)' -r
complete -c bird -n "__fish_bird_using_subcommand get" -l output -d 'Error output format: text (default for TTY), json (default for non-TTY)' -r -f -a "text\t'Default: colored, human-readable'
json\t'Machine-readable JSON, no color'"
complete -c bird -n "__fish_bird_using_subcommand get" -l pretty
complete -c bird -n "__fish_bird_using_subcommand get" -l plain -d 'Plain output (no color, no hyperlinks; script-friendly)'
complete -c bird -n "__fish_bird_using_subcommand get" -l no-color -d 'Disable ANSI colors (or set NO_COLOR)'
complete -c bird -n "__fish_bird_using_subcommand get" -l refresh -d 'Bypass store read, still write response to store'
complete -c bird -n "__fish_bird_using_subcommand get" -l no-cache -d 'Disable entity store entirely (no read, no write)'
complete -c bird -n "__fish_bird_using_subcommand get" -l cache-only -d 'Only serve from local store; never make API requests'
complete -c bird -n "__fish_bird_using_subcommand get" -s q -l quiet -d 'Suppress informational stderr output (keep only fatal errors)'
complete -c bird -n "__fish_bird_using_subcommand get" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c bird -n "__fish_bird_using_subcommand post" -s p -l param -r
complete -c bird -n "__fish_bird_using_subcommand post" -l query -r
complete -c bird -n "__fish_bird_using_subcommand post" -l body -r
complete -c bird -n "__fish_bird_using_subcommand post" -s u -l username -d 'Username for multi-user token selection (maps to xurl -u)' -r
complete -c bird -n "__fish_bird_using_subcommand post" -l output -d 'Error output format: text (default for TTY), json (default for non-TTY)' -r -f -a "text\t'Default: colored, human-readable'
json\t'Machine-readable JSON, no color'"
complete -c bird -n "__fish_bird_using_subcommand post" -l pretty
complete -c bird -n "__fish_bird_using_subcommand post" -l plain -d 'Plain output (no color, no hyperlinks; script-friendly)'
complete -c bird -n "__fish_bird_using_subcommand post" -l no-color -d 'Disable ANSI colors (or set NO_COLOR)'
complete -c bird -n "__fish_bird_using_subcommand post" -l refresh -d 'Bypass store read, still write response to store'
complete -c bird -n "__fish_bird_using_subcommand post" -l no-cache -d 'Disable entity store entirely (no read, no write)'
complete -c bird -n "__fish_bird_using_subcommand post" -l cache-only -d 'Only serve from local store; never make API requests'
complete -c bird -n "__fish_bird_using_subcommand post" -s q -l quiet -d 'Suppress informational stderr output (keep only fatal errors)'
complete -c bird -n "__fish_bird_using_subcommand post" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c bird -n "__fish_bird_using_subcommand put" -s p -l param -r
complete -c bird -n "__fish_bird_using_subcommand put" -l query -r
complete -c bird -n "__fish_bird_using_subcommand put" -l body -r
complete -c bird -n "__fish_bird_using_subcommand put" -s u -l username -d 'Username for multi-user token selection (maps to xurl -u)' -r
complete -c bird -n "__fish_bird_using_subcommand put" -l output -d 'Error output format: text (default for TTY), json (default for non-TTY)' -r -f -a "text\t'Default: colored, human-readable'
json\t'Machine-readable JSON, no color'"
complete -c bird -n "__fish_bird_using_subcommand put" -l pretty
complete -c bird -n "__fish_bird_using_subcommand put" -l plain -d 'Plain output (no color, no hyperlinks; script-friendly)'
complete -c bird -n "__fish_bird_using_subcommand put" -l no-color -d 'Disable ANSI colors (or set NO_COLOR)'
complete -c bird -n "__fish_bird_using_subcommand put" -l refresh -d 'Bypass store read, still write response to store'
complete -c bird -n "__fish_bird_using_subcommand put" -l no-cache -d 'Disable entity store entirely (no read, no write)'
complete -c bird -n "__fish_bird_using_subcommand put" -l cache-only -d 'Only serve from local store; never make API requests'
complete -c bird -n "__fish_bird_using_subcommand put" -s q -l quiet -d 'Suppress informational stderr output (keep only fatal errors)'
complete -c bird -n "__fish_bird_using_subcommand put" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c bird -n "__fish_bird_using_subcommand bookmarks" -s u -l username -d 'Username for multi-user token selection (maps to xurl -u)' -r
complete -c bird -n "__fish_bird_using_subcommand bookmarks" -l output -d 'Error output format: text (default for TTY), json (default for non-TTY)' -r -f -a "text\t'Default: colored, human-readable'
json\t'Machine-readable JSON, no color'"
complete -c bird -n "__fish_bird_using_subcommand bookmarks" -l pretty
complete -c bird -n "__fish_bird_using_subcommand bookmarks" -l plain -d 'Plain output (no color, no hyperlinks; script-friendly)'
complete -c bird -n "__fish_bird_using_subcommand bookmarks" -l no-color -d 'Disable ANSI colors (or set NO_COLOR)'
complete -c bird -n "__fish_bird_using_subcommand bookmarks" -l refresh -d 'Bypass store read, still write response to store'
complete -c bird -n "__fish_bird_using_subcommand bookmarks" -l no-cache -d 'Disable entity store entirely (no read, no write)'
complete -c bird -n "__fish_bird_using_subcommand bookmarks" -l cache-only -d 'Only serve from local store; never make API requests'
complete -c bird -n "__fish_bird_using_subcommand bookmarks" -s q -l quiet -d 'Suppress informational stderr output (keep only fatal errors)'
complete -c bird -n "__fish_bird_using_subcommand bookmarks" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c bird -n "__fish_bird_using_subcommand profile" -l output -d 'Error output format: text (default for TTY), json (default for non-TTY)' -r -f -a "text\t'Default: colored, human-readable'
json\t'Machine-readable JSON, no color'"
complete -c bird -n "__fish_bird_using_subcommand profile" -l pretty -d 'Pretty-print JSON output'
complete -c bird -n "__fish_bird_using_subcommand profile" -l plain -d 'Plain output (no color, no hyperlinks; script-friendly)'
complete -c bird -n "__fish_bird_using_subcommand profile" -l no-color -d 'Disable ANSI colors (or set NO_COLOR)'
complete -c bird -n "__fish_bird_using_subcommand profile" -l refresh -d 'Bypass store read, still write response to store'
complete -c bird -n "__fish_bird_using_subcommand profile" -l no-cache -d 'Disable entity store entirely (no read, no write)'
complete -c bird -n "__fish_bird_using_subcommand profile" -l cache-only -d 'Only serve from local store; never make API requests'
complete -c bird -n "__fish_bird_using_subcommand profile" -s q -l quiet -d 'Suppress informational stderr output (keep only fatal errors)'
complete -c bird -n "__fish_bird_using_subcommand profile" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c bird -n "__fish_bird_using_subcommand search" -l sort -d 'Sort results: recent (default), likes' -r
complete -c bird -n "__fish_bird_using_subcommand search" -l min-likes -d 'Minimum like count threshold' -r
complete -c bird -n "__fish_bird_using_subcommand search" -l max-results -d 'Maximum results per page (10-100, default: 100)' -r
complete -c bird -n "__fish_bird_using_subcommand search" -l pages -d 'Number of pages to fetch (1-10, default: 1)' -r
complete -c bird -n "__fish_bird_using_subcommand search" -s u -l username -d 'Username for multi-user token selection (maps to xurl -u)' -r
complete -c bird -n "__fish_bird_using_subcommand search" -l output -d 'Error output format: text (default for TTY), json (default for non-TTY)' -r -f -a "text\t'Default: colored, human-readable'
json\t'Machine-readable JSON, no color'"
complete -c bird -n "__fish_bird_using_subcommand search" -l pretty -d 'Pretty-print JSON output'
complete -c bird -n "__fish_bird_using_subcommand search" -l plain -d 'Plain output (no color, no hyperlinks; script-friendly)'
complete -c bird -n "__fish_bird_using_subcommand search" -l no-color -d 'Disable ANSI colors (or set NO_COLOR)'
complete -c bird -n "__fish_bird_using_subcommand search" -l refresh -d 'Bypass store read, still write response to store'
complete -c bird -n "__fish_bird_using_subcommand search" -l no-cache -d 'Disable entity store entirely (no read, no write)'
complete -c bird -n "__fish_bird_using_subcommand search" -l cache-only -d 'Only serve from local store; never make API requests'
complete -c bird -n "__fish_bird_using_subcommand search" -s q -l quiet -d 'Suppress informational stderr output (keep only fatal errors)'
complete -c bird -n "__fish_bird_using_subcommand search" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c bird -n "__fish_bird_using_subcommand thread" -l max-pages -d 'Maximum number of search result pages (default: 10, max: 25)' -r
complete -c bird -n "__fish_bird_using_subcommand thread" -s u -l username -d 'Username for multi-user token selection (maps to xurl -u)' -r
complete -c bird -n "__fish_bird_using_subcommand thread" -l output -d 'Error output format: text (default for TTY), json (default for non-TTY)' -r -f -a "text\t'Default: colored, human-readable'
json\t'Machine-readable JSON, no color'"
complete -c bird -n "__fish_bird_using_subcommand thread" -l pretty -d 'Pretty-print JSON output'
complete -c bird -n "__fish_bird_using_subcommand thread" -l plain -d 'Plain output (no color, no hyperlinks; script-friendly)'
complete -c bird -n "__fish_bird_using_subcommand thread" -l no-color -d 'Disable ANSI colors (or set NO_COLOR)'
complete -c bird -n "__fish_bird_using_subcommand thread" -l refresh -d 'Bypass store read, still write response to store'
complete -c bird -n "__fish_bird_using_subcommand thread" -l no-cache -d 'Disable entity store entirely (no read, no write)'
complete -c bird -n "__fish_bird_using_subcommand thread" -l cache-only -d 'Only serve from local store; never make API requests'
complete -c bird -n "__fish_bird_using_subcommand thread" -s q -l quiet -d 'Suppress informational stderr output (keep only fatal errors)'
complete -c bird -n "__fish_bird_using_subcommand thread" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c bird -n "__fish_bird_using_subcommand delete" -s p -l param -r
complete -c bird -n "__fish_bird_using_subcommand delete" -l query -r
complete -c bird -n "__fish_bird_using_subcommand delete" -s u -l username -d 'Username for multi-user token selection (maps to xurl -u)' -r
complete -c bird -n "__fish_bird_using_subcommand delete" -l output -d 'Error output format: text (default for TTY), json (default for non-TTY)' -r -f -a "text\t'Default: colored, human-readable'
json\t'Machine-readable JSON, no color'"
complete -c bird -n "__fish_bird_using_subcommand delete" -l pretty
complete -c bird -n "__fish_bird_using_subcommand delete" -l plain -d 'Plain output (no color, no hyperlinks; script-friendly)'
complete -c bird -n "__fish_bird_using_subcommand delete" -l no-color -d 'Disable ANSI colors (or set NO_COLOR)'
complete -c bird -n "__fish_bird_using_subcommand delete" -l refresh -d 'Bypass store read, still write response to store'
complete -c bird -n "__fish_bird_using_subcommand delete" -l no-cache -d 'Disable entity store entirely (no read, no write)'
complete -c bird -n "__fish_bird_using_subcommand delete" -l cache-only -d 'Only serve from local store; never make API requests'
complete -c bird -n "__fish_bird_using_subcommand delete" -s q -l quiet -d 'Suppress informational stderr output (keep only fatal errors)'
complete -c bird -n "__fish_bird_using_subcommand delete" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and not __fish_seen_subcommand_from check add remove list help" -s u -l username -d 'Username for multi-user token selection (maps to xurl -u)' -r
complete -c bird -n "__fish_bird_using_subcommand watchlist; and not __fish_seen_subcommand_from check add remove list help" -l output -d 'Error output format: text (default for TTY), json (default for non-TTY)' -r -f -a "text\t'Default: colored, human-readable'
json\t'Machine-readable JSON, no color'"
complete -c bird -n "__fish_bird_using_subcommand watchlist; and not __fish_seen_subcommand_from check add remove list help" -l pretty -d 'Pretty-print JSON output'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and not __fish_seen_subcommand_from check add remove list help" -l plain -d 'Plain output (no color, no hyperlinks; script-friendly)'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and not __fish_seen_subcommand_from check add remove list help" -l no-color -d 'Disable ANSI colors (or set NO_COLOR)'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and not __fish_seen_subcommand_from check add remove list help" -l refresh -d 'Bypass store read, still write response to store'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and not __fish_seen_subcommand_from check add remove list help" -l no-cache -d 'Disable entity store entirely (no read, no write)'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and not __fish_seen_subcommand_from check add remove list help" -l cache-only -d 'Only serve from local store; never make API requests'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and not __fish_seen_subcommand_from check add remove list help" -s q -l quiet -d 'Suppress informational stderr output (keep only fatal errors)'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and not __fish_seen_subcommand_from check add remove list help" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and not __fish_seen_subcommand_from check add remove list help" -f -a "check" -d 'Check recent activity for all watched users'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and not __fish_seen_subcommand_from check add remove list help" -f -a "add" -d 'Add a user to the watchlist'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and not __fish_seen_subcommand_from check add remove list help" -f -a "remove" -d 'Remove a user from the watchlist'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and not __fish_seen_subcommand_from check add remove list help" -f -a "list" -d 'Show the current watchlist'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and not __fish_seen_subcommand_from check add remove list help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from check" -s u -l username -d 'Username for multi-user token selection (maps to xurl -u)' -r
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from check" -l output -d 'Error output format: text (default for TTY), json (default for non-TTY)' -r -f -a "text\t'Default: colored, human-readable'
json\t'Machine-readable JSON, no color'"
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from check" -l plain -d 'Plain output (no color, no hyperlinks; script-friendly)'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from check" -l no-color -d 'Disable ANSI colors (or set NO_COLOR)'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from check" -l refresh -d 'Bypass store read, still write response to store'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from check" -l no-cache -d 'Disable entity store entirely (no read, no write)'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from check" -l cache-only -d 'Only serve from local store; never make API requests'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from check" -s q -l quiet -d 'Suppress informational stderr output (keep only fatal errors)'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from check" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from add" -l output -d 'Error output format: text (default for TTY), json (default for non-TTY)' -r -f -a "text\t'Default: colored, human-readable'
json\t'Machine-readable JSON, no color'"
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from add" -l plain -d 'Plain output (no color, no hyperlinks; script-friendly)'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from add" -l no-color -d 'Disable ANSI colors (or set NO_COLOR)'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from add" -l refresh -d 'Bypass store read, still write response to store'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from add" -l no-cache -d 'Disable entity store entirely (no read, no write)'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from add" -l cache-only -d 'Only serve from local store; never make API requests'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from add" -s q -l quiet -d 'Suppress informational stderr output (keep only fatal errors)'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from add" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from remove" -l output -d 'Error output format: text (default for TTY), json (default for non-TTY)' -r -f -a "text\t'Default: colored, human-readable'
json\t'Machine-readable JSON, no color'"
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from remove" -l plain -d 'Plain output (no color, no hyperlinks; script-friendly)'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from remove" -l no-color -d 'Disable ANSI colors (or set NO_COLOR)'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from remove" -l refresh -d 'Bypass store read, still write response to store'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from remove" -l no-cache -d 'Disable entity store entirely (no read, no write)'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from remove" -l cache-only -d 'Only serve from local store; never make API requests'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from remove" -s q -l quiet -d 'Suppress informational stderr output (keep only fatal errors)'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from remove" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from list" -s u -l username -d 'Username for multi-user token selection (maps to xurl -u)' -r
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from list" -l output -d 'Error output format: text (default for TTY), json (default for non-TTY)' -r -f -a "text\t'Default: colored, human-readable'
json\t'Machine-readable JSON, no color'"
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from list" -l plain -d 'Plain output (no color, no hyperlinks; script-friendly)'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from list" -l no-color -d 'Disable ANSI colors (or set NO_COLOR)'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from list" -l refresh -d 'Bypass store read, still write response to store'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from list" -l no-cache -d 'Disable entity store entirely (no read, no write)'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from list" -l cache-only -d 'Only serve from local store; never make API requests'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from list" -s q -l quiet -d 'Suppress informational stderr output (keep only fatal errors)'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from list" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from help" -f -a "check" -d 'Check recent activity for all watched users'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from help" -f -a "add" -d 'Add a user to the watchlist'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from help" -f -a "remove" -d 'Remove a user from the watchlist'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from help" -f -a "list" -d 'Show the current watchlist'
complete -c bird -n "__fish_bird_using_subcommand watchlist; and __fish_seen_subcommand_from help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c bird -n "__fish_bird_using_subcommand usage" -l since -d 'Show usage since this date (YYYY-MM-DD; default: 30 days ago)' -r
complete -c bird -n "__fish_bird_using_subcommand usage" -s u -l username -d 'Username for multi-user token selection (maps to xurl -u)' -r
complete -c bird -n "__fish_bird_using_subcommand usage" -l output -d 'Error output format: text (default for TTY), json (default for non-TTY)' -r -f -a "text\t'Default: colored, human-readable'
json\t'Machine-readable JSON, no color'"
complete -c bird -n "__fish_bird_using_subcommand usage" -l sync -d 'Sync actual usage from X API (requires Bearer token via xurl)'
complete -c bird -n "__fish_bird_using_subcommand usage" -l pretty -d 'Pretty-print output'
complete -c bird -n "__fish_bird_using_subcommand usage" -l plain -d 'Plain output (no color, no hyperlinks; script-friendly)'
complete -c bird -n "__fish_bird_using_subcommand usage" -l no-color -d 'Disable ANSI colors (or set NO_COLOR)'
complete -c bird -n "__fish_bird_using_subcommand usage" -l refresh -d 'Bypass store read, still write response to store'
complete -c bird -n "__fish_bird_using_subcommand usage" -l no-cache -d 'Disable entity store entirely (no read, no write)'
complete -c bird -n "__fish_bird_using_subcommand usage" -l cache-only -d 'Only serve from local store; never make API requests'
complete -c bird -n "__fish_bird_using_subcommand usage" -s q -l quiet -d 'Suppress informational stderr output (keep only fatal errors)'
complete -c bird -n "__fish_bird_using_subcommand usage" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c bird -n "__fish_bird_using_subcommand tweet" -l media-id -d 'Media ID to attach' -r
complete -c bird -n "__fish_bird_using_subcommand tweet" -s u -l username -d 'Username for multi-user token selection (maps to xurl -u)' -r
complete -c bird -n "__fish_bird_using_subcommand tweet" -l output -d 'Error output format: text (default for TTY), json (default for non-TTY)' -r -f -a "text\t'Default: colored, human-readable'
json\t'Machine-readable JSON, no color'"
complete -c bird -n "__fish_bird_using_subcommand tweet" -l plain -d 'Plain output (no color, no hyperlinks; script-friendly)'
complete -c bird -n "__fish_bird_using_subcommand tweet" -l no-color -d 'Disable ANSI colors (or set NO_COLOR)'
complete -c bird -n "__fish_bird_using_subcommand tweet" -l refresh -d 'Bypass store read, still write response to store'
complete -c bird -n "__fish_bird_using_subcommand tweet" -l no-cache -d 'Disable entity store entirely (no read, no write)'
complete -c bird -n "__fish_bird_using_subcommand tweet" -l cache-only -d 'Only serve from local store; never make API requests'
complete -c bird -n "__fish_bird_using_subcommand tweet" -s q -l quiet -d 'Suppress informational stderr output (keep only fatal errors)'
complete -c bird -n "__fish_bird_using_subcommand tweet" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c bird -n "__fish_bird_using_subcommand reply" -s u -l username -d 'Username for multi-user token selection (maps to xurl -u)' -r
complete -c bird -n "__fish_bird_using_subcommand reply" -l output -d 'Error output format: text (default for TTY), json (default for non-TTY)' -r -f -a "text\t'Default: colored, human-readable'
json\t'Machine-readable JSON, no color'"
complete -c bird -n "__fish_bird_using_subcommand reply" -l plain -d 'Plain output (no color, no hyperlinks; script-friendly)'
complete -c bird -n "__fish_bird_using_subcommand reply" -l no-color -d 'Disable ANSI colors (or set NO_COLOR)'
complete -c bird -n "__fish_bird_using_subcommand reply" -l refresh -d 'Bypass store read, still write response to store'
complete -c bird -n "__fish_bird_using_subcommand reply" -l no-cache -d 'Disable entity store entirely (no read, no write)'
complete -c bird -n "__fish_bird_using_subcommand reply" -l cache-only -d 'Only serve from local store; never make API requests'
complete -c bird -n "__fish_bird_using_subcommand reply" -s q -l quiet -d 'Suppress informational stderr output (keep only fatal errors)'
complete -c bird -n "__fish_bird_using_subcommand reply" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c bird -n "__fish_bird_using_subcommand like" -s u -l username -d 'Username for multi-user token selection (maps to xurl -u)' -r
complete -c bird -n "__fish_bird_using_subcommand like" -l output -d 'Error output format: text (default for TTY), json (default for non-TTY)' -r -f -a "text\t'Default: colored, human-readable'
json\t'Machine-readable JSON, no color'"
complete -c bird -n "__fish_bird_using_subcommand like" -l plain -d 'Plain output (no color, no hyperlinks; script-friendly)'
complete -c bird -n "__fish_bird_using_subcommand like" -l no-color -d 'Disable ANSI colors (or set NO_COLOR)'
complete -c bird -n "__fish_bird_using_subcommand like" -l refresh -d 'Bypass store read, still write response to store'
complete -c bird -n "__fish_bird_using_subcommand like" -l no-cache -d 'Disable entity store entirely (no read, no write)'
complete -c bird -n "__fish_bird_using_subcommand like" -l cache-only -d 'Only serve from local store; never make API requests'
complete -c bird -n "__fish_bird_using_subcommand like" -s q -l quiet -d 'Suppress informational stderr output (keep only fatal errors)'
complete -c bird -n "__fish_bird_using_subcommand like" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c bird -n "__fish_bird_using_subcommand unlike" -s u -l username -d 'Username for multi-user token selection (maps to xurl -u)' -r
complete -c bird -n "__fish_bird_using_subcommand unlike" -l output -d 'Error output format: text (default for TTY), json (default for non-TTY)' -r -f -a "text\t'Default: colored, human-readable'
json\t'Machine-readable JSON, no color'"
complete -c bird -n "__fish_bird_using_subcommand unlike" -l plain -d 'Plain output (no color, no hyperlinks; script-friendly)'
complete -c bird -n "__fish_bird_using_subcommand unlike" -l no-color -d 'Disable ANSI colors (or set NO_COLOR)'
complete -c bird -n "__fish_bird_using_subcommand unlike" -l refresh -d 'Bypass store read, still write response to store'
complete -c bird -n "__fish_bird_using_subcommand unlike" -l no-cache -d 'Disable entity store entirely (no read, no write)'
complete -c bird -n "__fish_bird_using_subcommand unlike" -l cache-only -d 'Only serve from local store; never make API requests'
complete -c bird -n "__fish_bird_using_subcommand unlike" -s q -l quiet -d 'Suppress informational stderr output (keep only fatal errors)'
complete -c bird -n "__fish_bird_using_subcommand unlike" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c bird -n "__fish_bird_using_subcommand repost" -s u -l username -d 'Username for multi-user token selection (maps to xurl -u)' -r
complete -c bird -n "__fish_bird_using_subcommand repost" -l output -d 'Error output format: text (default for TTY), json (default for non-TTY)' -r -f -a "text\t'Default: colored, human-readable'
json\t'Machine-readable JSON, no color'"
complete -c bird -n "__fish_bird_using_subcommand repost" -l plain -d 'Plain output (no color, no hyperlinks; script-friendly)'
complete -c bird -n "__fish_bird_using_subcommand repost" -l no-color -d 'Disable ANSI colors (or set NO_COLOR)'
complete -c bird -n "__fish_bird_using_subcommand repost" -l refresh -d 'Bypass store read, still write response to store'
complete -c bird -n "__fish_bird_using_subcommand repost" -l no-cache -d 'Disable entity store entirely (no read, no write)'
complete -c bird -n "__fish_bird_using_subcommand repost" -l cache-only -d 'Only serve from local store; never make API requests'
complete -c bird -n "__fish_bird_using_subcommand repost" -s q -l quiet -d 'Suppress informational stderr output (keep only fatal errors)'
complete -c bird -n "__fish_bird_using_subcommand repost" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c bird -n "__fish_bird_using_subcommand unrepost" -s u -l username -d 'Username for multi-user token selection (maps to xurl -u)' -r
complete -c bird -n "__fish_bird_using_subcommand unrepost" -l output -d 'Error output format: text (default for TTY), json (default for non-TTY)' -r -f -a "text\t'Default: colored, human-readable'
json\t'Machine-readable JSON, no color'"
complete -c bird -n "__fish_bird_using_subcommand unrepost" -l plain -d 'Plain output (no color, no hyperlinks; script-friendly)'
complete -c bird -n "__fish_bird_using_subcommand unrepost" -l no-color -d 'Disable ANSI colors (or set NO_COLOR)'
complete -c bird -n "__fish_bird_using_subcommand unrepost" -l refresh -d 'Bypass store read, still write response to store'
complete -c bird -n "__fish_bird_using_subcommand unrepost" -l no-cache -d 'Disable entity store entirely (no read, no write)'
complete -c bird -n "__fish_bird_using_subcommand unrepost" -l cache-only -d 'Only serve from local store; never make API requests'
complete -c bird -n "__fish_bird_using_subcommand unrepost" -s q -l quiet -d 'Suppress informational stderr output (keep only fatal errors)'
complete -c bird -n "__fish_bird_using_subcommand unrepost" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c bird -n "__fish_bird_using_subcommand follow" -l output -d 'Error output format: text (default for TTY), json (default for non-TTY)' -r -f -a "text\t'Default: colored, human-readable'
json\t'Machine-readable JSON, no color'"
complete -c bird -n "__fish_bird_using_subcommand follow" -l plain -d 'Plain output (no color, no hyperlinks; script-friendly)'
complete -c bird -n "__fish_bird_using_subcommand follow" -l no-color -d 'Disable ANSI colors (or set NO_COLOR)'
complete -c bird -n "__fish_bird_using_subcommand follow" -l refresh -d 'Bypass store read, still write response to store'
complete -c bird -n "__fish_bird_using_subcommand follow" -l no-cache -d 'Disable entity store entirely (no read, no write)'
complete -c bird -n "__fish_bird_using_subcommand follow" -l cache-only -d 'Only serve from local store; never make API requests'
complete -c bird -n "__fish_bird_using_subcommand follow" -s q -l quiet -d 'Suppress informational stderr output (keep only fatal errors)'
complete -c bird -n "__fish_bird_using_subcommand follow" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c bird -n "__fish_bird_using_subcommand unfollow" -l output -d 'Error output format: text (default for TTY), json (default for non-TTY)' -r -f -a "text\t'Default: colored, human-readable'
json\t'Machine-readable JSON, no color'"
complete -c bird -n "__fish_bird_using_subcommand unfollow" -l plain -d 'Plain output (no color, no hyperlinks; script-friendly)'
complete -c bird -n "__fish_bird_using_subcommand unfollow" -l no-color -d 'Disable ANSI colors (or set NO_COLOR)'
complete -c bird -n "__fish_bird_using_subcommand unfollow" -l refresh -d 'Bypass store read, still write response to store'
complete -c bird -n "__fish_bird_using_subcommand unfollow" -l no-cache -d 'Disable entity store entirely (no read, no write)'
complete -c bird -n "__fish_bird_using_subcommand unfollow" -l cache-only -d 'Only serve from local store; never make API requests'
complete -c bird -n "__fish_bird_using_subcommand unfollow" -s q -l quiet -d 'Suppress informational stderr output (keep only fatal errors)'
complete -c bird -n "__fish_bird_using_subcommand unfollow" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c bird -n "__fish_bird_using_subcommand dm" -l output -d 'Error output format: text (default for TTY), json (default for non-TTY)' -r -f -a "text\t'Default: colored, human-readable'
json\t'Machine-readable JSON, no color'"
complete -c bird -n "__fish_bird_using_subcommand dm" -l plain -d 'Plain output (no color, no hyperlinks; script-friendly)'
complete -c bird -n "__fish_bird_using_subcommand dm" -l no-color -d 'Disable ANSI colors (or set NO_COLOR)'
complete -c bird -n "__fish_bird_using_subcommand dm" -l refresh -d 'Bypass store read, still write response to store'
complete -c bird -n "__fish_bird_using_subcommand dm" -l no-cache -d 'Disable entity store entirely (no read, no write)'
complete -c bird -n "__fish_bird_using_subcommand dm" -l cache-only -d 'Only serve from local store; never make API requests'
complete -c bird -n "__fish_bird_using_subcommand dm" -s q -l quiet -d 'Suppress informational stderr output (keep only fatal errors)'
complete -c bird -n "__fish_bird_using_subcommand dm" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c bird -n "__fish_bird_using_subcommand block" -l output -d 'Error output format: text (default for TTY), json (default for non-TTY)' -r -f -a "text\t'Default: colored, human-readable'
json\t'Machine-readable JSON, no color'"
complete -c bird -n "__fish_bird_using_subcommand block" -l plain -d 'Plain output (no color, no hyperlinks; script-friendly)'
complete -c bird -n "__fish_bird_using_subcommand block" -l no-color -d 'Disable ANSI colors (or set NO_COLOR)'
complete -c bird -n "__fish_bird_using_subcommand block" -l refresh -d 'Bypass store read, still write response to store'
complete -c bird -n "__fish_bird_using_subcommand block" -l no-cache -d 'Disable entity store entirely (no read, no write)'
complete -c bird -n "__fish_bird_using_subcommand block" -l cache-only -d 'Only serve from local store; never make API requests'
complete -c bird -n "__fish_bird_using_subcommand block" -s q -l quiet -d 'Suppress informational stderr output (keep only fatal errors)'
complete -c bird -n "__fish_bird_using_subcommand block" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c bird -n "__fish_bird_using_subcommand unblock" -l output -d 'Error output format: text (default for TTY), json (default for non-TTY)' -r -f -a "text\t'Default: colored, human-readable'
json\t'Machine-readable JSON, no color'"
complete -c bird -n "__fish_bird_using_subcommand unblock" -l plain -d 'Plain output (no color, no hyperlinks; script-friendly)'
complete -c bird -n "__fish_bird_using_subcommand unblock" -l no-color -d 'Disable ANSI colors (or set NO_COLOR)'
complete -c bird -n "__fish_bird_using_subcommand unblock" -l refresh -d 'Bypass store read, still write response to store'
complete -c bird -n "__fish_bird_using_subcommand unblock" -l no-cache -d 'Disable entity store entirely (no read, no write)'
complete -c bird -n "__fish_bird_using_subcommand unblock" -l cache-only -d 'Only serve from local store; never make API requests'
complete -c bird -n "__fish_bird_using_subcommand unblock" -s q -l quiet -d 'Suppress informational stderr output (keep only fatal errors)'
complete -c bird -n "__fish_bird_using_subcommand unblock" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c bird -n "__fish_bird_using_subcommand mute" -l output -d 'Error output format: text (default for TTY), json (default for non-TTY)' -r -f -a "text\t'Default: colored, human-readable'
json\t'Machine-readable JSON, no color'"
complete -c bird -n "__fish_bird_using_subcommand mute" -l plain -d 'Plain output (no color, no hyperlinks; script-friendly)'
complete -c bird -n "__fish_bird_using_subcommand mute" -l no-color -d 'Disable ANSI colors (or set NO_COLOR)'
complete -c bird -n "__fish_bird_using_subcommand mute" -l refresh -d 'Bypass store read, still write response to store'
complete -c bird -n "__fish_bird_using_subcommand mute" -l no-cache -d 'Disable entity store entirely (no read, no write)'
complete -c bird -n "__fish_bird_using_subcommand mute" -l cache-only -d 'Only serve from local store; never make API requests'
complete -c bird -n "__fish_bird_using_subcommand mute" -s q -l quiet -d 'Suppress informational stderr output (keep only fatal errors)'
complete -c bird -n "__fish_bird_using_subcommand mute" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c bird -n "__fish_bird_using_subcommand unmute" -l output -d 'Error output format: text (default for TTY), json (default for non-TTY)' -r -f -a "text\t'Default: colored, human-readable'
json\t'Machine-readable JSON, no color'"
complete -c bird -n "__fish_bird_using_subcommand unmute" -l plain -d 'Plain output (no color, no hyperlinks; script-friendly)'
complete -c bird -n "__fish_bird_using_subcommand unmute" -l no-color -d 'Disable ANSI colors (or set NO_COLOR)'
complete -c bird -n "__fish_bird_using_subcommand unmute" -l refresh -d 'Bypass store read, still write response to store'
complete -c bird -n "__fish_bird_using_subcommand unmute" -l no-cache -d 'Disable entity store entirely (no read, no write)'
complete -c bird -n "__fish_bird_using_subcommand unmute" -l cache-only -d 'Only serve from local store; never make API requests'
complete -c bird -n "__fish_bird_using_subcommand unmute" -s q -l quiet -d 'Suppress informational stderr output (keep only fatal errors)'
complete -c bird -n "__fish_bird_using_subcommand unmute" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c bird -n "__fish_bird_using_subcommand doctor" -s u -l username -d 'Username for multi-user token selection (maps to xurl -u)' -r
complete -c bird -n "__fish_bird_using_subcommand doctor" -l output -d 'Error output format: text (default for TTY), json (default for non-TTY)' -r -f -a "text\t'Default: colored, human-readable'
json\t'Machine-readable JSON, no color'"
complete -c bird -n "__fish_bird_using_subcommand doctor" -l pretty
complete -c bird -n "__fish_bird_using_subcommand doctor" -l plain -d 'Plain output (no color, no hyperlinks; script-friendly)'
complete -c bird -n "__fish_bird_using_subcommand doctor" -l no-color -d 'Disable ANSI colors (or set NO_COLOR)'
complete -c bird -n "__fish_bird_using_subcommand doctor" -l refresh -d 'Bypass store read, still write response to store'
complete -c bird -n "__fish_bird_using_subcommand doctor" -l no-cache -d 'Disable entity store entirely (no read, no write)'
complete -c bird -n "__fish_bird_using_subcommand doctor" -l cache-only -d 'Only serve from local store; never make API requests'
complete -c bird -n "__fish_bird_using_subcommand doctor" -s q -l quiet -d 'Suppress informational stderr output (keep only fatal errors)'
complete -c bird -n "__fish_bird_using_subcommand doctor" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c bird -n "__fish_bird_using_subcommand cache; and not __fish_seen_subcommand_from clear stats help" -s u -l username -d 'Username for multi-user token selection (maps to xurl -u)' -r
complete -c bird -n "__fish_bird_using_subcommand cache; and not __fish_seen_subcommand_from clear stats help" -l output -d 'Error output format: text (default for TTY), json (default for non-TTY)' -r -f -a "text\t'Default: colored, human-readable'
json\t'Machine-readable JSON, no color'"
complete -c bird -n "__fish_bird_using_subcommand cache; and not __fish_seen_subcommand_from clear stats help" -l plain -d 'Plain output (no color, no hyperlinks; script-friendly)'
complete -c bird -n "__fish_bird_using_subcommand cache; and not __fish_seen_subcommand_from clear stats help" -l no-color -d 'Disable ANSI colors (or set NO_COLOR)'
complete -c bird -n "__fish_bird_using_subcommand cache; and not __fish_seen_subcommand_from clear stats help" -l refresh -d 'Bypass store read, still write response to store'
complete -c bird -n "__fish_bird_using_subcommand cache; and not __fish_seen_subcommand_from clear stats help" -l no-cache -d 'Disable entity store entirely (no read, no write)'
complete -c bird -n "__fish_bird_using_subcommand cache; and not __fish_seen_subcommand_from clear stats help" -l cache-only -d 'Only serve from local store; never make API requests'
complete -c bird -n "__fish_bird_using_subcommand cache; and not __fish_seen_subcommand_from clear stats help" -s q -l quiet -d 'Suppress informational stderr output (keep only fatal errors)'
complete -c bird -n "__fish_bird_using_subcommand cache; and not __fish_seen_subcommand_from clear stats help" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c bird -n "__fish_bird_using_subcommand cache; and not __fish_seen_subcommand_from clear stats help" -f -a "clear" -d 'Delete all cache entries'
complete -c bird -n "__fish_bird_using_subcommand cache; and not __fish_seen_subcommand_from clear stats help" -f -a "stats" -d 'Show cache status (JSON default, --pretty for human-readable)'
complete -c bird -n "__fish_bird_using_subcommand cache; and not __fish_seen_subcommand_from clear stats help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c bird -n "__fish_bird_using_subcommand cache; and __fish_seen_subcommand_from clear" -s u -l username -d 'Username for multi-user token selection (maps to xurl -u)' -r
complete -c bird -n "__fish_bird_using_subcommand cache; and __fish_seen_subcommand_from clear" -l output -d 'Error output format: text (default for TTY), json (default for non-TTY)' -r -f -a "text\t'Default: colored, human-readable'
json\t'Machine-readable JSON, no color'"
complete -c bird -n "__fish_bird_using_subcommand cache; and __fish_seen_subcommand_from clear" -l plain -d 'Plain output (no color, no hyperlinks; script-friendly)'
complete -c bird -n "__fish_bird_using_subcommand cache; and __fish_seen_subcommand_from clear" -l no-color -d 'Disable ANSI colors (or set NO_COLOR)'
complete -c bird -n "__fish_bird_using_subcommand cache; and __fish_seen_subcommand_from clear" -l refresh -d 'Bypass store read, still write response to store'
complete -c bird -n "__fish_bird_using_subcommand cache; and __fish_seen_subcommand_from clear" -l no-cache -d 'Disable entity store entirely (no read, no write)'
complete -c bird -n "__fish_bird_using_subcommand cache; and __fish_seen_subcommand_from clear" -l cache-only -d 'Only serve from local store; never make API requests'
complete -c bird -n "__fish_bird_using_subcommand cache; and __fish_seen_subcommand_from clear" -s q -l quiet -d 'Suppress informational stderr output (keep only fatal errors)'
complete -c bird -n "__fish_bird_using_subcommand cache; and __fish_seen_subcommand_from clear" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c bird -n "__fish_bird_using_subcommand cache; and __fish_seen_subcommand_from stats" -s u -l username -d 'Username for multi-user token selection (maps to xurl -u)' -r
complete -c bird -n "__fish_bird_using_subcommand cache; and __fish_seen_subcommand_from stats" -l output -d 'Error output format: text (default for TTY), json (default for non-TTY)' -r -f -a "text\t'Default: colored, human-readable'
json\t'Machine-readable JSON, no color'"
complete -c bird -n "__fish_bird_using_subcommand cache; and __fish_seen_subcommand_from stats" -l pretty
complete -c bird -n "__fish_bird_using_subcommand cache; and __fish_seen_subcommand_from stats" -l plain -d 'Plain output (no color, no hyperlinks; script-friendly)'
complete -c bird -n "__fish_bird_using_subcommand cache; and __fish_seen_subcommand_from stats" -l no-color -d 'Disable ANSI colors (or set NO_COLOR)'
complete -c bird -n "__fish_bird_using_subcommand cache; and __fish_seen_subcommand_from stats" -l refresh -d 'Bypass store read, still write response to store'
complete -c bird -n "__fish_bird_using_subcommand cache; and __fish_seen_subcommand_from stats" -l no-cache -d 'Disable entity store entirely (no read, no write)'
complete -c bird -n "__fish_bird_using_subcommand cache; and __fish_seen_subcommand_from stats" -l cache-only -d 'Only serve from local store; never make API requests'
complete -c bird -n "__fish_bird_using_subcommand cache; and __fish_seen_subcommand_from stats" -s q -l quiet -d 'Suppress informational stderr output (keep only fatal errors)'
complete -c bird -n "__fish_bird_using_subcommand cache; and __fish_seen_subcommand_from stats" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c bird -n "__fish_bird_using_subcommand cache; and __fish_seen_subcommand_from help" -f -a "clear" -d 'Delete all cache entries'
complete -c bird -n "__fish_bird_using_subcommand cache; and __fish_seen_subcommand_from help" -f -a "stats" -d 'Show cache status (JSON default, --pretty for human-readable)'
complete -c bird -n "__fish_bird_using_subcommand cache; and __fish_seen_subcommand_from help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c bird -n "__fish_bird_using_subcommand completions" -s u -l username -d 'Username for multi-user token selection (maps to xurl -u)' -r
complete -c bird -n "__fish_bird_using_subcommand completions" -l output -d 'Error output format: text (default for TTY), json (default for non-TTY)' -r -f -a "text\t'Default: colored, human-readable'
json\t'Machine-readable JSON, no color'"
complete -c bird -n "__fish_bird_using_subcommand completions" -l plain -d 'Plain output (no color, no hyperlinks; script-friendly)'
complete -c bird -n "__fish_bird_using_subcommand completions" -l no-color -d 'Disable ANSI colors (or set NO_COLOR)'
complete -c bird -n "__fish_bird_using_subcommand completions" -l refresh -d 'Bypass store read, still write response to store'
complete -c bird -n "__fish_bird_using_subcommand completions" -l no-cache -d 'Disable entity store entirely (no read, no write)'
complete -c bird -n "__fish_bird_using_subcommand completions" -l cache-only -d 'Only serve from local store; never make API requests'
complete -c bird -n "__fish_bird_using_subcommand completions" -s q -l quiet -d 'Suppress informational stderr output (keep only fatal errors)'
complete -c bird -n "__fish_bird_using_subcommand completions" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c bird -n "__fish_bird_using_subcommand help; and not __fish_seen_subcommand_from login me get post put bookmarks profile search thread delete watchlist usage tweet reply like unlike repost unrepost follow unfollow dm block unblock mute unmute doctor cache completions help" -f -a "login" -d 'Authenticate via xurl (OAuth2 PKCE browser flow)'
complete -c bird -n "__fish_bird_using_subcommand help; and not __fish_seen_subcommand_from login me get post put bookmarks profile search thread delete watchlist usage tweet reply like unlike repost unrepost follow unfollow dm block unblock mute unmute doctor cache completions help" -f -a "me" -d 'Show current user (GET /2/users/me)'
complete -c bird -n "__fish_bird_using_subcommand help; and not __fish_seen_subcommand_from login me get post put bookmarks profile search thread delete watchlist usage tweet reply like unlike repost unrepost follow unfollow dm block unblock mute unmute doctor cache completions help" -f -a "get" -d 'GET request to path (e.g. /2/users/me or /2/users/{id}/bookmarks with -p id=123)'
complete -c bird -n "__fish_bird_using_subcommand help; and not __fish_seen_subcommand_from login me get post put bookmarks profile search thread delete watchlist usage tweet reply like unlike repost unrepost follow unfollow dm block unblock mute unmute doctor cache completions help" -f -a "post" -d 'POST request to path'
complete -c bird -n "__fish_bird_using_subcommand help; and not __fish_seen_subcommand_from login me get post put bookmarks profile search thread delete watchlist usage tweet reply like unlike repost unrepost follow unfollow dm block unblock mute unmute doctor cache completions help" -f -a "put" -d 'PUT request to path'
complete -c bird -n "__fish_bird_using_subcommand help; and not __fish_seen_subcommand_from login me get post put bookmarks profile search thread delete watchlist usage tweet reply like unlike repost unrepost follow unfollow dm block unblock mute unmute doctor cache completions help" -f -a "bookmarks" -d 'List bookmarks for the current user (paginated, max_results=100)'
complete -c bird -n "__fish_bird_using_subcommand help; and not __fish_seen_subcommand_from login me get post put bookmarks profile search thread delete watchlist usage tweet reply like unlike repost unrepost follow unfollow dm block unblock mute unmute doctor cache completions help" -f -a "profile" -d 'Look up a user profile by username'
complete -c bird -n "__fish_bird_using_subcommand help; and not __fish_seen_subcommand_from login me get post put bookmarks profile search thread delete watchlist usage tweet reply like unlike repost unrepost follow unfollow dm block unblock mute unmute doctor cache completions help" -f -a "search" -d 'Search recent tweets (GET /2/tweets/search/recent)'
complete -c bird -n "__fish_bird_using_subcommand help; and not __fish_seen_subcommand_from login me get post put bookmarks profile search thread delete watchlist usage tweet reply like unlike repost unrepost follow unfollow dm block unblock mute unmute doctor cache completions help" -f -a "thread" -d 'Reconstruct a conversation thread from a tweet'
complete -c bird -n "__fish_bird_using_subcommand help; and not __fish_seen_subcommand_from login me get post put bookmarks profile search thread delete watchlist usage tweet reply like unlike repost unrepost follow unfollow dm block unblock mute unmute doctor cache completions help" -f -a "delete" -d 'DELETE request to path'
complete -c bird -n "__fish_bird_using_subcommand help; and not __fish_seen_subcommand_from login me get post put bookmarks profile search thread delete watchlist usage tweet reply like unlike repost unrepost follow unfollow dm block unblock mute unmute doctor cache completions help" -f -a "watchlist" -d 'Monitor users: check recent activity, manage watchlist'
complete -c bird -n "__fish_bird_using_subcommand help; and not __fish_seen_subcommand_from login me get post put bookmarks profile search thread delete watchlist usage tweet reply like unlike repost unrepost follow unfollow dm block unblock mute unmute doctor cache completions help" -f -a "usage" -d 'View API usage and costs'
complete -c bird -n "__fish_bird_using_subcommand help; and not __fish_seen_subcommand_from login me get post put bookmarks profile search thread delete watchlist usage tweet reply like unlike repost unrepost follow unfollow dm block unblock mute unmute doctor cache completions help" -f -a "tweet" -d 'Post a tweet (via xurl)'
complete -c bird -n "__fish_bird_using_subcommand help; and not __fish_seen_subcommand_from login me get post put bookmarks profile search thread delete watchlist usage tweet reply like unlike repost unrepost follow unfollow dm block unblock mute unmute doctor cache completions help" -f -a "reply" -d 'Reply to a tweet (via xurl)'
complete -c bird -n "__fish_bird_using_subcommand help; and not __fish_seen_subcommand_from login me get post put bookmarks profile search thread delete watchlist usage tweet reply like unlike repost unrepost follow unfollow dm block unblock mute unmute doctor cache completions help" -f -a "like" -d 'Like a tweet (via xurl)'
complete -c bird -n "__fish_bird_using_subcommand help; and not __fish_seen_subcommand_from login me get post put bookmarks profile search thread delete watchlist usage tweet reply like unlike repost unrepost follow unfollow dm block unblock mute unmute doctor cache completions help" -f -a "unlike" -d 'Unlike a tweet (via xurl)'
complete -c bird -n "__fish_bird_using_subcommand help; and not __fish_seen_subcommand_from login me get post put bookmarks profile search thread delete watchlist usage tweet reply like unlike repost unrepost follow unfollow dm block unblock mute unmute doctor cache completions help" -f -a "repost" -d 'Repost (retweet) a tweet (via xurl)'
complete -c bird -n "__fish_bird_using_subcommand help; and not __fish_seen_subcommand_from login me get post put bookmarks profile search thread delete watchlist usage tweet reply like unlike repost unrepost follow unfollow dm block unblock mute unmute doctor cache completions help" -f -a "unrepost" -d 'Undo a repost (via xurl)'
complete -c bird -n "__fish_bird_using_subcommand help; and not __fish_seen_subcommand_from login me get post put bookmarks profile search thread delete watchlist usage tweet reply like unlike repost unrepost follow unfollow dm block unblock mute unmute doctor cache completions help" -f -a "follow" -d 'Follow a user (via xurl)'
complete -c bird -n "__fish_bird_using_subcommand help; and not __fish_seen_subcommand_from login me get post put bookmarks profile search thread delete watchlist usage tweet reply like unlike repost unrepost follow unfollow dm block unblock mute unmute doctor cache completions help" -f -a "unfollow" -d 'Unfollow a user (via xurl)'
complete -c bird -n "__fish_bird_using_subcommand help; and not __fish_seen_subcommand_from login me get post put bookmarks profile search thread delete watchlist usage tweet reply like unlike repost unrepost follow unfollow dm block unblock mute unmute doctor cache completions help" -f -a "dm" -d 'Send a direct message (via xurl)'
complete -c bird -n "__fish_bird_using_subcommand help; and not __fish_seen_subcommand_from login me get post put bookmarks profile search thread delete watchlist usage tweet reply like unlike repost unrepost follow unfollow dm block unblock mute unmute doctor cache completions help" -f -a "block" -d 'Block a user (via xurl)'
complete -c bird -n "__fish_bird_using_subcommand help; and not __fish_seen_subcommand_from login me get post put bookmarks profile search thread delete watchlist usage tweet reply like unlike repost unrepost follow unfollow dm block unblock mute unmute doctor cache completions help" -f -a "unblock" -d 'Unblock a user (via xurl)'
complete -c bird -n "__fish_bird_using_subcommand help; and not __fish_seen_subcommand_from login me get post put bookmarks profile search thread delete watchlist usage tweet reply like unlike repost unrepost follow unfollow dm block unblock mute unmute doctor cache completions help" -f -a "mute" -d 'Mute a user (via xurl)'
complete -c bird -n "__fish_bird_using_subcommand help; and not __fish_seen_subcommand_from login me get post put bookmarks profile search thread delete watchlist usage tweet reply like unlike repost unrepost follow unfollow dm block unblock mute unmute doctor cache completions help" -f -a "unmute" -d 'Unmute a user (via xurl)'
complete -c bird -n "__fish_bird_using_subcommand help; and not __fish_seen_subcommand_from login me get post put bookmarks profile search thread delete watchlist usage tweet reply like unlike repost unrepost follow unfollow dm block unblock mute unmute doctor cache completions help" -f -a "doctor" -d 'Show what is available: xurl status, commands, and entity store health'
complete -c bird -n "__fish_bird_using_subcommand help; and not __fish_seen_subcommand_from login me get post put bookmarks profile search thread delete watchlist usage tweet reply like unlike repost unrepost follow unfollow dm block unblock mute unmute doctor cache completions help" -f -a "cache" -d 'Manage the HTTP response cache'
complete -c bird -n "__fish_bird_using_subcommand help; and not __fish_seen_subcommand_from login me get post put bookmarks profile search thread delete watchlist usage tweet reply like unlike repost unrepost follow unfollow dm block unblock mute unmute doctor cache completions help" -f -a "completions" -d 'Generate shell completions'
complete -c bird -n "__fish_bird_using_subcommand help; and not __fish_seen_subcommand_from login me get post put bookmarks profile search thread delete watchlist usage tweet reply like unlike repost unrepost follow unfollow dm block unblock mute unmute doctor cache completions help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c bird -n "__fish_bird_using_subcommand help; and __fish_seen_subcommand_from watchlist" -f -a "check" -d 'Check recent activity for all watched users'
complete -c bird -n "__fish_bird_using_subcommand help; and __fish_seen_subcommand_from watchlist" -f -a "add" -d 'Add a user to the watchlist'
complete -c bird -n "__fish_bird_using_subcommand help; and __fish_seen_subcommand_from watchlist" -f -a "remove" -d 'Remove a user from the watchlist'
complete -c bird -n "__fish_bird_using_subcommand help; and __fish_seen_subcommand_from watchlist" -f -a "list" -d 'Show the current watchlist'
complete -c bird -n "__fish_bird_using_subcommand help; and __fish_seen_subcommand_from cache" -f -a "clear" -d 'Delete all cache entries'
complete -c bird -n "__fish_bird_using_subcommand help; and __fish_seen_subcommand_from cache" -f -a "stats" -d 'Show cache status (JSON default, --pretty for human-readable)'
