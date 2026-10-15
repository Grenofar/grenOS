# Signaler une faille de sécurité dans grenOS Linux

grenOS Linux est une distribution basée sur Debian trixie. Si vous découvrez une faille de sécurité dans l'un de ses composants, merci de nous le signaler afin que nous puissions protéger nos utilisateurs.

## Comment signaler une faille

**Si la faille est exploitable, ne l'ouvrez pas en issue publique.** Une issue
est lisible par tout le monde à la seconde où elle est écrite, y compris par
quelqu'un qui s'en servirait avant que le correctif n'existe. Passez par l'avis
de sécurité privé de GitHub, qui ne demande aucune adresse courriel et reste
invisible jusqu'à publication :

https://github.com/Grenofar/grenOS/security/advisories/new

Pour tout le reste — un doute, une question, un durcissement à proposer — une
issue ordinaire convient très bien :
https://github.com/Grenofar/grenOS/issues

Dans votre message, veuillez inclure :
- Une description claire et détaillée du problème rencontré.
- Les étapes permettant de reproduire la faille, si possible.
- La version de grenOS concernée.
- Les composants affectés (voir ci-dessous).

## Composants concernés

Cette procédure s'applique aux éléments suivants de grenOS Linux :
- L'image système située dans le répertoire `linux/`.
- Les programmes du bureau grenOS (grenos-shell, grenos-fichiers, grenos-parametres, etc.).
- Le dépôt APT personnalisé de grenOS.

## Ce qui est exclu

Le noyau Rust situé sous `kernel/` est abandonné depuis le 17 septembre 2026 et ne reçoit plus de mises à jour de sécurité. Toute faille découverte dans cette partie ne sera pas traitée.

## Important

- grenOS ne propose pas de programme de récompense financière (bounty) pour les signalements de sécurité.
- Nous nous efforçons de répondre rapidement, mais aucun délai de réponse n'est garanti.
- Aucune adresse courriel ni formulaire web dédié : tout passe par GitHub.
- Un avis de sécurité privé reste **invisible** tant qu'il n'est pas publié ; une issue ordinaire, elle, est publique immédiatement.

Nous vous remercions pour votre contribution à la sécurité de grenOS.
