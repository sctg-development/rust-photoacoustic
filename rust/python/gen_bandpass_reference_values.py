#!/usr/bin/env python3
"""
Générateur de valeurs de référence pour le filtre passe-bande BandpassFilter

Ce script utilise scipy.signal pour générer des valeurs de référence qui serviront
à valider l'implémentation Rust du BandpassFilter pour différents ordres (2, 4, 6, 8, 10).

Il génère des signaux de test avec différentes fréquences et calcule la réponse 
du filtre scipy pour chaque configuration, puis sauvegarde les résultats en JSON
pour les tests d'intégration Rust.
"""

import numpy as np
import scipy.signal as signal
import json
import matplotlib.pyplot as plt
from pathlib import Path

# Configuration des tests
SAMPLE_RATE = 48000
CENTER_FREQ = 1000.0  # Hz
BANDWIDTH = 200.0     # Hz
ORDERS = [2, 4, 6, 8, 10]
SIGNAL_LENGTH = 1024  # Nombre d'échantillons

def create_test_signals():
    """Crée différents signaux de test pour valider le filtre"""
    t = np.arange(SIGNAL_LENGTH) / SAMPLE_RATE
    
    signals = {}
    
    # Signal sinusoïdal à la fréquence centrale (devrait passer)
    signals['center_freq'] = np.sin(2 * np.pi * CENTER_FREQ * t)
    
    # Signal sinusoïdal dans la bande passante (devrait passer)
    signals['in_band'] = np.sin(2 * np.pi * (CENTER_FREQ + BANDWIDTH/4) * t)
    
    # Signal sinusoïdal en dehors de la bande (devrait être atténué)
    signals['out_of_band_low'] = np.sin(2 * np.pi * (CENTER_FREQ - BANDWIDTH) * t)
    signals['out_of_band_high'] = np.sin(2 * np.pi * (CENTER_FREQ + BANDWIDTH) * t)
    
    # Signal à impulsion (pour tester la réponse impulsionnelle)
    impulse = np.zeros(SIGNAL_LENGTH)
    impulse[0] = 1.0
    signals['impulse'] = impulse
    
    # Signal de bruit blanc (pour tester le filtrage du bruit)
    np.random.seed(42)  # Pour la reproductibilité
    signals['white_noise'] = np.random.normal(0, 0.1, SIGNAL_LENGTH)
    
    # Signal multi-fréquences
    multi_freq = (np.sin(2 * np.pi * 500 * t) +      # En dessous de la bande
                  np.sin(2 * np.pi * CENTER_FREQ * t) + # Dans la bande
                  np.sin(2 * np.pi * 2000 * t))        # Au-dessus de la bande
    signals['multi_freq'] = multi_freq
    
    return signals

def design_scipy_bandpass_filter(order, center_freq, bandwidth, sample_rate):
    """
    Conçoit un filtre passe-bande Butterworth avec scipy.signal
    
    Args:
        order: Ordre du filtre (doit être pair)
        center_freq: Fréquence centrale en Hz
        bandwidth: Largeur de bande en Hz
        sample_rate: Fréquence d'échantillonnage en Hz
    
    Returns:
        sos: Sections du filtre au format Second-Order Sections
    """
    # Calcul des fréquences de coupure
    low_freq = center_freq - bandwidth / 2
    high_freq = center_freq + bandwidth / 2
    
    # Normalisation par la fréquence de Nyquist
    nyquist = sample_rate / 2
    low_norm = low_freq / nyquist
    high_norm = high_freq / nyquist
    
    # Vérification des limites
    if low_norm <= 0 or high_norm >= 1:
        raise ValueError(f"Frequencies out of range: {low_freq}-{high_freq} Hz for Fs={sample_rate} Hz")
    
    # Conception du filtre passe-bande Butterworth
    # scipy.signal.butter produit automatiquement des sections SOS optimisées
    sos = signal.butter(order, [low_norm, high_norm], 
                       btype='band', analog=False, output='sos')
    
    return sos

