#!/bin/bash

# Script pour compter les lignes de code dans tous les fichiers Rust du projet

echo "=== Comptage des lignes de code Rust ==="
echo

# Trouver tous les fichiers .rs et compter les lignes en excluant les fichiers dans le dossier target
echo "Détail par fichier:"
echo "-------------------"
# Exclure les fichiers dans le dossier target
# Utiliser find pour trouver tous les fichiers .rs
find . -name "*.rs" -type f | grep -v "./target" | sort | while read file; do
    lines=$(wc -l < "$file")
    echo "$lines lignes : $file"
done

# Calculer le total
total_lines=$(find . -name "*.rs" -type f -exec wc -l {} \; | awk '{total += $1} END {print total}')
total_files=$(find . -name "*.rs" -type f | wc -l)

echo
echo "-------------------"
echo "Total: $total_lines lignes dans $total_files fichiers .rs"
echo

# Statistiques par dossier
echo "Total par dossier:"
echo "-------------------"
for dir in $(find . -name "*.rs" -type f | grep -v "./target" | xargs dirname | sort | uniq); do
    dir_lines=$(find "$dir" -name "*.rs" -type f -exec wc -l {} \; | awk '{total += $1} END {print total}')
    dir_files=$(find "$dir" -name "*.rs" -type f | wc -l | tr -d ' ')
    printf "%5d lignes dans %2d fichiers : %s\n" "$dir_lines" "$dir_files" "$dir"
done | sort -nr

# Exclure les commentaires et les lignes vides
echo
echo "Lignes de code (sans commentaires ni lignes vides):"
echo "-------------------"
code_lines=$(find . -name "*.rs" -type f -not -path ./target -exec grep -v -E '^\s*(//|$)' {} \; | wc -l)
echo "$code_lines lignes de code (sans commentaires ni lignes vides)"

# Bonus: Top 5 des fichiers les plus longs
echo
echo "Top 5 des fichiers les plus longs:"
echo "-------------------"
find . -name "*.rs" -type f -not -path ./target -exec wc -l {} \; | sort -nr | head -n 5 | sed 's/^\s*//' | sed 's/ /\t/' | awk '{printf "%5d lignes : %s\n", $1, $2}'
