# GUIDE D'UTILISATION : Notaire DZ

Guide pratique pour le personnel d'une étude notariale. Chaque écran s'ouvre depuis le menu de
droite en arabe, ou de gauche en français. Aucune connexion Internet n'est nécessaire.

Le PDF illustré destiné au client se trouve dans `docs/manual/NotaireDZ-Manuel.pdf`.

---

## 1. Première ouverture

1. Lancer l'application depuis le raccourci du Bureau.
2. L'écran de connexion affiche déjà l'identifiant `admin` et le mot de passe `admin`.
3. Cliquer sur **دخول** pour entrer dans l'étude.

L'étude est créée automatiquement au premier lancement, avec une clé d'appareil enregistrée à côté
de la base de données. Changer ensuite le mot de passe dans **Paramètres**, section
**بيانات الحساب**.

Le raccourci **Notaire DZ (demo)** ouvre une étude d'essai séparée, sans connexion, avec des
données d'exemple. Elle porte un badge permanent et ne touche jamais aux vraies données.

## 2. Changer de langue

En haut de la fenêtre, deux boutons : **العربية** et **Français**. Le changement est immédiat et la
mise en page passe de droite à gauche à gauche à droite, sans redémarrage.

## 3. Écran d'accueil

Quatre cartes résument la journée :

- **مواعيد اليوم** : rendez-vous de signature et d'accueil.
- **ملفات تنتظر إجراء** : dossiers à revoir ou documents manquants.
- **وثائق تنتهي صلاحيتها** : procurations et certificats qui expirent sous trente jours.
- **أرصدة غير مسددة** : montants encore dus par les clients.

Deux boutons : **+ ملف جديد** ouvre un dossier, **بيانات الحساب** ouvre la fiche du compte.

Les chiffres restent à zéro tant que les dossiers correspondants ne sont pas saisis. Les sections
Contrats, Comptabilité, Conformité et Agenda sont annoncées comme prévues pour une version
ultérieure.

## 4. Clients

Menu **الزبائن**.

- **جديد** crée une fiche : nom en arabe et en français, qualité (particulier, société), téléphone,
  adresse, numéro d'identité ou registre de commerce.
- La recherche accepte les deux écritures et signale un doublon probable avant l'enregistrement.
- Ouvrir une fiche permet de la corriger ; chaque modification est inscrite au journal d'audit.

## 5. Dossiers

Menu **الملفات**.

- **جديد** crée un dossier : client, type d'acte, objet, date d'ouverture, responsable.
- Une liste de contrôle suit les pièces attendues.
- Les documents déposés sont chiffrés dans le coffre-fort et leur empreinte est enregistrée.
- Les permissions par dossier décident qui peut consulter, modifier ou exporter.

## 6. Journal d'audit

Menu **الإعدادات**, section Audit.

- Affiche le nombre d'entrées et l'empreinte de tête de la chaîne.
- Le bouton de vérification recalcule la chaîne entière : toute suppression ou modification d'une
  entrée apparaît immédiatement.
- Chaque connexion, écriture, export et lecture sensible y figure.

## 7. Sauvegarde et restauration

Menu **الإعدادات**, section Sauvegarde.

- **Sauvegarder maintenant** produit une archive chiffrée dans `backups\`.
- L'archive contient la base chiffrée et les documents du coffre-fort.
- La restauration vérifie l'intégrité avant de remplacer quoi que ce soit et conserve une
  sauvegarde de l'état courant.
- Copier régulièrement les archives sur une clé USB ou un disque externe.

## 8. Compte et mot de passe

Menu **الإعدادات**, section **بيانات الحساب**.

- Changer l'identifiant de connexion.
- Changer le mot de passe : le mot de passe actuel est demandé, le nouveau doit faire au moins
  cinq caractères.
- En cas d'oubli, l'écran de connexion propose **code de récupération** : le code remis à la
  création de l'étude permet de définir un nouveau mot de passe sans perdre les données.

## 9. Code de récupération

Il est affiché une seule fois, à la création de l'étude, en six groupes de caractères. Il doit être
noté sur papier et conservé hors du poste. Il n'est jamais affiché à nouveau : seul son
remplacement est possible depuis les paramètres de sécurité.

## 10. Limites de cette version

- Rédaction des actes, tarifs et émoluments, comptabilité, calendrier de conformité et agenda ne
  sont pas encore disponibles ; leurs écrans l'indiquent clairement.
- La reconnaissance de texte (OCR) demande l'installation de Tesseract sur le poste.
- Le logiciel fonctionne sur un seul ordinateur. Pour changer de machine, installer puis restaurer
  la dernière sauvegarde.
- Aucune règle de droit n'est inventée : les taux et délais sont des données versionnées, marquées
  à vérifier jusqu'à validation par un notaire ou un juriste.
