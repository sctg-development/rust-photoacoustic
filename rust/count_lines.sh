#!/bin/bash
# Copyright (c) 2025, Ronan Le Meillat
# This file is part of the Rust Photoacoustic project.
# SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).


# ===================================
# Configuration
# ===================================
FILE_EXTENSION="${1:-.rs}"        # Allow passing extension as first argument
EXCLUDE_PATHS=("./target/" "./oxide*" "./oxide-auth-rocket-patched/target" "./oxide-auth-rocket-patched/target/*" "./target/*")
SCRIPT_VERSION="1.2.0"

# ===================================
# Helper Functions
# ===================================
print_header() {
    echo "=== Code Metrics for ${FILE_EXTENSION} files (v${SCRIPT_VERSION}) ==="
    echo "Run at: $(date)"
    echo
}

# Build exclusion parameters for find command
build_find_exclude_params() {
    local params=""
    for path in "${EXCLUDE_PATHS[@]}"; do
        params+=" ! -path \"${path}\""
    done
    echo "$params"
}

# Find all files matching the criteria with exclusions applied
find_files() {
    local exclude_params=$(build_find_exclude_params)
    eval "find . -name \"*${FILE_EXTENSION}\" -type f ${exclude_params} ! -path \"*/\.*\" ! -path \"*/target/*\""
}

# Get all eligible files as an array
get_files_array() {
    local files=()
    while IFS= read -r line; do
        files+=("$line")
    done < <(find_files)
    echo "${files[@]}"
}

# Count lines in files matching criteria
count_lines() {
    local count=0
    for file in $(find_files); do
        lines=$(wc -l < "$file")
        ((count += lines))
    done
    echo $count
}

# Count number of files matching criteria
count_files() {
    find_files | wc -l | tr -d ' '
}

# Count non-comment, non-empty lines
count_code_lines() {
    local count=0
    for file in $(find_files); do
        # Only count lines that are not comments or empty
        code_lines=$(grep -v -E '^\s*(//|$)' "$file" 2>/dev/null | wc -l | tr -d ' ')
        ((count += code_lines))
    done
    echo $count
}

# ===================================
# Metrics Collection
# ===================================

# Print per-file details
print_file_details() {
    echo "Details per file:"
    echo "-------------------"
    
    find_files | sort | while read -r file; do
        lines=$(wc -l < "$file")
        code_lines=$(grep -v -E '^\s*(//|$)' "$file" | wc -l)
        comment_lines=$((lines - code_lines))
        printf "%5d lines (%4d code, %4d comments) : %s\n" "$lines" "$code_lines" "$comment_lines" "$file"
    done
}

# Print per-directory statistics
print_dir_statistics() {
    echo "Total per folder (sorted by line count):"
    echo "-------------------"
    
    # First get a list of all directories containing matching files
    local dirs=()
    while IFS= read -r dir; do
        dirs+=("$dir")
    done < <(find_files | xargs dirname | sort | uniq)
    
    # Process each directory
    for dir in "${dirs[@]}"; do
        local dir_files=()
        while IFS= read -r file; do
            dir_files+=("$file")
        # Use find_files with a directory filter to ensure consistent exclusions
        done < <(find_files | grep "^$dir/")
        
        # Skip if no matching files found
        if [[ ${#dir_files[@]} -eq 0 ]]; then
            continue
        fi
        
        # Calculate metrics
        local dir_lines=0
        local dir_code_lines=0
        
        for file in "${dir_files[@]}"; do
            lines=$(wc -l < "$file")
            code_lines=$(grep -v -E '^\s*(//|$)' "$file" | wc -l)
            ((dir_lines += lines))
            ((dir_code_lines += code_lines))
        done
        
        local dir_file_count=${#dir_files[@]}
        printf "%5d lines (%5d code) in %2d files : %s\n" "$dir_lines" "$dir_code_lines" "$dir_file_count" "$dir"
    done | sort -nr
}

# Print top N longest files
print_top_files() {
    local top_count=5
    echo "Top ${top_count} longest files:"
    echo "-------------------"
    
    find_files | xargs wc -l 2>/dev/null | sort -nr | grep -v 'total' | head -n "$top_count" | awk '{if (NF>1) {printf "%5d lines : %s\n", $1, $2}}'
}

# Print code to comment ratio
print_code_comment_ratio() {
    total_lines=$(count_lines)
    code_lines=$(count_code_lines)
    comment_lines=$((total_lines - code_lines))
    ratio=$(awk "BEGIN {printf \"%.2f\", ($comment_lines / ($total_lines + 1) * 100)}")
    
    echo "Lines of code (excluding comments and empty lines):"
    echo "-------------------"
    echo "  Total lines: $total_lines"
    echo "  Code lines: $code_lines"
    echo "  Comment lines: $comment_lines"
    echo "  Code to comment ratio: $ratio %"
}

# Print summary metrics
print_summary() {
    total_lines=$(count_lines)
    total_files=$(count_files)
    
    echo "-------------------"
    echo "Total: $total_lines lines in $total_files ${FILE_EXTENSION} files"
    echo "-------------------"
}

# ===================================
# Main Execution
# ===================================
main() {
    print_header
    print_file_details
    echo
    print_summary
    echo
    print_dir_statistics
    echo 
    print_code_comment_ratio
    echo
    print_top_files
}

# Run the script
main
