export interface PluginConfigField { name: string; label: string; placeholder: string; type: "text" | "password"; }
export interface PluginSubService { id: string; label: string; enabled: boolean; icon: string; }
export interface UserPlugin {
  id: string; cloudId?: string; name: string; description: string; icon: string; category: string;
  status: "connected" | "disconnected" | "setup"; authType: "none" | "api_key" | "oauth";
  fields: Record<string, string>; configFields: PluginConfigField[]; enabled: boolean; createdAt: string;
  profileName?: string; profileAvatar?: string; profileDetails?: string; subServices?: PluginSubService[];
}
export function createInitialPlugins(): UserPlugin[] {
  return [
    {
      id: "plg-github",
      name: "GitHub",
      description: "Connectez votre compte GitHub pour donner à l'assistant accès à vos dépôts, issues et pull requests.",
      icon: "🐈",
      category: "Développement",
      status: "setup",
      authType: "api_key",
      fields: { token: "" },
      configFields: [
        { name: "token", label: "Personal Access Token (PAT)", placeholder: "ghp_...", type: "password" }
      ],
      enabled: true,
      createdAt: "2026-06-29T08:00:00.000Z"
    },
    {
      id: "plg-google",
      name: "Google Workspace",
      description: "Connectez votre compte Google pour donner accès à vos documents Docs, Sheets, Drive, Keep, Calendar et Slides.",
      icon: "🤖",
      category: "Productivité",
      status: "setup",
      authType: "oauth",
      fields: { token: "" },
      configFields: [
        { name: "token", label: "Jeton d'accès Google API", placeholder: "ya29.a0a...", type: "password" }
      ],
      enabled: false,
      createdAt: "2026-06-29T08:00:00.000Z",
      subServices: [
        { id: "docs", label: "Google Docs", enabled: true, icon: "📄" },
        { id: "sheets", label: "Google Sheets", enabled: true, icon: "📊" },
        { id: "slides", label: "Google Slides", enabled: true, icon: "📉" },
        { id: "drive", label: "Google Drive", enabled: true, icon: "📁" },
        { id: "calendar", label: "Google Calendar", enabled: true, icon: "📅" },
        { id: "keep", label: "Google Keep", enabled: true, icon: "💡" }
      ]
    },
    {
      id: "plg-microsoft",
      name: "Microsoft 365",
      description: "Connectez votre compte Microsoft pour donner accès à vos e-mails Outlook, documents Word/Excel, OneDrive, OneNote et Teams.",
      icon: "💼",
      category: "Productivité",
      status: "setup",
      authType: "oauth",
      fields: { token: "" },
      configFields: [
        { name: "token", label: "Jeton d'accès Microsoft Graph API", placeholder: "eyJ0eXAi...", type: "password" }
      ],
      enabled: false,
      createdAt: "2026-06-29T08:00:00.000Z",
      subServices: [
        { id: "outlook", label: "Outlook Mail", enabled: true, icon: "✉️" },
        { id: "onedrive", label: "OneDrive", enabled: true, icon: "📁" },
        { id: "word", label: "Word / Excel", enabled: true, icon: "📄" },
        { id: "onenote", label: "OneNote", enabled: true, icon: "📓" },
        { id: "teams", label: "Teams", enabled: true, icon: "💬" }
      ]
    },
    {
      id: "plg-apple",
      name: "Apple Ecosystem",
      description: "Connectez vos applications natives Apple pour permettre à l'assistant d'interagir localement avec Notes, Calendrier, Rappels, Mail et Messages.",
      icon: "🍎",
      category: "Écosystème",
      status: "setup",
      authType: "none",
      fields: {},
      configFields: [],
      enabled: false,
      createdAt: "2026-06-29T08:00:00.000Z",
      subServices: [
        { id: "notes", label: "Notes", enabled: true, icon: "📝" },
        { id: "calendar", label: "Calendrier", enabled: true, icon: "📅" },
        { id: "reminders", label: "Rappels", enabled: true, icon: "🔔" },
        { id: "mail", label: "Mail", enabled: true, icon: "✉️" },
        { id: "messages", label: "Messages", enabled: true, icon: "💬" }
      ]
    },
    {
      id: "plg-meta",
      name: "Meta Suite",
      description: "Liez votre compte Meta pour interagir directement avec WhatsApp, Messenger, Instagram et Threads.",
      icon: "♾️",
      category: "Communication",
      status: "setup",
      authType: "oauth",
      fields: { token: "" },
      configFields: [
        { name: "token", label: "Jeton d'accès Meta Graph API", placeholder: "EAACw...", type: "password" }
      ],
      enabled: false,
      createdAt: "2026-06-29T08:00:00.000Z",
      subServices: [
        { id: "whatsapp", label: "WhatsApp", enabled: true, icon: "💬" },
        { id: "messenger", label: "Messenger", enabled: true, icon: "✉️" },
        { id: "instagram", label: "Instagram", enabled: true, icon: "📸" },
        { id: "threads", label: "Threads", enabled: true, icon: "🧵" }
      ]
    },
    {
      id: "plg-slack",
      name: "Slack",
      description: "Connectez votre espace de travail Slack pour lire, rechercher et envoyer des messages dans vos canaux de discussion.",
      icon: "💬",
      category: "Communication",
      status: "setup",
      authType: "api_key",
      fields: { token: "" },
      configFields: [
        { name: "token", label: "Slack Bot OAuth Token", placeholder: "xoxb-...", type: "password" }
      ],
      enabled: false,
      createdAt: "2026-06-29T08:00:00.000Z"
    },
    {
      id: "plg-spotify",
      name: "Spotify",
      description: "Connectez votre compte Spotify pour rechercher des musiques, gérer la lecture et parcourir vos playlists.",
      icon: "🎵",
      category: "Divertissement",
      status: "setup",
      authType: "oauth",
      fields: { token: "" },
      configFields: [
        { name: "token", label: "Spotify Access Token", placeholder: "BQD...", type: "password" }
      ],
      enabled: false,
      createdAt: "2026-06-29T08:00:00.000Z"
    },
    {
      id: "plg-chrome",
      name: "Chrome Extension",
      description: "Se connecte à l'extension ARO Chrome locale pour lire et modifier l'onglet actif.",
      icon: "🌐",
      category: "Navigateur",
      status: "connected",
      authType: "none",
      fields: { port: "8080" },
      configFields: [
        { name: "port", label: "Port d'écoute local", placeholder: "8080", type: "text" }
      ],
      enabled: true,
      createdAt: "2026-06-29T08:00:00.000Z"
    },
    {
      id: "plg-canva",
      name: "Canva",
      description: "Liez votre compte Canva pour créer des designs, éditer des modèles et exporter des visuels directement.",
      icon: "🎨",
      category: "Conception",
      status: "setup",
      authType: "oauth",
      fields: { token: "" },
      configFields: [
        { name: "token", label: "Jeton d'accès Canva API", placeholder: "eyJhbGci...", type: "password" }
      ],
      enabled: false,
      createdAt: "2026-06-29T08:00:00.000Z"
    },
    {
      id: "plg-figma",
      name: "Figma",
      description: "Connectez votre compte Figma pour inspecter vos fichiers de conception, extraire des composants et commenter vos maquettes.",
      icon: "📐",
      category: "Conception",
      status: "setup",
      authType: "api_key",
      fields: { token: "" },
      configFields: [
        { name: "token", label: "Personal Access Token Figma", placeholder: "figd_...", type: "password" }
      ],
      enabled: false,
      createdAt: "2026-06-29T08:00:00.000Z"
    },
    {
      id: "plg-jira",
      name: "Jira",
      description: "Synchronisez ARO avec votre instance Jira pour suivre vos tickets, mettre à jour les statuts et assigner des tâches.",
      icon: "🎫",
      category: "Productivité",
      status: "setup",
      authType: "api_key",
      fields: { token: "", url: "" },
      configFields: [
        { name: "token", label: "Jeton d'API Atlassian / Jira", placeholder: "ATATT3x...", type: "password" },
        { name: "url", label: "URL de votre instance (ex: company.atlassian.net)", placeholder: "company.atlassian.net", type: "text" }
      ],
      enabled: false,
      createdAt: "2026-06-29T08:00:00.000Z"
    },
    {
      id: "plg-vercel",
      name: "Vercel",
      description: "Liez votre compte Vercel pour suivre vos déploiements, inspecter les logs et gérer vos domaines.",
      icon: "▲",
      category: "Développement",
      status: "setup",
      authType: "api_key",
      fields: { token: "" },
      configFields: [
        { name: "token", label: "Jeton d'API Vercel", placeholder: "val_...", type: "password" }
      ],
      enabled: false,
      createdAt: "2026-06-29T08:00:00.000Z"
    },
    {
      id: "plg-linear",
      name: "Linear",
      description: "Liez votre compte Linear pour synchroniser vos cycles, créer des issues et gérer votre backlog de développement.",
      icon: "📈",
      category: "Développement",
      status: "setup",
      authType: "api_key",
      fields: { token: "" },
      configFields: [
        { name: "token", label: "Jeton d'API Personnel Linear", placeholder: "lin_api_...", type: "password" }
      ],
      enabled: false,
      createdAt: "2026-06-29T08:00:00.000Z"
    },
    {
      id: "plg-gitlab",
      name: "GitLab",
      description: "Connectez votre compte GitLab pour gérer vos projets, pipelines CI/CD, issues et merge requests.",
      icon: "🦊",
      category: "Développement",
      status: "setup",
      authType: "api_key",
      fields: { token: "", url: "https://gitlab.com" },
      configFields: [
        { name: "token", label: "Private Personal Access Token", placeholder: "glpat-...", type: "password" },
        { name: "url", label: "URL de l'instance GitLab (défaut: https://gitlab.com)", placeholder: "https://gitlab.com", type: "text" }
      ],
      enabled: false,
      createdAt: "2026-06-29T08:00:00.000Z"
    },
    {
      id: "plg-discord",
      name: "Discord",
      description: "Connectez votre bot Discord pour interagir avec vos serveurs, envoyer des messages et modérer des salons.",
      icon: "👾",
      category: "Communication",
      status: "setup",
      authType: "api_key",
      fields: { token: "" },
      configFields: [
        { name: "token", label: "Discord Bot Token", placeholder: "MTIz...", type: "password" }
      ],
      enabled: false,
      createdAt: "2026-06-29T08:00:00.000Z"
    },
    {
      id: "plg-trello",
      name: "Trello",
      description: "Synchronisez ARO avec vos tableaux Trello pour organiser vos listes de tâches et déplacer vos cartes.",
      icon: "📋",
      category: "Productivité",
      status: "setup",
      authType: "api_key",
      fields: { token: "", api_key: "" },
      configFields: [
        { name: "api_key", label: "Trello API Key", placeholder: "a1b2...", type: "text" },
        { name: "token", label: "Trello OAuth Token", placeholder: "clt-...", type: "password" }
      ],
      enabled: false,
      createdAt: "2026-06-29T08:00:00.000Z"
    },
    {
      id: "plg-zoom",
      name: "Zoom",
      description: "Liez votre compte Zoom pour planifier, démarrer ou rejoindre des réunions et récupérer vos rapports de réunion.",
      icon: "📹",
      category: "Communication",
      status: "setup",
      authType: "oauth",
      fields: { token: "" },
      configFields: [
        { name: "token", label: "Jeton d'accès Zoom JWT/OAuth", placeholder: "eyJhbGci...", type: "password" }
      ],
      enabled: false,
      createdAt: "2026-06-29T08:00:00.000Z"
    },
    {
      id: "plg-dropbox",
      name: "Dropbox",
      description: "Liez votre compte Dropbox pour synchroniser, lister et récupérer vos fichiers stockés sur le cloud.",
      icon: "📦",
      category: "Productivité",
      status: "setup",
      authType: "oauth",
      fields: { token: "" },
      configFields: [
        { name: "token", label: "Jeton d'accès Dropbox", placeholder: "sl.B1...", type: "password" }
      ],
      enabled: false,
      createdAt: "2026-06-29T08:00:00.000Z"
    },
    {
      id: "plg-aws",
      name: "AWS",
      description: "Connectez votre console AWS pour suivre vos instances EC2, fonctions Lambda, buckets S3 et budgets Cloud.",
      icon: "☁️",
      category: "Développement",
      status: "setup",
      authType: "api_key",
      fields: { access_key_id: "", secret_access_key: "", region: "us-east-1" },
      configFields: [
        { name: "access_key_id", label: "AWS Access Key ID", placeholder: "AKIA...", type: "text" },
        { name: "secret_access_key", label: "AWS Secret Access Key", placeholder: "secret...", type: "password" },
        { name: "region", label: "Région AWS par défaut", placeholder: "us-east-1", type: "text" }
      ],
      enabled: false,
      createdAt: "2026-06-29T08:00:00.000Z"
    },
    {
      id: "plg-gcp",
      name: "Google Cloud",
      description: "Connectez vos projets GCP pour lister vos VM Compute Engine, buckets Cloud Storage et coûts.",
      icon: "⚡",
      category: "Développement",
      status: "setup",
      authType: "api_key",
      fields: { project_id: "", client_email: "", private_key: "" },
      configFields: [
        { name: "project_id", label: "Project ID GCP", placeholder: "my-project-123", type: "text" },
        { name: "client_email", label: "Client Email du compte de service", placeholder: "service-account@...", type: "text" },
        { name: "private_key", label: "Clé privée du compte de service (JSON)", placeholder: "---BEGIN PRIVATE KEY---...", type: "password" }
      ],
      enabled: false,
      createdAt: "2026-06-29T08:00:00.000Z"
    },
    {
      id: "plg-kubernetes",
      name: "Kubernetes",
      description: "Liez votre cluster Kubernetes pour surveiller vos pods, déploiements, services et vérifier les logs.",
      icon: "☸️",
      category: "Développement",
      status: "setup",
      authType: "api_key",
      fields: { kubeconfig: "" },
      configFields: [
        { name: "kubeconfig", label: "Fichier Kubeconfig (YAML/JSON)", placeholder: "apiVersion: v1...", type: "password" }
      ],
      enabled: false,
      createdAt: "2026-06-29T08:00:00.000Z"
    },
    {
      id: "plg-huggingface",
      name: "Hugging Face",
      description: "Liez votre compte Hugging Face pour lister vos modèles, datasets et Spaces.",
      icon: "🤗",
      category: "IA",
      status: "setup",
      authType: "api_key",
      fields: { token: "" },
      configFields: [
        { name: "token", label: "Hugging Face User Access Token", placeholder: "hf_...", type: "password" }
      ],
      enabled: false,
      createdAt: "2026-06-29T08:00:00.000Z"
    },
    {
      id: "plg-clickup",
      name: "ClickUp",
      description: "Liez votre espace de travail ClickUp pour suivre vos tâches, modifier vos objectifs et collaborer.",
      icon: "🎯",
      category: "Productivité",
      status: "setup",
      authType: "api_key",
      fields: { token: "" },
      configFields: [
        { name: "token", label: "ClickUp Personal API Token", placeholder: "pk_...", type: "password" }
      ],
      enabled: false,
      createdAt: "2026-06-29T08:00:00.000Z"
    },
    {
      id: "plg-salesforce",
      name: "Salesforce",
      description: "Connectez votre instance Salesforce pour suivre vos opportunités de vente, leads et synchroniser vos comptes clients.",
      icon: "☁️",
      category: "Productivité",
      status: "setup",
      authType: "oauth",
      fields: { token: "", instance_url: "" },
      configFields: [
        { name: "instance_url", label: "URL de l'instance Salesforce (ex: mycompany.my.salesforce.com)", placeholder: "mycompany.my.salesforce.com", type: "text" },
        { name: "token", label: "Jeton d'accès Salesforce", placeholder: "00D...", type: "password" }
      ],
      enabled: false,
      createdAt: "2026-06-29T08:00:00.000Z"
    },
    {
      id: "plg-hubspot",
      name: "HubSpot",
      description: "Liez votre compte HubSpot pour gérer vos contacts CRM, suivre vos pipelines de vente et automatiser vos communications.",
      icon: "🧡",
      category: "Productivité",
      status: "setup",
      authType: "api_key",
      fields: { token: "" },
      configFields: [
        { name: "token", label: "Jeton d'accès HubSpot (PAT)", placeholder: "pat-na1-...", type: "password" }
      ],
      enabled: false,
      createdAt: "2026-06-29T08:00:00.000Z"
    },
    {
      id: "plg-asana",
      name: "Asana",
      description: "Synchronisez vos projets Asana pour lister les tâches assignées, échéances et collaborer.",
      icon: "🎯",
      category: "Productivité",
      status: "setup",
      authType: "api_key",
      fields: { token: "" },
      configFields: [
        { name: "token", label: "Personal Access Token Asana", placeholder: "0/123...", type: "password" }
      ],
      enabled: false,
      createdAt: "2026-06-29T08:00:00.000Z"
    },
    {
      id: "plg-twilio",
      name: "Twilio",
      description: "Liez votre compte Twilio pour envoyer des SMS, passer des appels et suivre vos communications.",
      icon: "📞",
      category: "Communication",
      status: "setup",
      authType: "api_key",
      fields: { account_sid: "", auth_token: "" },
      configFields: [
        { name: "account_sid", label: "Account SID Twilio", placeholder: "AC...", type: "text" },
        { name: "auth_token", label: "Auth Token Twilio", placeholder: "auth...", type: "password" }
      ],
      enabled: false,
      createdAt: "2026-06-29T08:00:00.000Z"
    },
    {
      id: "plg-shopify",
      name: "Shopify",
      description: "Connectez votre boutique Shopify pour suivre vos commandes en temps réel, produits et analyser votre chiffre d'affaires.",
      icon: "🛍️",
      category: "Commerce",
      status: "setup",
      authType: "api_key",
      fields: { token: "", shop_name: "" },
      configFields: [
        { name: "shop_name", label: "Nom de votre boutique (ex: mystore)", placeholder: "mystore", type: "text" },
        { name: "token", label: "Shopify Admin Access Token", placeholder: "shpat_...", type: "password" }
      ],
      enabled: false,
      createdAt: "2026-06-29T08:00:00.000Z"
    },
    {
      id: "plg-stripe",
      name: "Stripe",
      description: "Connectez votre compte Stripe pour suivre vos transactions, abonnements, remboursements et versements en direct.",
      icon: "💳",
      category: "Finance",
      status: "setup",
      authType: "api_key",
      fields: { token: "" },
      configFields: [
        { name: "token", label: "Stripe API Secret Key (sk_live_... ou sk_test_...)", placeholder: "sk_...", type: "password" }
      ],
      enabled: false,
      createdAt: "2026-06-29T08:00:00.000Z"
    },
    {
      id: "plg-mailchimp",
      name: "Mailchimp",
      description: "Liez votre compte Mailchimp pour suivre vos campagnes emailing, listes de diffusion et rapports d'audience.",
      icon: "✉️",
      category: "Marketing",
      status: "setup",
      authType: "api_key",
      fields: { token: "", datacenter: "us1" },
      configFields: [
        { name: "token", label: "Mailchimp API Key", placeholder: "a1b2c3...", type: "password" },
        { name: "datacenter", label: "Code du Datacenter (ex: us1, us20, etc.)", placeholder: "us1", type: "text" }
      ],
      enabled: false,
      createdAt: "2026-06-29T08:00:00.000Z"
    },
    {
      id: "plg-intercom",
      name: "Intercom",
      description: "Synchronisez ARO avec Intercom pour gérer vos conversations de support client, tickets et utilisateurs.",
      icon: "💬",
      category: "Communication",
      status: "setup",
      authType: "api_key",
      fields: { token: "" },
      configFields: [
        { name: "token", label: "Access Token Intercom", placeholder: "dG9r...", type: "password" }
      ],
      enabled: false,
      createdAt: "2026-06-29T08:00:00.000Z"
    },
    {
      id: "plg-sentry",
      name: "Sentry",
      description: "Liez votre compte Sentry pour suivre vos erreurs d'application, rapports de plantages et performances en direct.",
      icon: "🎯",
      category: "Développement",
      status: "setup",
      authType: "api_key",
      fields: { token: "", organization_slug: "" },
      configFields: [
        { name: "organization_slug", label: "Slug de votre organisation Sentry", placeholder: "my-org", type: "text" },
        { name: "token", label: "Auth Token Sentry", placeholder: "sntryu_...", type: "password" }
      ],
      enabled: false,
      createdAt: "2026-06-29T08:00:00.000Z"
    },
    {
      id: "plg-datadog",
      name: "Datadog",
      description: "Connectez Datadog pour surveiller vos métriques d'infrastructure, dashboards et alertes système.",
      icon: "🐕",
      category: "Développement",
      status: "setup",
      authType: "api_key",
      fields: { api_key: "", application_key: "", site: "datadoghq.com" },
      configFields: [
        { name: "site", label: "Datadog Site (ex: datadoghq.com, datadoghq.eu)", placeholder: "datadoghq.com", type: "text" },
        { name: "api_key", label: "Datadog API Key", placeholder: "api_key...", type: "password" },
        { name: "application_key", label: "Datadog Application Key", placeholder: "app_key...", type: "password" }
      ],
      enabled: false,
      createdAt: "2026-06-29T08:00:00.000Z"
    },
    {
      id: "plg-plaid",
      name: "Plaid",
      description: "Liez Plaid pour connecter de manière sécurisée vos comptes bancaires et synchroniser vos transactions et soldes.",
      icon: "🏦",
      category: "Finance",
      status: "setup",
      authType: "api_key",
      fields: { client_id: "", secret: "", environment: "sandbox" },
      configFields: [
        { name: "client_id", label: "Plaid Client ID", placeholder: "client_...", type: "text" },
        { name: "secret", label: "Plaid Secret Key", placeholder: "secret_...", type: "password" },
        { name: "environment", label: "Environnement Plaid (sandbox, development, production)", placeholder: "sandbox", type: "text" }
      ],
      enabled: false,
      createdAt: "2026-06-29T08:00:00.000Z"
    },
    {
      id: "plg-paypal",
      name: "PayPal",
      description: "Liez votre compte PayPal Business pour suivre vos transactions, abonnements clients et transferts.",
      icon: "💳",
      category: "Finance",
      status: "setup",
      authType: "api_key",
      fields: { client_id: "", client_secret: "", environment: "sandbox" },
      configFields: [
        { name: "client_id", label: "PayPal Client ID", placeholder: "AcA...", type: "text" },
        { name: "client_secret", label: "PayPal Client Secret", placeholder: "EnG...", type: "password" },
        { name: "environment", label: "Environnement PayPal (sandbox ou live)", placeholder: "sandbox", type: "text" }
      ],
      enabled: false,
      createdAt: "2026-06-29T08:00:00.000Z"
    },
    {
      id: "plg-fitbit",
      name: "Fitbit",
      description: "Liez votre compte Fitbit pour synchroniser votre activité physique, fréquence cardiaque, sommeil et santé.",
      icon: "⌚",
      category: "Santé",
      status: "setup",
      authType: "oauth",
      fields: { token: "" },
      configFields: [
        { name: "token", label: "Jeton d'accès Fitbit (OAuth)", placeholder: "eyJhbGci...", type: "password" }
      ],
      enabled: false,
      createdAt: "2026-06-29T08:00:00.000Z"
    },
    {
      id: "plg-withings",
      name: "Withings",
      description: "Liez vos appareils Withings pour suivre vos mesures de poids, tension artérielle, sommeil et ECG.",
      icon: "🩺",
      category: "Santé",
      status: "setup",
      authType: "oauth",
      fields: { token: "" },
      configFields: [
        { name: "token", label: "Access Token Withings", placeholder: "access_...", type: "password" }
      ],
      enabled: false,
      createdAt: "2026-06-29T08:00:00.000Z"
    },
    {
      id: "plg-supabase",
      name: "Supabase",
      description: "Connectez votre projet Supabase pour surveiller votre base PostgreSQL, utilisateurs et fichiers.",
      icon: "⚡",
      category: "Développement",
      status: "setup",
      authType: "api_key",
      fields: { url: "", anon_key: "" },
      configFields: [
        { name: "url", label: "URL de l'API Supabase (ex: https://xyz.supabase.co)", placeholder: "https://xyz.supabase.co", type: "text" },
        { name: "anon_key", label: "Supabase Anon Key", placeholder: "eyJhbGci...", type: "password" }
      ],
      enabled: false,
      createdAt: "2026-06-29T08:00:00.000Z"
    },
    {
      id: "plg-coinbase",
      name: "Coinbase",
      description: "Liez Coinbase pour suivre votre portefeuille de cryptomonnaies, vos transactions et vos avoirs en direct.",
      icon: "🪙",
      category: "Finance",
      status: "setup",
      authType: "api_key",
      fields: { token: "" },
      configFields: [
        { name: "token", label: "Access Token ou API Key Coinbase", placeholder: "coinbase_...", type: "password" }
      ],
      enabled: false,
      createdAt: "2026-06-29T08:00:00.000Z"
    },
    {
      id: "plg-strava",
      name: "Strava",
      description: "Synchronisez vos activités sportives (course, cyclisme, natation), itinéraires et segments Strava.",
      icon: "🏃",
      category: "Santé",
      status: "setup",
      authType: "oauth",
      fields: { token: "" },
      configFields: [
        { name: "token", label: "Access Token Strava", placeholder: "strava_...", type: "password" }
      ],
      enabled: false,
      createdAt: "2026-06-29T08:00:00.000Z"
    },
    {
      id: "plg-openai",
      name: "OpenAI",
      description: "Connectez votre clé API OpenAI pour utiliser GPT-4o, DALL-E ou Whisper dans vos workflows ARO.",
      icon: "🤖",
      category: "IA",
      status: "setup",
      authType: "api_key",
      fields: { token: "" },
      configFields: [
        { name: "token", label: "OpenAI API Key", placeholder: "sk-proj-...", type: "password" }
      ],
      enabled: false,
      createdAt: "2026-06-29T08:00:00.000Z"
    },
    {
      id: "plg-anthropic",
      name: "Anthropic",
      description: "Connectez votre clé API Anthropic pour utiliser les modèles Claude 3.5 Sonnet dans vos workflows.",
      icon: "🧠",
      category: "IA",
      status: "setup",
      authType: "api_key",
      fields: { token: "" },
      configFields: [
        { name: "token", label: "Anthropic API Key", placeholder: "sk-ant-...", type: "password" }
      ],
      enabled: false,
      createdAt: "2026-06-29T08:00:00.000Z"
    },
    {
      id: "plg-sendgrid",
      name: "SendGrid",
      description: "Synchronisez SendGrid pour envoyer des emails transactionnels et suivre vos statistiques de délivrabilité.",
      icon: "✉️",
      category: "Marketing",
      status: "setup",
      authType: "api_key",
      fields: { token: "" },
      configFields: [
        { name: "token", label: "SendGrid API Key", placeholder: "SG.o...", type: "password" }
      ],
      enabled: false,
      createdAt: "2026-06-29T08:00:00.000Z"
    },
    {
      id: "plg-bitbucket",
      name: "Bitbucket",
      description: "Liez Bitbucket pour synchroniser vos dépôts de code Git, vos pull requests et vos pipelines.",
      icon: "🪣",
      category: "Développement",
      status: "setup",
      authType: "api_key",
      fields: { username: "", app_password: "" },
      configFields: [
        { name: "username", label: "Nom d'utilisateur Bitbucket", placeholder: "username", type: "text" },
        { name: "app_password", label: "Mot de passe d'application Bitbucket", placeholder: "app_pass_...", type: "password" }
      ],
      enabled: false,
      createdAt: "2026-06-29T08:00:00.000Z"
    },
    {
      id: "plg-pinterest",
      name: "Pinterest",
      description: "Synchronisez vos tableaux et épingles Pinterest pour automatiser vos partages créatifs.",
      icon: "📌",
      category: "Marketing",
      status: "setup",
      authType: "oauth",
      fields: { token: "" },
      configFields: [
        { name: "token", label: "Access Token Pinterest", placeholder: "pina_...", type: "password" }
      ],
      enabled: false,
      createdAt: "2026-06-29T08:00:00.000Z"
    },
    {
      id: "plg-reddit",
      name: "Reddit",
      description: "Connectez Reddit pour suivre des subreddits, rechercher des posts ou automatiser la publication.",
      icon: "👽",
      category: "Marketing",
      status: "setup",
      authType: "oauth",
      fields: { token: "" },
      configFields: [
        { name: "token", label: "Access Token Reddit", placeholder: "reddit_token_...", type: "password" }
      ],
      enabled: false,
      createdAt: "2026-06-29T08:00:00.000Z"
    },
    {
      id: "plg-ghost",
      name: "Ghost CMS",
      description: "Synchronisez Ghost pour publier des articles et gérer les newsletters de votre site internet.",
      icon: "👻",
      category: "Marketing",
      status: "setup",
      authType: "api_key",
      fields: { url: "", admin_api_key: "" },
      configFields: [
        { name: "url", label: "URL de votre blog Ghost", placeholder: "https://monblog.ghost.io", type: "text" },
        { name: "admin_api_key", label: "Admin API Key Ghost", placeholder: "key_...", type: "password" }
      ],
      enabled: false,
      createdAt: "2026-06-29T08:00:00.000Z"
    },
    {
      id: "plg-twitch",
      name: "Twitch",
      description: "Suivez vos streams favoris, le statut en direct de vos chaînes ou interagissez avec le chat Twitch.",
      icon: "📺",
      category: "Communication",
      status: "setup",
      authType: "oauth",
      fields: { token: "", client_id: "" },
      configFields: [
        { name: "token", label: "User OAuth Access Token Twitch", placeholder: "oauth:...", type: "password" },
        { name: "client_id", label: "Client ID de votre application Twitch", placeholder: "client_id_...", type: "text" }
      ],
      enabled: false,
      createdAt: "2026-06-29T08:00:00.000Z"
    },
    {
      id: "plg-calendly",
      name: "Calendly",
      description: "Synchronisez vos rendez-vous et vos événements de planification directement depuis Calendly.",
      icon: "📅",
      category: "Productivité",
      status: "setup",
      authType: "api_key",
      fields: { token: "" },
      configFields: [
        { name: "token", label: "Personal Access Token Calendly", placeholder: "calendly_...", type: "password" }
      ],
      enabled: false,
      createdAt: "2026-06-29T08:00:00.000Z"
    },
    {
      id: "plg-digitalocean",
      name: "DigitalOcean",
      description: "Liez DigitalOcean pour suivre vos Droplets, vos volumes de stockage et l'état de vos bases de données.",
      icon: "🌊",
      category: "Cloud",
      status: "setup",
      authType: "api_key",
      fields: { token: "" },
      configFields: [
        { name: "token", label: "Personal Access Token DigitalOcean", placeholder: "dop_v1_...", type: "password" }
      ],
      enabled: false,
      createdAt: "2026-06-29T08:00:00.000Z"
    },
    {
      id: "plg-airtable",
      name: "Airtable",
      description: "Synchronisez ARO avec vos bases Airtable pour lire, écrire et modifier des données structurées.",
      icon: "📊",
      category: "Productivité",
      status: "setup",
      authType: "api_key",
      fields: { token: "" },
      configFields: [
        { name: "token", label: "Airtable Personal Access Token (PAT)", placeholder: "pat.key...", type: "password" }
      ],
      enabled: false,
      createdAt: "2026-06-29T08:00:00.000Z"
    },
    {
      id: "plg-monday",
      name: "Monday.com",
      description: "Connectez Monday.com pour suivre vos tableaux de bord, vos tâches d'équipe et vos jalons de projet.",
      icon: "📅",
      category: "Productivité",
      status: "setup",
      authType: "api_key",
      fields: { token: "" },
      configFields: [
        { name: "token", label: "Monday API Token", placeholder: "eyJhbGci...", type: "password" }
      ],
      enabled: false,
      createdAt: "2026-06-29T08:00:00.000Z"
    },
    {
      id: "plg-heroku",
      name: "Heroku",
      description: "Synchronisez Heroku pour suivre l'état de vos applications, vos dynos et vos bases de données actives.",
      icon: "💜",
      category: "Développement",
      status: "setup",
      authType: "api_key",
      fields: { token: "" },
      configFields: [
        { name: "token", label: "Heroku API Key", placeholder: "key_...", type: "password" }
      ],
      enabled: false,
      createdAt: "2026-06-29T08:00:00.000Z"
    },
    {
      id: "plg-netlify",
      name: "Netlify",
      description: "Liez Netlify pour suivre vos déploiements de sites web, vos formulaires de contact et vos fonctions serverless.",
      icon: "⚡",
      category: "Développement",
      status: "setup",
      authType: "api_key",
      fields: { token: "" },
      configFields: [
        { name: "token", label: "Netlify Personal Access Token", placeholder: "netlify_...", type: "password" }
      ],
      enabled: false,
      createdAt: "2026-06-29T08:00:00.000Z"
    },
    {
      id: "plg-pipedrive",
      name: "Pipedrive",
      description: "Synchronisez Pipedrive pour suivre vos deals, vos contacts commerciaux et vos activités de vente.",
      icon: "🦊",
      category: "Productivité",
      status: "setup",
      authType: "api_key",
      fields: { api_token: "" },
      configFields: [
        { name: "api_token", label: "Pipedrive API Token", placeholder: "token_...", type: "password" }
      ],
      enabled: false,
      createdAt: "2026-06-29T08:00:00.000Z"
    },
    {
      id: "plg-buffer",
      name: "Buffer",
      description: "Liez Buffer pour planifier des publications et suivre l'engagement sur vos réseaux sociaux.",
      icon: "🥞",
      category: "Marketing",
      status: "setup",
      authType: "api_key",
      fields: { token: "" },
      configFields: [
        { name: "token", label: "Buffer Access Token", placeholder: "buf_...", type: "password" }
      ],
      enabled: false,
      createdAt: "2026-06-29T08:00:00.000Z"
    },
    {
      id: "plg-miro",
      name: "Miro",
      description: "Liez Miro pour suivre vos tableaux blancs virtuels, vos schémas et vos brainstormings d'équipe.",
      icon: "🎨",
      category: "Productivité",
      status: "setup",
      authType: "api_key",
      fields: { token: "" },
      configFields: [
        { name: "token", label: "Miro OAuth Token", placeholder: "miro_...", type: "password" }
      ],
      enabled: false,
      createdAt: "2026-06-29T08:00:00.000Z"
    },
    {
      id: "plg-gitbook",
      name: "GitBook",
      description: "Synchronisez GitBook pour documenter vos projets et suivre les modifications de vos espaces de travail.",
      icon: "📖",
      category: "Notes",
      status: "setup",
      authType: "api_key",
      fields: { token: "" },
      configFields: [
        { name: "token", label: "Personal Access Token GitBook", placeholder: "gb_api_...", type: "password" }
      ],
      enabled: false,
      createdAt: "2026-06-29T08:00:00.000Z"
    },
    {
      id: "plg-mailgun",
      name: "Mailgun",
      description: "Connectez Mailgun pour envoyer des emails transactionnels, gérer des domaines et suivre les taux de rebond.",
      icon: "🔫",
      category: "Marketing",
      status: "setup",
      authType: "api_key",
      fields: { domain: "", api_key: "" },
      configFields: [
        { name: "domain", label: "Domaine Mailgun", placeholder: "mg.exemple.com", type: "text" },
        { name: "api_key", label: "Mailgun Private API Key", placeholder: "key-...", type: "password" }
      ],
      enabled: false,
      createdAt: "2026-06-29T08:00:00.000Z"
    },
    {
      id: "plg-plausible",
      name: "Plausible",
      description: "Suivez vos statistiques d'audience web en temps réel de manière respectueuse de la vie privée avec Plausible.",
      icon: "📈",
      category: "Marketing",
      status: "setup",
      authType: "api_key",
      fields: { site_id: "", token: "" },
      configFields: [
        { name: "site_id", label: "ID du Site Plausible (ex: exemple.com)", placeholder: "exemple.com", type: "text" },
        { name: "token", label: "API Token Plausible", placeholder: "plausible_token...", type: "password" }
      ],
      enabled: false,
      createdAt: "2026-06-29T08:00:00.000Z"
    },
    {
      id: "plg-notion",
      name: "Notion",
      description: "Synchronisez ARO avec Notion pour lire, rechercher et ajouter des pages à vos bases de données.",
      icon: "📓",
      category: "Notes",
      status: "setup",
      authType: "api_key",
      fields: { integration_token: "", page_id: "" },
      configFields: [
        { name: "integration_token", label: "Jeton d'intégration Notion", placeholder: "secret_...", type: "password" },
        { name: "page_id", label: "ID Page Notion Parent", placeholder: "optional page id", type: "text" }
      ],
      enabled: false,
      createdAt: "2026-06-29T08:00:00.000Z"
    }
    ];
}
export function isSensitiveIntegrationField(name: string): boolean {
  const normalized = name.toLowerCase();
  return normalized.includes("token") || normalized.includes("secret") || normalized.includes("password")
    || normalized.includes("private") || normalized === "api_key" || normalized.endsWith("_api_key") || normalized.endsWith("key");
}
export function pluginSafeForLocalStorage(plugin: UserPlugin): UserPlugin {
  return { ...plugin, fields: Object.fromEntries(Object.entries(plugin.fields).map(([key, value]) =>
    [key, isSensitiveIntegrationField(key) && value ? "" : value])) };
}
export function getPluginsByCategory(plugins: UserPlugin[]): Record<string, UserPlugin[]> {
  const groups: Record<string, UserPlugin[]> = {};
  for (const plugin of plugins) {
    const category = plugin.category || "Autre";
    if (!groups[category]) groups[category] = [];
    groups[category].push(plugin);
  }
  return groups;
}
export function pluginPayload(plugin: UserPlugin, fields: Record<string, string> = plugin.fields) {
  return { clientId: plugin.id, name: plugin.name, description: plugin.description, category: plugin.category,
    status: plugin.status, authType: plugin.authType, enabled: plugin.enabled, fields,
    config: { clientId: plugin.id, configFields: plugin.configFields, profileName: plugin.profileName,
      profileAvatar: plugin.profileAvatar, profileDetails: plugin.profileDetails, subServices: plugin.subServices } };
}
export function mergePluginPresets(presets: UserPlugin[], persisted: UserPlugin[]): UserPlugin[] {
  const byId = new Map<string, UserPlugin>();
  for (const plugin of presets) byId.set(plugin.id, plugin);
  for (const plugin of persisted) {
    const current = byId.get(plugin.id);
    byId.set(plugin.id, current ? { ...current, ...plugin } : plugin);
  }
  return Array.from(byId.values());
}
