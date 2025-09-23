#!/usr/bin/env python3
"""
Script pour convertir un document Markdown avec diagrammes Mermaid en DOCX
Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
This file is part of the rust-photoacoustic project and is licensed under the
SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).
"""

import re
import os
import subprocess
import tempfile
from pathlib import Path

def extract_mermaid_diagrams(markdown_content):
    """Extrait les diagrammes Mermaid du contenu Markdown"""
    pattern = r'```mermaid\n(.*?)\n```'
    diagrams = re.findall(pattern, markdown_content, re.DOTALL)
    return diagrams

def create_mermaid_files(diagrams, temp_dir):
    """Crée des fichiers .mmd temporaires pour chaque diagramme"""
    diagram_files = []
    for i, diagram in enumerate(diagrams):
        mmd_file = os.path.join(temp_dir, f'diagram_{i}.mmd')
        with open(mmd_file, 'w', encoding='utf-8') as f:
            f.write(diagram.strip())
        diagram_files.append(mmd_file)
    return diagram_files

def convert_mermaid_to_png(mmd_files, temp_dir):
    """Convertit les fichiers Mermaid en images PNG"""
    png_files = []
    for mmd_file in mmd_files:
        png_file = mmd_file.replace('.mmd', '.png')
        try:
            subprocess.run(['mmdc', '-i', mmd_file, '-o', png_file, '--backgroundColor', 'white'], 
                         check=True, capture_output=True)
            png_files.append(png_file)
            print(f"✅ Diagramme converti : {os.path.basename(png_file)}")
        except subprocess.CalledProcessError as e:
            print(f"❌ Erreur lors de la conversion de {mmd_file}: {e}")
            png_files.append(None)
    return png_files

def replace_mermaid_with_images(markdown_content, png_files):
    """Remplace les blocs Mermaid par des références d'images"""
    pattern = r'```mermaid\n.*?\n```'
    png_index = 0
    
    def replace_func(match):
        nonlocal png_index
        if png_index < len(png_files) and png_files[png_index] is not None:
            image_path = png_files[png_index]
            png_index += 1
            return f'![Diagramme {png_index}]({image_path})'
        else:
            png_index += 1
            return '[Diagramme non disponible]'
    
    return re.sub(pattern, replace_func, markdown_content, flags=re.DOTALL)

def convert_to_docx(markdown_file, output_file):
    """Convertit le fichier Markdown modifié en DOCX avec Pandoc"""
    try:
        cmd = [
            'pandoc',
            markdown_file,
            '-o', output_file,
            '--from', 'markdown',
            '--to', 'docx'
        ]
        
        subprocess.run(cmd, check=True)
        print(f"✅ Document DOCX créé : {output_file}")
        return True
    except subprocess.CalledProcessError as e:
        print(f"❌ Erreur lors de la conversion Pandoc : {e}")
        return False

def main():
    input_file = 'investor_presentation_fr.md'
    output_file = 'investor_presentation_fr.docx'
    
    if not os.path.exists(input_file):
        print(f"❌ Fichier non trouvé : {input_file}")
        return
    
    # Lire le fichier Markdown
    with open(input_file, 'r', encoding='utf-8') as f:
        content = f.read()
    
    print(f"📖 Lecture du fichier : {input_file}")
    
    # Créer un dossier temporaire
    with tempfile.TemporaryDirectory() as temp_dir:
        print(f"📁 Dossier temporaire : {temp_dir}")
        
        # Extraire les diagrammes Mermaid
        diagrams = extract_mermaid_diagrams(content)
        print(f"🔍 {len(diagrams)} diagramme(s) Mermaid trouvé(s)")
        
        if diagrams:
            # Créer les fichiers Mermaid
            mmd_files = create_mermaid_files(diagrams, temp_dir)
            
            # Convertir en PNG
            png_files = convert_mermaid_to_png(mmd_files, temp_dir)
            
            # Remplacer les diagrammes par des images
            modified_content = replace_mermaid_with_images(content, png_files)
        else:
            modified_content = content
        
        # Créer le fichier Markdown modifié
        temp_md = os.path.join(temp_dir, 'temp_presentation.md')
        with open(temp_md, 'w', encoding='utf-8') as f:
            f.write(modified_content)
        
        # Convertir en DOCX
        if convert_to_docx(temp_md, output_file):
            print(f"\n🎉 Conversion terminée avec succès !")
            print(f"📄 Fichier de sortie : {output_file}")
        else:
            print("\n❌ Échec de la conversion")

if __name__ == '__main__':
    main()