def compute_frequency_response(sos, sample_rate, n_points=1024):
    """Calcule la réponse en fréquence du filtre"""
    w, h = signal.sosfreqz(sos, worN=n_points, fs=sample_rate)
    return w.tolist(), np.abs(h).tolist(), np.angle(h).tolist()

def process_signals_with_filter(signals, sos):
    """Traite tous les signaux de test avec le filtre scipy"""
    processed = {}
    
    for signal_name, signal_data in signals.items():
        # Application du filtre avec sosfilt (recommandé pour la stabilité numérique)
        filtered = signal.sosfilt(sos, signal_data)
        processed[signal_name] = filtered.tolist()
    
    return processed

def generate_reference_data():
    """Génère toutes les données de référence pour tous les ordres"""
    print("Génération des signaux de test...")
    test_signals = create_test_signals()
    
    reference_data = {
        'config': {
            'sample_rate': SAMPLE_RATE,
            'center_freq': CENTER_FREQ,
            'bandwidth': BANDWIDTH,
            'signal_length': SIGNAL_LENGTH
        },
        'test_signals': {name: data.tolist() for name, data in test_signals.items()},
        'filters': {}
    }
    
    for order in ORDERS:
        print(f"Traitement de l'ordre {order}...")
        
        # Conception du filtre scipy
        sos = design_scipy_bandpass_filter(order, CENTER_FREQ, BANDWIDTH, SAMPLE_RATE)
        
        # Affichage des coefficients pour debug
        print(f"  Coefficients SOS pour l'ordre {order}:")
        for i, section in enumerate(sos):
            b = section[:3]  # b0, b1, b2
            a = section[3:]  # a0, a1, a2 (a0 normalisé à 1.0)
            print(f"    Section {i}: b=[{b[0]:.6f}, {b[1]:.6f}, {b[2]:.6f}], a=[{a[0]:.6f}, {a[1]:.6f}, {a[2]:.6f}]")
        
        # Calcul de la réponse en fréquence
        freqs, magnitude, phase = compute_frequency_response(sos, SAMPLE_RATE)
        
        # Traitement des signaux de test
        processed_signals = process_signals_with_filter(test_signals, sos)
        
        # Stockage des résultats pour cet ordre
        reference_data['filters'][f'order_{order}'] = {
            'sos_coefficients': sos.tolist(),
            'frequency_response': {
                'frequencies': freqs,
                'magnitude': magnitude,
                'phase': phase
            },
            'processed_signals': processed_signals
        }
    
    return reference_data

def save_reference_data(data, output_file):
    """Sauvegarde les données de référence en JSON"""
    output_path = Path(output_file)
    output_path.parent.mkdir(parents=True, exist_ok=True)
    
    with open(output_path, 'w') as f:
        json.dump(data, f, indent=2)
    
    print(f"Données de référence sauvegardées dans: {output_path}")

def plot_frequency_responses(reference_data):
    """Génère des graphiques de comparaison des réponses en fréquence"""
    plt.figure(figsize=(12, 8))
    
    for order in ORDERS:
        filter_data = reference_data['filters'][f'order_{order}']
        freqs = np.array(filter_data['frequency_response']['frequencies'])
        magnitude = np.array(filter_data['frequency_response']['magnitude'])
        
        # Conversion en dB
        magnitude_db = 20 * np.log10(np.maximum(magnitude, 1e-10))
        
        plt.semilogx(freqs, magnitude_db, label=f'Ordre {order}')
    
    plt.axvline(CENTER_FREQ - BANDWIDTH/2, color='red', linestyle='--', alpha=0.7, label='Fc - BW/2')
    plt.axvline(CENTER_FREQ, color='black', linestyle='-', alpha=0.7, label='Fc')
    plt.axvline(CENTER_FREQ + BANDWIDTH/2, color='red', linestyle='--', alpha=0.7, label='Fc + BW/2')
    
    plt.xlabel('Fréquence (Hz)')
    plt.ylabel('Magnitude (dB)')
    plt.title(f'Réponse en fréquence - Filtre passe-bande Butterworth\nFc={CENTER_FREQ}Hz, BW={BANDWIDTH}Hz')
    plt.grid(True, alpha=0.3)
    plt.legend()
    plt.ylim(-80, 5)
    
    # Sauvegarder le graphique
    plot_path = Path(__file__).parent / 'bandpass_frequency_response.png'
    plt.savefig(plot_path, dpi=300, bbox_inches='tight')
    print(f"Graphique sauvegardé dans: {plot_path}")
    
    plt.show()

