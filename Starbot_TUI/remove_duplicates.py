#!/usr/bin/env python3
"""
Remove duplicate handler functions from tui.rs
"""

def remove_function(lines, start_line, func_name):
    """Remove a function from lines, finding the matching closing brace."""
    # start_line is 0-indexed
    if start_line >= len(lines):
        return lines

    # Count braces to find function end
    brace_count = 0
    i = start_line
    started = False

    while i < len(lines):
        line = lines[i]
        for char in line:
            if char == '{':
                brace_count += 1
                started = True
            elif char == '}':
                brace_count -= 1
                if started and brace_count == 0:
                    # Found closing brace, delete from start_line to i (inclusive)
                    print(f"  Removing {func_name}: lines {start_line+1}-{i+1} ({i-start_line+1} lines)")
                    return lines[:start_line] + lines[i+1:]
        i += 1

    print(f"  WARNING: Could not find closing brace for {func_name}")
    return lines

# Read file
with open('src/commands/tui.rs', 'r') as f:
    lines = f.readlines()

print(f"Original file: {len(lines)} lines")

# Functions to remove (name, approximate line number - 1-indexed)
# Working from bottom to top to maintain line numbers
functions_to_remove = [
    ('extract_usage_line', 3389),
    ('extract_provider_model', 3363),
    ('extract_reply', 3352),
    ('send_chat_text', 1543),
    ('retry_last_chat', 1512),
    ('apply_workspace_selection', 1475),
    ('move_selection_wrap', 1459),
    ('move_selection', 1443),
    ('handle_key', 1020),
    ('handle_event', 1005),
    ('handle_tui_msg', 532),
    ('build_chat_body', 505),
    ('spawn_chat_request', 490),
    ('spawn_tool_propose', 467),
    ('spawn_workspaces_fetch', 460),
    ('spawn_health_fetch', 453),
    ('spawn_models_fetch', 446),
]

# Sort by line number descending (remove from bottom to top)
functions_to_remove.sort(key=lambda x: x[1], reverse=True)

for func_name, line_num in functions_to_remove:
    # Find the function in the file (search nearby in case line numbers shifted)
    found = False
    for offset in range(-5, 10):
        check_line = line_num - 1 + offset  # Convert to 0-indexed
        if check_line < 0 or check_line >= len(lines):
            continue
        if f'fn {func_name}(' in lines[check_line]:
            lines = remove_function(lines, check_line, func_name)
            found = True
            break

    if not found:
        print(f"  WARNING: Could not find {func_name} near line {line_num}")

# Write back
with open('src/commands/tui.rs', 'w') as f:
    f.writelines(lines)

print(f"\n✅ Done! New file: {len(lines)} lines")
print(f"   Removed: {len(functions_to_remove)} functions")
