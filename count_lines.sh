#!/bin/bash

# Script to count lines of code in all Rust files in the project

echo "=== Rust code line count ==="
echo

# Find all .rs files and count lines, excluding files in the target folder
echo "Details per file:"
echo "-------------------"
# Exclude files in the target folder
# Use find to locate all .rs files
find . -name "*.rs" -type f | grep -v "./target" | sort | while read file; do
    lines=$(wc -l < "$file")
    echo "$lines lines : $file"
done

# Calculate the total
total_lines=$(find . -name "*.rs" -type f -exec wc -l {} \; | awk '{total += $1} END {print total}')
total_files=$(find . -name "*.rs" -type f | wc -l)

echo
echo "-------------------"
echo "Total: $total_lines lines in $total_files .rs files"
echo

# Statistics per folder
echo "Total per folder:"
echo "-------------------"
for dir in $(find . -name "*.rs" -type f | grep -v "./target" | xargs dirname | sort | uniq); do
    dir_lines=$(find "$dir" -name "*.rs" -type f -exec wc -l {} \; | awk '{total += $1} END {print total}')
    dir_files=$(find "$dir" -name "*.rs" -type f | wc -l | tr -d ' ')
    printf "%5d lines in %2d files : %s\n" "$dir_lines" "$dir_files" "$dir"
done | sort -nr

# Exclude comments and empty lines
echo
echo "Lines of code (excluding comments and empty lines):"
echo "-------------------"
code_lines=$(find . -name "*.rs" -type f -not -path ./target -exec grep -v -E '^\s*(//|$)' {} \; | wc -l)
echo "$code_lines lines of code (excluding comments and empty lines)"

# Bonus: Top 5 longest files
echo
echo "Top 5 longest files:"
echo "-------------------"
find . -name "*.rs" -type f -not -path ./target -exec wc -l {} \; | sort -nr | head -n 5 | sed 's/^\s*//' | sed 's/ /\t/' | awk '{printf "%5d lines : %s\n", $1, $2}'