def validate_filter_performance(reference_data):
    """Valide que les filtres ont les caractéristiques attendues"""
    print("\nValidation des performances des filtres:")
    print("="*50)
    
    for order in ORDERS:
        filter_data = reference_data['filters'][f'order_{order}']
        freqs = np.array(filter_data['frequency_response']['frequencies'])
        magnitude = np.array(filter_data['frequency_response']['magnitude'])
        
        # Trouver l'indice de la fréquence centrale
        center_idx = np.argmin(np.abs(freqs - CENTER_FREQ))
        center_magnitude = magnitude[center_idx]
        
        # Trouver les indices des fréquences de coupure
        low_cutoff_idx = np.argmin(np.abs(freqs - (CENTER_FREQ - BANDWIDTH/2)))
        high_cutoff_idx = np.argmin(np.abs(freqs - (CENTER_FREQ + BANDWIDTH/2)))
        
        low_cutoff_magnitude = magnitude[low_cutoff_idx]
        high_cutoff_magnitude = magnitude[high_cutoff_idx]
        
        # Calculer l'atténuation aux fréquences de coupure (devrait être ~-3dB)
        low_cutoff_db = 20 * np.log10(low_cutoff_magnitude / center_magnitude)
        high_cutoff_db = 20 * np.log10(high_cutoff_magnitude / center_magnitude)
        
        print(f"Ordre {order:2d}:")
        print(f"  Magnitude à Fc: {center_magnitude:.4f}")
        print(f"  Atténuation à Fc-BW/2: {low_cutoff_db:.2f} dB")
        print(f"  Atténuation à Fc+BW/2: {high_cutoff_db:.2f} dB")
        
        # Calculer la pente de coupure (approximation)
        # Chercher la fréquence où l'atténuation est de -20dB
        target_magnitude = center_magnitude * 10**(-20/20)
        
        # Côté bas
        low_side_idx = np.where((freqs < CENTER_FREQ) & (magnitude < target_magnitude))
        if len(low_side_idx[0]) > 0:
            low_20db_freq = freqs[low_side_idx[0][-1]]
            low_slope_freq_range = (CENTER_FREQ - BANDWIDTH/2) - low_20db_freq
            if low_slope_freq_range > 0:
                # Approximation de la pente: (20-3)/log10(freq_ratio)
                freq_ratio = (CENTER_FREQ - BANDWIDTH/2) / low_20db_freq
                low_slope = 17 / np.log10(freq_ratio)  # dB/decade
                print(f"  Pente côté bas: ~{low_slope:.1f} dB/decade")
        
        print()

if __name__ == "__main__":
    print("Générateur de valeurs de référence pour BandpassFilter")
    print("="*60)
    
    # Génération des données de référence
    reference_data = generate_reference_data()
    
    # Validation des performances
    validate_filter_performance(reference_data)
    
    # Sauvegarde des données
    output_file = Path(__file__).parent / '../tests/data/bandpass_reference_values.json'
    save_reference_data(reference_data, output_file)
    
    # Génération des graphiques
    plot_frequency_responses(reference_data)
    
    print("\nGénération terminée avec succès!")
    print("Les données de référence sont prêtes pour les tests Rust.")
