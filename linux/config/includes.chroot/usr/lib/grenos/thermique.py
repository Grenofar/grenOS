#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Module de lecture de la température et de la vitesse des ventilateurs.
Parcourt /sys/class/hwmon et /sys/class/thermal pour récupérer les données.
"""

import os
import glob

def _read_file(path):
    """Lit un fichier et retourne son contenu stripped, ou None en cas d'erreur."""
    try:
        with open(path, 'r') as f:
            return f.read().strip()
    except (OSError, IOError):
        return None

def _parse_temp(temp_str):
    """Convertit une température en millidegrés Celsius en degrés Celsius."""
    try:
        return int(temp_str) / 1000.0
    except (ValueError, TypeError):
        return None

def _parse_fan_speed(speed_str):
    """Convertit une vitesse de ventilateur en RPM."""
    try:
        return int(speed_str)
    except (ValueError, TypeError):
        return None

def get_thermal_info(hwmon_path='/sys/class/hwmon', thermal_path='/sys/class/thermal'):
    """
    Lit les capteurs de température et de vitesse des ventilateurs.
    
    Args:
        hwmon_path (str): Chemin vers le répertoire hwmon (défaut: /sys/class/hwmon)
        thermal_path (str): Chemin vers le répertoire thermal (défaut: /sys/class/thermal)
    
    Returns:
        tuple: (liste de tuples (nom, valeur), verdict)
               - liste: tuples (nom, valeur) où valeur est en °C pour temp, en RPM pour fan
               - verdict: 'OK' si au moins un capteur lu, 'NO_SENSORS' sinon
    """
    results = []
    
    # Lecture des capteurs hwmon (température et ventilateurs)
    for hwmon_dir in glob.glob(os.path.join(hwmon_path, 'hwmon*')):
        # Lecture de la température
        temp_files = glob.glob(os.path.join(hwmon_dir, 'temp*_input'))
        for temp_file in temp_files:
            # Extraction du nom du capteur
            label_file = temp_file.replace('_input', '_label')
            label = _read_file(label_file)
            if label is None:
                # Utiliser le nom du fichier si pas de label
                label = os.path.basename(temp_file).replace('_input', '')
            
            temp_str = _read_file(temp_file)
            if temp_str is not None:
                temp_celsius = _parse_temp(temp_str)
                if temp_celsius is not None:
                    results.append((label, temp_celsius))
        
        # Lecture de la vitesse des ventilateurs
        fan_files = glob.glob(os.path.join(hwmon_dir, 'fan*_input'))
        for fan_file in fan_files:
            # Extraction du nom du ventilateur
            label_file = fan_file.replace('_input', '_label')
            label = _read_file(label_file)
            if label is None:
                label = os.path.basename(fan_file).replace('_input', '')
            
            speed_str = _read_file(fan_file)
            if speed_str is not None:
                speed_rpm = _parse_fan_speed(speed_str)
                if speed_rpm is not None:
                    results.append((label, speed_rpm))
    
    # Lecture des capteurs thermal (température uniquement)
    for thermal_dir in glob.glob(os.path.join(thermal_path, 'thermal_zone*')):
        temp_file = os.path.join(thermal_dir, 'temp')
        type_file = os.path.join(thermal_dir, 'type')
        
        label = _read_file(type_file)
        if label is None:
            label = os.path.basename(thermal_dir)
        
        temp_str = _read_file(temp_file)
        if temp_str is not None:
            temp_celsius = _parse_temp(temp_str)
            if temp_celsius is not None:
                results.append((label, temp_celsius))
    
    # Détermination du verdict
    verdict = 'OK' if results else 'NO_SENSORS'
    
    return results, verdict
