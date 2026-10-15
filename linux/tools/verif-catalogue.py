#!/usr/bin/env python3
\"\"\"
Why this guard exists:
On 2026-09-24 an agent replaced the whole catalogue and resurrected an application
Grenofar had asked to remove, and a green CI said nothing.
This script validates linux/data/catalogue.json to prevent such silent regressions.
\"\"\"

import json
import sys
from pathlib import Path

def main() -> int:
    catalogue_path = Path("linux/data/catalogue.json")
    try:
        with catalogue_path.open("r", encoding="utf-8") as f:
            data = json.load(f)
    except json.JSONDecodeError as e:
        print(f"Invalid JSON: {e}", file=sys.stderr)
        return 1
    except Exception as e:
        print(f"Error reading file: {e}", file=sys.stderr)
        return 1

    if not isinstance(data, dict) or "applications" not in data:
        print("Missing 'applications' key or not a dict", file=sys.stderr)
        return 1

    applications = data["applications"]
    if not isinstance(applications, list):
        print("'applications' is not a list", file=sys.stderr)
        return 1

    seen_slugs = set()
    seen_idents = set()
    faults = []

    # Existing sources from the catalogue (for source validation)
    existing_sources = {app.get("source") for app in applications if isinstance(app, dict) and "source" in app}

    for idx, app in enumerate(applications):
        if not isinstance(app, dict):
            faults.append(f"Entry {idx} is not an object")
            continue

        slug = app.get("slug")
        identifiant = app.get("identifiant")

        if slug is None:
            faults.append(f"Entry {idx} missing 'slug'")
        elif not isinstance(slug, str):
            faults.append(f"Entry {idx} 'slug' is not a string")
        elif slug in seen_slugs:
            faults.append(f"Duplicate slug: {slug}")
        else:
            seen_slugs.add(slug)

        if identifiant is None:
            faults.append(f"Entry {idx} missing 'identifiant'")
        elif not isinstance(identifiant, str):
            faults.append(f"Entry {idx} 'identifiant' is not a string")
        elif identifiant in seen_idents:
            faults.append(f"Duplicate identifiant: {identifiant}")
        else:
            seen_idents.add(identifiant)

        # Expected fields
        expected_fields = {"slug", "identifiant", "nom", "description", "categorie", "logo", "source"}
        actual_fields = set(app.keys())
        missing = expected_fields - actual_fields
        extra = actual_fields - expected_fields

        for field in missing:
            faults.append(f"Entry {slug if slug else f'index {idx}'} missing field: {field}")
        for field in extra:
            faults.append(f"Entry {slug if slug else f'index {idx}'} extra field: {field}")

        # Logo URL validation
        logo = app.get("logo")
        if logo is not None and identifiant is not None:
            if not isinstance(logo, str):
                faults.append(f"Entry {slug if slug else f'index {idx}'} 'logo' is not a string")
            elif not logo.endswith(f"{identifiant}.png"):
                faults.append(f"Entry {slug if slug else f'index {idx}'} logo URL does not end with identifiant + '.png': {logo}")

        # Source validation
        source = app.get("source")
        if source is not None:
            if not isinstance(source, str):
                faults.append(f"Entry {slug if slug else f'index {idx}'} 'source' is not a string")
            elif source not in existing_sources:
                faults.append(f"Entry {slug if slug else f'index {idx}'} source not in existing set: {source}")

    if faults:
        for fault in faults:
            print(fault, file=sys.stderr)
        return 1
    else:
        print(len(applications))
        return 0

if __name__ == "__main__":
    sys.exit(main())
