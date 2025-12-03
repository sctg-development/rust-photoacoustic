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

def convert_mermaid_to_svg(mmd_files, temp_dir):
    """Convertit les fichiers Mermaid en images SVG avec rendu du texte optimisé pour Word"""
    svg_files = []
    for mmd_file in mmd_files:
        svg_file = mmd_file.replace('.mmd', '.svg')
        svg_id = Path(svg_file).stem
        try:
            # Configuration Mermaid pour utiliser du texte SVG natif (pas HTML dans foreignObject)
            config_file = mmd_file.replace('.mmd', '_config.json')
            with open(config_file, 'w', encoding='utf-8') as f:
                f.write('{"htmlLabels": false, "fontFamily": "Arial"}')
            
            # Utiliser SVG avec configuration pour l'encodage UTF-8 et texte natif
            subprocess.run([
                'mmdc',
                '-i', mmd_file,
                '-o', svg_file,
                '-c', config_file,
                '--backgroundColor', 'transparent'
            ], check=True, capture_output=True, env=dict(os.environ, LANG='C.UTF-8'))
            svg_files.append(svg_file)
            print(f"✅ Diagramme converti : {os.path.basename(svg_file)}")
        except subprocess.CalledProcessError as e:
            print(f"❌ Erreur lors de la conversion de {mmd_file}: {e}")
            svg_files.append(None)
    return svg_files

def replace_mermaid_with_images(markdown_content, svg_files):
    """Remplace les blocs Mermaid par des références d'images"""
    pattern = r'```mermaid\n.*?\n```'
    svg_index = 0

    def replace_func(match):
        nonlocal svg_index
        if svg_index < len(svg_files) and svg_files[svg_index] is not None:
            image_path = svg_files[svg_index]
            # Utiliser seulement le nom du fichier, pas le chemin complet
            image_filename = os.path.basename(image_path)
            svg_index += 1
            return f'![Diagramme {svg_index}]({image_filename})'
        else:
            svg_index += 1
            return '[Diagramme non disponible]'
    
    return re.sub(pattern, replace_func, markdown_content, flags=re.DOTALL)

def convert_to_docx(markdown_file, output_file, temp_dir):
    """Convertit le fichier Markdown modifié en DOCX avec Pandoc"""
    try:
        # Changer vers le répertoire temporaire pour que les chemins relatifs fonctionnent
        original_cwd = os.getcwd()
        os.chdir(temp_dir)
        
        try:
            cmd = [
                'pandoc',
                os.path.basename(markdown_file),  # Utiliser seulement le nom du fichier
                '-o', os.path.join(original_cwd, output_file),  # Chemin absolu pour la sortie
                '--from', 'markdown',
                '--to', 'docx'
            ]
            
            subprocess.run(cmd, check=True)
            print(f"✅ Document DOCX créé : {output_file}")
            return True
        finally:
            os.chdir(original_cwd)  # Remettre le répertoire original
            
    except subprocess.CalledProcessError as e:
        print(f"❌ Erreur lors de la conversion Pandoc : {e}")
        return False

def main():
    import sys
    
    # Vérifier les arguments
    if len(sys.argv) != 2:
        print("Usage: python3 convert_to_docx.py <fichier.md>")
        print("Exemple: python3 convert_to_docx.py investor_presentation_fr.md")
        return
    
    input_file = sys.argv[1]
    
    # Générer le nom du fichier de sortie en remplaçant l'extension
    if input_file.endswith('.md'):
        output_file = input_file[:-3] + '.docx'
    else:
        output_file = input_file + '.docx'
    
    if not os.path.exists(input_file):
        print(f"❌ Fichier non trouvé : {input_file}")
        return
    
    # Lire le fichier Markdown
    with open(input_file, 'r', encoding='utf-8') as f:
        content = f.read()
    
    print(f"📖 Lecture du fichier : {input_file}")
    
    # Créer un dossier temporaire automatiquement géré
    with tempfile.TemporaryDirectory() as temp_dir:
        print(f"📁 Dossier temporaire : {temp_dir}")
        
        # Extraire les diagrammes Mermaid
        diagrams = extract_mermaid_diagrams(content)
        print(f"🔍 {len(diagrams)} diagramme(s) Mermaid trouvé(s)")
        
        if diagrams:
            # Créer les fichiers Mermaid
            mmd_files = create_mermaid_files(diagrams, temp_dir)
            
            # Convertir en SVG
            svg_files = convert_mermaid_to_svg(mmd_files, temp_dir)
            
            # Remplacer les diagrammes par des images
            modified_content = replace_mermaid_with_images(content, svg_files)
        else:
            modified_content = content
        
        # Créer le fichier Markdown modifié
        temp_md = os.path.join(temp_dir, 'temp_presentation.md')
        with open(temp_md, 'w', encoding='utf-8') as f:
            f.write(modified_content)
        
        # Convertir en DOCX
        if convert_to_docx(temp_md, output_file, temp_dir):
            print(f"\n🎉 Conversion terminée avec succès !")
            print(f"📄 Fichier de sortie : {output_file}")
        else:
            print("\n❌ Échec de la conversion")

if __name__ == '__main__':
    main()