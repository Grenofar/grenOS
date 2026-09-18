"""Ce que pèsera l'image, avant de la construire.

Lit les index de trixie, suit Depends et Recommends comme apt le fait par
défaut, et donne la taille installée du tout, puis les vingt plus gros
paquets. Une estimation suffit : elle dit quoi couper, et l'ISO compressée
fait grosso modo 40 à 45 % de la taille installée.

    python3 taille.py [--sans paquet1,paquet2] [--sans-recommande]
"""
import gzip
import re
import sys
import urllib.request

AREAS = ['main', 'contrib', 'non-free', 'non-free-firmware']
BASE = 'http://deb.debian.org/debian/dists/trixie'
LIST = r'C:\Users\userAsus\Documents\GitHub\grenOS\linux\config\package-lists\grenos.list.chroot'


def index():
    packages, provides = {}, {}
    for area in AREAS:
        raw = urllib.request.urlopen(f'{BASE}/{area}/binary-amd64/Packages.gz', timeout=180).read()
        text = gzip.decompress(raw).decode('utf-8', 'replace')
        for block in text.split('\n\n'):
            if not block.strip():
                continue
            fields = {}
            for line in block.split('\n'):
                m = re.match(r'^([A-Za-z-]+): (.*)$', line)
                if m:
                    fields[m.group(1)] = m.group(2)
            name = fields.get('Package')
            if not name or name in packages:
                continue
            packages[name] = {
                'size': int(fields.get('Installed-Size', 0)),
                'depends': fields.get('Depends', ''),
                'recommends': fields.get('Recommends', ''),
            }
            for virtual in re.split(r',\s*', fields.get('Provides', '')):
                virtual = virtual.split()[0] if virtual.strip() else ''
                if virtual:
                    provides.setdefault(virtual, name)
    return packages, provides


def needs(field):
    """Les noms d'une ligne Depends : la première solution de chaque alternative."""
    out = []
    for clause in re.split(r',\s*', field or ''):
        clause = clause.strip()
        if not clause:
            continue
        first = re.split(r'\s*\|\s*', clause)[0]
        name = first.split()[0]
        if not name.startswith('$'):
            out.append(name)
    return out


def closure(packages, provides, wanted, with_recommends=True):
    seen, queue, missing = set(), list(wanted), set()
    while queue:
        name = queue.pop()
        if name in seen:
            continue
        real = name if name in packages else provides.get(name)
        if not real:
            missing.add(name)
            continue
        seen.add(real)
        queue.extend(needs(packages[real]['depends']))
        if with_recommends:
            queue.extend(needs(packages[real]['recommends']))
    return seen, missing


if __name__ == '__main__':
    drop = set()
    for arg in sys.argv[1:]:
        if arg.startswith('--sans='):
            drop = set(arg.split('=', 1)[1].split(','))
    with_rec = '--sans-recommande' not in sys.argv

    packages, provides = index()
    wanted = [l.strip() for l in open(LIST, encoding='utf-8') if l.strip() and not l.strip().startswith('#')]
    wanted = [w for w in wanted if w not in drop]
    # Ce que live-build installe toujours en plus de notre liste.
    wanted += ['linux-image-amd64', 'live-boot', 'live-config', 'live-config-systemd', 'systemd-sysv', 'grub-pc']

    seen, missing = closure(packages, provides, wanted, with_rec)
    total = sum(packages[n]['size'] for n in seen)
    print(f"{len(seen)} paquets, {total / 1024:.0f} Mio installes "
          f"(recommandes {'inclus' if with_rec else 'exclus'})")
    print(f"ISO estimee : {total / 1024 * 0.42:.0f} a {total / 1024 * 0.47:.0f} Mio")
    if missing:
        print('introuvables :', sorted(missing)[:10])
    biggest = sorted(seen, key=lambda n: -packages[n]['size'])[:20]
    print('les plus gros :')
    for name in biggest:
        print(f"  {packages[name]['size'] / 1024:7.1f} Mio  {name}")
