#!/usr/bin/env python3
# Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
# This file is part of the rust-photoacoustic project and is licensed under the
# SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).
"""
Script to remove 'auto-initialize' feature from pyo3 dependency in Cargo.toml
Uses tomli for robust TOML parsing.
"""

import sys
import os

try:
    import tomllib
except ImportError:
    try:
        import tomli as tomllib
    except ImportError:
        print("Error: Neither tomllib nor tomli available. Install with: pip install tomli tomli_w")
        sys.exit(1)

try:
    import tomli_w
except ImportError:
    print("Error: tomli_w not available. Install with: pip install tomli_w")
    sys.exit(1)


def remove_auto_initialize(cargo_toml_path: str) -> None:
    """
    Remove 'auto-initialize' feature from pyo3 dependency in Cargo.toml
    
    Args:
        cargo_toml_path: Path to Cargo.toml file
    """
    if not os.path.exists(cargo_toml_path):
        print(f"Error: {cargo_toml_path} not found")
        sys.exit(1)
    
    # Read and parse the TOML file
    with open(cargo_toml_path, 'rb') as f:
        data = tomllib.load(f)
    
    # Check if pyo3 exists in dependencies
    if 'dependencies' not in data or 'pyo3' not in data['dependencies']:
        print("Warning: pyo3 not found in dependencies")
        return
    
    pyo3_dep = data['dependencies']['pyo3']
    
    # If it's a string (version), skip
    if isinstance(pyo3_dep, str):
        print("pyo3 is a simple version string, no features to remove")
        return
    
    # If features list exists, remove 'auto-initialize'
    if 'features' in pyo3_dep and isinstance(pyo3_dep['features'], list):
        original_count = len(pyo3_dep['features'])
        pyo3_dep['features'] = [
            f for f in pyo3_dep['features']
            if f != 'auto-initialize'
        ]
        removed_count = original_count - len(pyo3_dep['features'])
        
        if removed_count > 0:
            print(f"Removed 'auto-initialize' feature from pyo3 ({removed_count} occurrence(s))")
        else:
            print("'auto-initialize' feature not found in pyo3 features list")
    else:
        print("No features list found in pyo3 dependency")
    
    # Write back to the file
    with open(cargo_toml_path, 'wb') as f:
        tomli_w.dump(data, f)
    
    print(f"Successfully updated {cargo_toml_path}")


if __name__ == '__main__':
    cargo_path = '/rust-photoacoustic/rust/Cargo.toml'
    if len(sys.argv) > 1:
        cargo_path = sys.argv[1]
    
    remove_auto_initialize(cargo_path)
