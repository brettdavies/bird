#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.10"
# dependencies = []
# ///
"""Lifted from generate-changelog.sh — categorize merged PRs into CHANGELOG.md sections.

Invoked by generate-changelog.sh with positional args:
    OWNER REPO PR_LIST CHANGELOG_PATH VERSION TAG

PR_LIST is a comma-separated list of PR numbers.
"""

import json, re, subprocess, sys

owner = sys.argv[1]
repo = sys.argv[2]
pr_numbers = [int(n) for n in sys.argv[3].split(',')]
changelog_path = sys.argv[4]
version = sys.argv[5]
tag = sys.argv[6] if len(sys.argv) > 6 else f'v{version}'
tag_prefix = 'v' if tag.startswith('v') else ''

CATEGORIES = ['Added', 'Changed', 'Fixed', 'Documentation']

def fetch_pr(num):
    """Fetch PR body and author from GitHub API."""
    try:
        result = subprocess.run(
            ['gh', 'api', f'repos/{owner}/{repo}/pulls/{num}',
             '--jq', '{body: .body, author: .user.login}'],
            capture_output=True, text=True, timeout=10
        )
        if result.returncode == 0:
            return json.loads(result.stdout)
    except Exception:
        pass
    return None

def extract_changelog_sections(body):
    """Extract categorized bullets from ## Changelog section with ### subsections."""
    sections = {}
    if not body:
        return sections

    changelog_match = re.search(r'^## Changelog\s*$', body, re.MULTILINE)
    if not changelog_match:
        return sections

    rest = body[changelog_match.end():]
    next_h2 = re.search(r'^## ', rest, re.MULTILINE)
    changelog_content = rest[:next_h2.start()] if next_h2 else rest

    current_section = None
    for line in changelog_content.split('\n'):
        h3_match = re.match(r'^### (.+)', line)
        if h3_match:
            current_section = h3_match.group(1).strip()
            if current_section not in sections:
                sections[current_section] = []
        elif current_section and re.match(r'^- ', line):
            sections[current_section].append(line)
        elif current_section and sections.get(current_section) and re.match(r'^  \S', line):
            # Continuation line (indented, part of previous bullet) — join to last bullet
            sections[current_section][-1] = sections[current_section][-1].rstrip() + ' ' + line.strip()

    return sections

def extract_flat_changes(body):
    """Fallback: extract flat bullet list from ## Changes section."""
    bullets = []
    if not body:
        return bullets

    changes_match = re.search(r'^## Changes\s*$', body, re.MULTILINE)
    if not changes_match:
        return bullets

    rest = body[changes_match.end():]
    next_h2 = re.search(r'^## ', rest, re.MULTILINE)
    changes_content = rest[:next_h2.start()] if next_h2 else rest

    for line in changes_content.split('\n'):
        if re.match(r'^- ', line):
            bullets.append(line)
        elif bullets and re.match(r'^  \S', line):
            # Continuation line (indented, part of previous bullet) — join to last bullet
            bullets[-1] = bullets[-1].rstrip() + ' ' + line.strip()

    return bullets

# Collect all categorized entries from PR bodies
all_entries = {}  # category -> list of bullets
for num in pr_numbers:
    pr_data = fetch_pr(num)
    if not pr_data:
        continue

    body = pr_data.get('body', '') or ''
    author = pr_data.get('author', '')
    attrib = f' by @{author} in [#{num}](https://github.com/{owner}/{repo}/pull/{num})' if author else ''

    # Try new template format first (## Changelog with ### subsections)
    sections = extract_changelog_sections(body)

    if sections:
        for category, bullets in sections.items():
            if not bullets:
                continue
            if category not in all_entries:
                all_entries[category] = []
            first = True
            for bullet in bullets:
                if first and ' by @' not in bullet:
                    all_entries[category].append(bullet + attrib)
                else:
                    all_entries[category].append(bullet)
                first = False
    else:
        # Fallback: flat ## Changes section
        flat = extract_flat_changes(body)
        if flat:
            category = 'Changed'
            if category not in all_entries:
                all_entries[category] = []
            first = True
            for bullet in flat:
                if first and ' by @' not in bullet:
                    all_entries[category].append(bullet + attrib)
                else:
                    all_entries[category].append(bullet)
                first = False

if not all_entries:
    sys.exit(0)

with open(changelog_path, 'r') as f:
    content = f.read()

# Find the version section header line (preserve it with the date)
header_pattern = rf'^## \[{re.escape(version)}\].*$'
header_match = re.search(header_pattern, content, re.MULTILINE)
if not header_match:
    sys.exit(0)

header_line = header_match.group(0)

new_section = header_line + '\n'
for cat in CATEGORIES:
    if cat in all_entries and all_entries[cat]:
        new_section += f'\n### {cat}\n\n'
        for bullet in all_entries[cat]:
            new_section += bullet + '\n'

# Include any categories not in the standard list
for cat in all_entries:
    if cat not in CATEGORIES and all_entries[cat]:
        new_section += f'\n### {cat}\n\n'
        for bullet in all_entries[cat]:
            new_section += bullet + '\n'

# Find the previous version tag for the Full Changelog link
prev_match = re.search(rf'## \[{re.escape(version)}\].*?\n## \[([^\]]+)\]', content, re.DOTALL)
if prev_match:
    prev_version = prev_match.group(1)
    new_section += f'\n**Full Changelog**: [{tag_prefix}{prev_version}...{tag_prefix}{version}](https://github.com/{owner}/{repo}/compare/{tag_prefix}{prev_version}...{tag_prefix}{version})\n'

# Replace the version section in the file
section_pattern = rf'## \[{re.escape(version)}\].*?(?=\n## \[|\Z)'
new_content = re.sub(section_pattern, new_section.rstrip() + '\n', content, count=1, flags=re.DOTALL)

with open(changelog_path, 'w') as f:
    f.write(new_content)
