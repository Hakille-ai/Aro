<script lang="ts">
  import Building2 from "@lucide/svelte/icons/building-2";
  import Eye from "@lucide/svelte/icons/eye";
  import EyeOff from "@lucide/svelte/icons/eye-off";
  import Info from "@lucide/svelte/icons/info";
  import Lock from "@lucide/svelte/icons/lock";
  import Mail from "@lucide/svelte/icons/mail";
  import RefreshCw from "@lucide/svelte/icons/refresh-cw";
  import User from "@lucide/svelte/icons/user";
  import { fade } from "svelte/transition";

  type AuthMode = "login" | "register" | "invitation" | "forgot-password" | "reset-password";

  export let language: "fr" | "en";
  export let mode: AuthMode;
  export let invitationToken: string;
  export let email: string;
  export let password: string;
  export let name: string;
  export let organizationName: string;
  export let showPassword: boolean;
  export let busy: boolean;
  export let error: string;
  export let successMessage: string = "";
  export let devTokenUrl: string = "";
  export let onSubmit: () => void | Promise<void>;
  export let onToggleMode: () => void;
  export let onForgotPassword: () => void = () => {};
</script>

<main class="cloud-auth-page">
  <div class="auth-bg-aurora aurora-1"></div>
  <div class="auth-bg-aurora aurora-2"></div>
  <div class="auth-bg-aurora aurora-3"></div>

  <div class="cloud-auth-container">
    {#if mode === "login" || mode === "invitation"}
      <form class="cloud-auth-panel glassmorphic-panel" on:submit|preventDefault={onSubmit} transition:fade={{ duration: 200 }}>
        <div class="cloud-auth-brand">
          <div class="brand-logo-container">
            <img src="/logo.png" alt="ARO logo" />
            <div class="logo-reflection"></div>
          </div>
          <div class="brand-info"><span class="brand-title">ARO</span></div>
        </div>

        <div class="auth-header-section">
          <h1>{mode === "invitation" ? (language === "fr" ? "Accepter l’invitation" : "Accept invitation") : (language === "fr" ? "Connexion" : "Sign In")}</h1>
          <p>{mode === "invitation" ? (language === "fr" ? "Confirmez votre adresse et choisissez ou saisissez votre mot de passe." : "Confirm your email and choose or enter your password.") : (language === "fr" ? "Accédez à votre espace de travail" : "Access your workspace")}</p>
        </div>

        <div class="auth-inputs-group">
          <div class="input-wrapper">
            <Mail size={16} class="input-icon-left" />
            <input class="cloud-auth-input" bind:value={email} placeholder={language === "fr" ? "Adresse e-mail" : "Email address"} type="email" autocomplete="email" required />
          </div>
          <div class="input-wrapper password-wrapper">
            <Lock size={16} class="input-icon-left" />
            <input class="cloud-auth-input password-input" bind:value={password} placeholder={language === "fr" ? "Mot de passe (min. 10 caractères)" : "Password (min. 10 chars)"} type={showPassword ? "text" : "password"} autocomplete="current-password" required />
            <button class="password-toggle-btn" type="button" on:click={() => (showPassword = !showPassword)}>
              {#if showPassword}<EyeOff size={16} />{:else}<Eye size={16} />{/if}
            </button>
          </div>
        </div>

        {#if mode === "login"}
          <div class="forgot-password-row">
            <button type="button" class="link-btn text-sm" on:click={onForgotPassword}>
              {language === "fr" ? "Mot de passe oublié ?" : "Forgot password?"}
            </button>
          </div>
        {/if}

        {#if error}
          <div class="cloud-auth-error-banner" transition:fade={{ duration: 150 }}><Info size={14} class="error-icon" /><span>{error}</span></div>
        {/if}

        <button class="cloud-auth-btn-primary" type="submit" disabled={busy || !email.trim() || password.length < 10 || (mode === "invitation" && invitationToken.length < 32)}>
          {#if busy}
            <RefreshCw size={16} class="spinning-icon" /><span>{language === "fr" ? "Connexion en cours..." : "Connecting..."}</span>
          {:else}
            <span>{mode === "invitation" ? (language === "fr" ? "Rejoindre le workspace" : "Join workspace") : (language === "fr" ? "Se connecter" : "Sign In")}</span>
          {/if}
        </button>

        <div class="auth-switch-link">
          <span>
            {mode === "invitation" ? (language === "fr" ? "Vous avez déjà accepté ?" : "Already accepted?") : (language === "fr" ? "Nouveau sur ARO ?" : "New to ARO?")}
            <button type="button" class="link-btn" on:click={onToggleMode}>{mode === "invitation" ? (language === "fr" ? "Se connecter" : "Sign in") : (language === "fr" ? "Créer un compte" : "Create account")}</button>
          </span>
        </div>
      </form>
    {:else if mode === "forgot-password"}
      <form class="cloud-auth-panel glassmorphic-panel" on:submit|preventDefault={onSubmit} transition:fade={{ duration: 200 }}>
        <div class="cloud-auth-brand">
          <div class="brand-logo-container">
            <img src="/logo.png" alt="ARO logo" />
            <div class="logo-reflection"></div>
          </div>
          <div class="brand-info"><span class="brand-title">ARO</span></div>
        </div>

        <div class="auth-header-section">
          <h1>{language === "fr" ? "Mot de passe oublié" : "Reset Password"}</h1>
          <p>{language === "fr" ? "Entrez votre adresse e-mail pour recevoir un lien de réinitialisation sécurisé." : "Enter your email to receive a secure reset link."}</p>
        </div>

        <div class="auth-inputs-group">
          <div class="input-wrapper">
            <Mail size={16} class="input-icon-left" />
            <input class="cloud-auth-input" bind:value={email} placeholder={language === "fr" ? "Adresse e-mail" : "Email address"} type="email" autocomplete="email" required />
          </div>
        </div>

        {#if successMessage}
          <div class="cloud-auth-success-banner" transition:fade={{ duration: 150 }}>
            <span>{successMessage}</span>
            {#if devTokenUrl}
              <div class="dev-token-box" style="margin-top: 8px; font-size: 11px; opacity: 0.8; word-break: break-all;">
                Lien direct de test : <code>{devTokenUrl}</code>
              </div>
            {/if}
          </div>
        {/if}

        {#if error}
          <div class="cloud-auth-error-banner" transition:fade={{ duration: 150 }}><Info size={14} class="error-icon" /><span>{error}</span></div>
        {/if}

        <button class="cloud-auth-btn-primary" type="submit" disabled={busy || !email.trim()}>
          {#if busy}
            <RefreshCw size={16} class="spinning-icon" /><span>{language === "fr" ? "Envoi en cours..." : "Sending..."}</span>
          {:else}
            <span>{language === "fr" ? "Envoyer l'e-mail de réinitialisation" : "Send reset link"}</span>
          {/if}
        </button>

        <div class="auth-switch-link">
          <button type="button" class="link-btn" on:click={onToggleMode}>
            {language === "fr" ? "← Retour à la connexion" : "← Back to sign in"}
          </button>
        </div>
      </form>
    {:else}
      <form class="cloud-auth-panel glassmorphic-panel" on:submit|preventDefault={onSubmit} transition:fade={{ duration: 200 }}>
        <div class="cloud-auth-brand">
          <div class="brand-logo-container">
            <img src="/logo.png" alt="ARO logo" />
            <div class="logo-reflection"></div>
          </div>
          <div class="brand-info"><span class="brand-title">ARO</span></div>
        </div>

        <div class="auth-header-section"><h1>{language === "fr" ? "Créer votre espace" : "Create Account"}</h1></div>
        <div class="auth-inputs-group">
          <div class="input-wrapper">
            <User size={16} class="input-icon-left" />
            <input class="cloud-auth-input" bind:value={name} placeholder={language === "fr" ? "Nom complet" : "Full name"} autocomplete="name" required />
          </div>
          <div class="input-wrapper">
            <Building2 size={16} class="input-icon-left" />
            <input class="cloud-auth-input" bind:value={organizationName} placeholder={language === "fr" ? "Nom de l'organisation" : "Organization name"} autocomplete="organization" />
          </div>
          <div class="input-wrapper">
            <Mail size={16} class="input-icon-left" />
            <input class="cloud-auth-input" bind:value={email} placeholder={language === "fr" ? "Adresse e-mail" : "Email address"} type="email" autocomplete="email" required />
          </div>
          <div class="input-wrapper password-wrapper">
            <Lock size={16} class="input-icon-left" />
            <input class="cloud-auth-input password-input" bind:value={password} placeholder={language === "fr" ? "Mot de passe (min. 10 caractères)" : "Password (min. 10 chars)"} type={showPassword ? "text" : "password"} autocomplete="new-password" required />
            <button class="password-toggle-btn" type="button" on:click={() => (showPassword = !showPassword)}>
              {#if showPassword}<EyeOff size={16} />{:else}<Eye size={16} />{/if}
            </button>
          </div>
        </div>

        {#if error}
          <div class="cloud-auth-error-banner" transition:fade={{ duration: 150 }}><Info size={14} class="error-icon" /><span>{error}</span></div>
        {/if}

        <button class="cloud-auth-btn-primary" type="submit" disabled={busy || !email.trim() || password.length < 10}>
          {#if busy}<RefreshCw size={16} class="spinning-icon" /><span>{language === "fr" ? "Création..." : "Creating..."}</span>{:else}<span>{language === "fr" ? "Créer l'espace" : "Create Workspace"}</span>{/if}
        </button>

        <div class="auth-switch-link">
          <span>{language === "fr" ? "Vous avez déjà un compte ?" : "Already have an account?"}<button type="button" class="link-btn" on:click={onToggleMode}>{language === "fr" ? "Se connecter" : "Sign in"}</button></span>
        </div>
      </form>
    {/if}
  </div>
</main>

<style>
  .forgot-password-row {
    display: flex;
    justify-content: flex-end;
    margin-top: -6px;
    margin-bottom: 8px;
  }

  .link-btn.text-sm {
    font-size: 0.78rem;
    color: #3b82f6;
    background: transparent;
    border: none;
    cursor: pointer;
    text-decoration: underline;
    transition: color 0.15s ease;
  }

  .link-btn.text-sm:hover {
    color: #60a5fa;
  }

  .cloud-auth-success-banner {
    background: rgba(16, 185, 129, 0.12);
    border: 1px solid rgba(16, 185, 129, 0.3);
    color: #10b981;
    border-radius: 8px;
    padding: 10px 14px;
    font-size: 0.8rem;
    line-height: 1.4;
    margin-bottom: 12px;
  }
</style>
