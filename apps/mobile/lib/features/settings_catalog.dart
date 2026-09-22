import 'package:lucide_icons_flutter/lucide_icons.dart';
import 'package:flutter/material.dart';

import '../core/api.dart';

class SettingField {
  final String key, label;
  final bool multiline, required;
  final List<String>? options;
  const SettingField(
    this.key,
    this.label, {
    this.multiline = false,
    this.required = false,
    this.options,
  });
}

class SettingsSection {
  final String id, title, group, description;
  final IconData icon;
  final String? collection;
  final List<SettingField> fields;
  final Json defaults;
  const SettingsSection(
    this.id,
    this.title,
    this.group,
    this.icon,
    this.description, {
    this.collection,
    this.fields = const [],
    this.defaults = const {},
  });
}

// IDs follow apps/desktop/src/features/settings/types.ts (general is a models alias).
const settingsSections = [
  SettingsSection(
    'billing', 'Offre & consommation', 'Compte & équipe', LucideIcons.creditCard,
    'Abonnement, crédit de calcul et plafonds de dépense de cet espace.',
  ),
  SettingsSection(
    'profile',
    'Profil',
    'Compte & équipe',
    LucideIcons.user,
    'Votre identité et la sécurité de votre compte.',
  ),
  SettingsSection(
    'organization',
    'Organisation',
    'Compte & équipe',
    LucideIcons.building2,
    'Votre espace partagé, ses membres et leurs rôles.',
  ),
  SettingsSection(
    'models',
    'Modèle IA',
    "Configuration de l’IA",
    LucideIcons.cpu,
    'Choisissez les modèles utilisés par votre espace.',
  ),
  SettingsSection(
    'system-prompt',
    'Instructions ARO',
    "Configuration de l’IA",
    LucideIcons.fileText,
    'Définissez les instructions de chaque mode.',
    collection: 'system-prompts',
    fields: [
      SettingField(
        'mode',
        'Mode',
        options: ['chat', 'think', 'code', 'summarize', 'quiet'],
      ),
      SettingField('identity', 'Identité', multiline: true),
      SettingField('rules', 'Règles', multiline: true),
      SettingField('formatting', 'Format des réponses', multiline: true),
    ],
    defaults: {'mode': 'chat', 'enabled': true},
  ),
  SettingsSection(
    'instructions',
    'Profils ARO',
    "Configuration de l’IA",
    LucideIcons.bot,
    'Des profils adaptés à vos façons de travailler.',
    collection: 'personalities',
    fields: [
      SettingField('name', 'Nom', required: true),
      SettingField('description', 'Description'),
      SettingField('prompt', 'Instructions', multiline: true, required: true),
    ],
    defaults: {'temperature': 0.7, 'isDefault': false},
  ),
  SettingsSection(
    'memory',
    'Mémoire',
    "Configuration de l’IA",
    LucideIcons.brain,
    'Consultez et organisez les informations mémorisées.',
    collection: 'memories',
    fields: [
      SettingField('content', 'Souvenir', multiline: true, required: true),
      SettingField(
        'category',
        'Catégorie',
        options: ['personal', 'technical', 'preference', 'system'],
      ),
    ],
    defaults: {'category': 'personal', 'pinned': false, 'source': 'user'},
  ),
  SettingsSection(
    'agents',
    'Agents',
    "Configuration de l’IA",
    LucideIcons.bot,
    'Configurez vos assistants spécialisés.',
    collection: 'agent-definitions',
    fields: [
      SettingField('name', 'Nom', required: true),
      SettingField('description', 'Description'),
      SettingField(
        'systemPrompt',
        'Instructions',
        multiline: true,
        required: true,
      ),
    ],
    defaults: {'enabled': true},
  ),
  SettingsSection(
    'permissions',
    "Autorisations de l’IA",
    "Configuration de l’IA",
    LucideIcons.shield,
    'Inspectez les autorisations des agents et du téléphone.',
  ),
  SettingsSection(
    'search',
    'Recherche web',
    "Configuration de l’IA",
    LucideIcons.search,
    'Le moteur utilisé pour enrichir les réponses.',
  ),
  SettingsSection(
    'skills',
    'Skills',
    'Extensions & APIs',
    LucideIcons.terminal,
    'Créez et gérez les méthodes de travail ARO.',
    collection: 'skills',
    fields: [
      SettingField('name', 'Nom', required: true),
      SettingField('description', 'Description'),
      SettingField('content', 'Instructions', multiline: true, required: true),
    ],
    defaults: {'kind': 'prompt', 'enabled': true, 'triggers': []},
  ),
  SettingsSection(
    'plugins',
    'Plugins',
    'Extensions & APIs',
    LucideIcons.puzzle,
    'Les extensions installées sur votre serveur.',
  ),
  SettingsSection(
    'mcp',
    'MCP Servers',
    'Extensions & APIs',
    LucideIcons.network,
    'Connectez des outils à votre environnement d’exécution.',
    collection: 'mcp-servers',
    fields: [
      SettingField('name', 'Nom', required: true),
      SettingField('transport', 'Transport', options: ['sse', 'stdio']),
      SettingField('url', 'URL du serveur'),
      SettingField('command', 'Commande sur l’exécuteur'),
    ],
    defaults: {
      'transport': 'sse',
      'enabled': false,
      'args': [],
      'status': 'disconnected',
      'tools': [],
      'resources': [],
    },
  ),
  SettingsSection(
    'hooks',
    'Hooks & Webhooks',
    'Extensions & APIs',
    LucideIcons.zap,
    'Les notifications webhook de votre espace.',
    collection: 'hooks',
    fields: [
      SettingField('name', 'Nom', required: true),
      SettingField('url', 'URL HTTPS', required: true),
      SettingField('events', 'Événements (séparés par des virgules)'),
    ],
    defaults: {'enabled': false, 'status': 'idle'},
  ),
  SettingsSection(
    'scheduler',
    'Planificateur',
    'Extensions & APIs',
    LucideIcons.clock,
    'Gérez les tâches planifiées sur le serveur.',
    collection: 'scheduled-tasks',
    fields: [
      SettingField('name', 'Nom', required: true),
      SettingField('prompt', 'Demande', multiline: true, required: true),
      SettingField('cronExpression', 'Expression cron', required: true),
    ],
    defaults: {'scheduleType': 'cron', 'enabled': false, 'status': 'idle'},
  ),
  SettingsSection(
    'voice',
    'Voix & Audio',
    'Préférences & audio',
    LucideIcons.volume2,
    'Dictée et lecture vocale sur votre téléphone.',
  ),
  SettingsSection(
    'preferences',
    'Apparence',
    'Préférences & audio',
    LucideIcons.slidersHorizontal,
    'Retrouvez le style ARO, à votre façon.',
  ),
  SettingsSection(
    'notifications',
    'Notifications',
    'Préférences & audio',
    LucideIcons.bell,
    'Alertes in-app et e-mails envoyés par votre serveur.',
  ),
  SettingsSection(
    'shortcuts',
    'Raccourcis',
    'Préférences & audio',
    LucideIcons.keyboard,
    'Navigation rapide avec un clavier connecté.',
  ),
  SettingsSection(
    'browser',
    'Navigateur',
    "Configuration de l’IA",
    LucideIcons.globe,
    'Historique local et ouverture des liens sur ce téléphone.',
  ),
  SettingsSection(
    'computer',
    'Appareil',
    "Configuration de l’IA",
    LucideIcons.monitorSmartphone,
    'Informations sur cet appareil. Le contrôle du PC reste sur desktop.',
  ),
  SettingsSection(
    'paths',
    'Chemins',
    'Système',
    LucideIcons.folderOpen,
    'Les chemins appartiennent à leur appareil d’exécution.',
  ),
  SettingsSection(
    'monitoring',
    'Monitoring',
    'Système',
    LucideIcons.chartLine,
    'Activité et consommation de votre espace.',
  ),
  SettingsSection(
    'system',
    'Maintenance',
    'Système',
    LucideIcons.settings,
    'Connexion, synchronisation et données locales.',
  ),
];
