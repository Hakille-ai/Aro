<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { billingRequest } from "../../../lib/api/billing";
  import { defaultCatalog, eurosToMicros, formatMicros, trustedPaymentUrl, type CommercialCatalog, type BillingSnapshot, type ComputeKey } from "../../billing/model";
  export let language: "fr" | "en" = "fr";
  export let authenticated = false;
  let catalog: CommercialCatalog = defaultCatalog;
  let snapshot: BillingSnapshot | null = null;
  let keys: ComputeKey[] = [];
  let loading = true, busy = false, error = "", notice = "", keySecret = "", keyName = "";
  let monthly = "0", perRequest = "0", seats = 1;
  let disposed = false, revision = 0;
  const requestIds = new Map<string, string>();
  $: fr = language === "fr";
  $: available = Math.max(0, (snapshot?.account.balanceMicros ?? 0) - (snapshot?.account.reservedMicros ?? 0));
  $: editable = !!snapshot?.canManage && !busy && !loading;
  $: statusLabel = snapshot?.entitlements.managedSync ? (fr ? "Abonnement actif" : "Active subscription") : (fr ? "Aucun abonnement cloud actif" : "No active cloud subscription");
  const descriptions: Record<string, [string, string]> = {
    community: ["Votre espace local, vos modèles et votre clé personnelle. Usages permis par PolyForm Noncommercial 1.0.0.", "Your local workspace, models and personal key. Uses permitted by PolyForm Noncommercial 1.0.0."],
    cloud: ["Synchronisation personnelle sur le cloud ARO. Calcul IA séparé.", "Personal sync on ARO Cloud. AI compute is separate."],
    business: ["Licence commerciale, espace d’équipe et administration. Modèles locaux ou clé personnelle acceptés.", "Commercial license, team workspace and administration. Local models and personal keys supported."],
    enterprise: ["Déploiement accompagné et engagements définis par contrat. Infrastructure dédiée sur devis.", "Assisted deployment and contractual commitments. Dedicated infrastructure quoted separately."],
  };
  function money(value: number) { return formatMicros(value, language); }
  async function refresh() {
    const current = ++revision;
    loading = true; error = ""; keySecret = "";
    try {
      const nextCatalog = await billingRequest<CommercialCatalog>("GET", "/billing/catalog");
      const next = authenticated ? await billingRequest<BillingSnapshot>("GET", "/billing/account") : null;
      const nextKeys = next?.canManage ? await billingRequest<ComputeKey[]>("GET", "/billing/compute-keys") : [];
      if (disposed || current !== revision) return;
      catalog = nextCatalog; snapshot = next; keys = nextKeys;
      monthly = String((next?.account.monthlyLimitMicros ?? 0) / 1_000_000);
      perRequest = String((next?.account.perRequestLimitMicros ?? 0) / 1_000_000);
      seats = next?.account.seats ?? 1;
    } catch (e) { if (!disposed && current === revision) { snapshot = null; keys = []; error = String((e as Error).message ?? e); } }
    finally { if (!disposed && current === revision) loading = false; }
  }
  async function action(work: () => Promise<void>) {
    if (busy) return; busy = true; error = ""; notice = "";
    try { await work(); } catch (e) { if (!disposed) error = String((e as Error).message ?? e); }
    finally { if (!disposed) busy = false; }
  }
  async function openPayment(url: string) {
    const safe = trustedPaymentUrl(url); if (disposed) return;
    if ("__TAURI_INTERNALS__" in window) await invoke("billing_open_payment", { url: safe });
    else window.location.assign(safe);
  }
  async function checkout(plan: string, amountCents = 0) {
    const identity = `${plan}:${plan === "business" ? seats : 1}:${amountCents}`;
    let requestId = requestIds.get(identity);
    if (!requestId) { requestId = crypto.randomUUID(); requestIds.set(identity, requestId); }
    const result = await billingRequest<{ url: string }>("POST", "/billing/checkout", { requestId, plan, seats: plan === "business" ? seats : 1, amountCents });
    await openPayment(result.url);
  }
  async function saveLimits() {
    const monthlyLimitMicros = eurosToMicros(monthly), perRequestLimitMicros = eurosToMicros(perRequest);
    if (perRequestLimitMicros > monthlyLimitMicros) throw new Error(fr ? "Le plafond par requête doit être inférieur au plafond mensuel." : "The request limit must not exceed the monthly limit.");
    await billingRequest("PUT", "/billing/limits", { monthlyLimitMicros, perRequestLimitMicros });
    await refresh(); notice = fr ? "Budgets enregistrés." : "Budgets saved.";
  }
  async function createKey() {
    const result = await billingRequest<{ secret: string }>("POST", "/billing/compute-keys", { name: keyName.trim() });
    if (disposed) return;
    keySecret = result.secret; keyName = "";
    const next = await billingRequest<ComputeKey[]>("GET", "/billing/compute-keys");
    if (!disposed) keys = next;
  }
  onMount(() => { void refresh(); });
  onDestroy(() => { disposed = true; revision++; keySecret = ""; });
</script>

<section class="billing-page" aria-labelledby="billing-title">
  {#if snapshot && snapshot.account.balanceMicros < 0}<p class="banner error">{fr ? "Le solde est négatif à la suite d’une régularisation. Consultez l’historique ou contactez l’assistance avant de relancer le calcul." : "Your balance is negative after an adjustment. Review the ledger or contact support before using compute."} {money(snapshot.account.balanceMicros)}</p>{/if}
  {#if snapshot?.pendingCheckoutId && !snapshot.entitlements.managedSync}
    <div class="banner"><p>{fr ? "Une commande est déjà en attente pour cet espace." : "This workspace has a pending checkout."}</p><button disabled={!editable || !snapshot.checkoutAvailable} on:click={() => action(async () => { const result = await billingRequest<{url:string}>("POST", "/billing/checkout/resume", {requestId:snapshot?.pendingCheckoutId}); await openPayment(result.url); })}>{fr ? "Reprendre le paiement" : "Resume checkout"}</button></div>
  {/if}
  <div class="heading"><div><p class="eyebrow">ARO</p><h1 id="billing-title">{fr ? "Offre & consommation" : "Plan & usage"}</h1><p class="quiet">{fr ? "Votre logiciel, votre cloud, votre budget IA." : "Your software, your cloud, your AI budget."}</p></div><button class="secondary" disabled={loading || busy} on:click={refresh}>{fr ? "Actualiser" : "Refresh"}</button></div>
  {#if error}<p class="banner error" role="alert">{error}</p>{/if}
  {#if notice}<p class="banner" role="status">{notice}</p>{/if}
  {#if loading}<p role="status">{fr ? "Chargement de votre offre…" : "Loading your plan…"}</p>{/if}
  {#if !authenticated}<p class="banner">{fr ? "Connectez-vous pour gérer votre offre." : "Sign in to manage your plan."}</p>{/if}
  {#if snapshot}
    <div class="current-plan"><div><p class="eyebrow">{fr ? "Offre de cet espace" : "Workspace plan"}</p><h2>{catalog.plans.find(p => p.id === snapshot?.account.plan)?.name ?? "Community"}</h2><p>{statusLabel}{#if snapshot.account.validUntil} · {new Date(snapshot.account.validUntil).toLocaleDateString(language)}{/if}</p>{#if snapshot.account.cancelAtPeriodEnd}<p>{fr ? "Résiliation prévue en fin de période." : "Cancels at period end."}</p>{/if}</div><button class="secondary" disabled={!editable || !snapshot.portalAvailable} on:click={() => action(async () => { const r = await billingRequest<{url:string}>("POST", "/billing/portal"); await openPayment(r.url); })}>{fr ? "Abonnement et factures" : "Subscription and invoices"}</button></div>
  {/if}
  <p class="quiet">{fr ? "Tarifs de lancement proposés. Aucun abonnement n’inclut une consommation IA illimitée." : "Proposed launch prices. No subscription includes unlimited AI usage."}</p>
  <div class="offers">{#each catalog.plans as offer}<article class:featured={offer.id === "business"}>
    <h2>{offer.name}</h2><p class="price">{#if offer.startingAt}<small>{fr ? "À partir de " : "From "}</small>{/if}{money(offer.amountCents * 10_000)}</p>
    <p class="quiet">{offer.amountCents === 0 ? (fr ? "sans abonnement" : "no subscription") : `${offer.taxBehavior === "inclusive" ? (fr ? "TTC" : "incl. tax") : (fr ? "HT" : "excl. tax")} / ${offer.perSeat ? (fr ? "utilisateur / " : "user / ") : ""}${offer.interval === "year" ? (fr ? "an" : "year") : (fr ? "mois" : "month")}`}</p>
    <p class="description">{descriptions[offer.id]?.[fr ? 0 : 1]}</p>
    {#if offer.id === "business"}<label>{fr ? "Utilisateurs" : "Users"}<input type="number" min="1" max="1000" step="1" bind:value={seats} disabled={!editable}/></label>{/if}
    {#if offer.selfServe}<button disabled={!editable || !snapshot?.checkoutAvailable || (offer.id === "business" && (!Number.isInteger(seats) || seats < 1 || seats > 1000)) || snapshot?.entitlements.managedSync} on:click={() => action(() => checkout(offer.id))}>{snapshot?.account.plan === offer.id && snapshot.entitlements.managedSync ? (fr ? "Offre actuelle" : "Current plan") : (fr ? "Choisir cette offre" : "Choose plan")}</button>
    {:else if offer.id === "enterprise" && catalog.salesEmail}<a class="button secondary" href={`mailto:${catalog.salesEmail}`}>{fr ? "Contacter l’équipe" : "Contact sales"}</a>
    {:else}<p class="quiet">{offer.id === "community" ? (fr ? "Modèles locaux et clé personnelle" : "Local models and personal key") : (fr ? "Disponible sur contrat" : "Available by agreement")}</p>{/if}
  </article>{/each}</div>
  {#if !catalog.checkoutAvailable}<p class="quiet">{fr ? "L’achat n’est pas encore activé sur ce serveur. Aucun paiement ni changement d’offre ne sera simulé." : "Purchases are not enabled on this server. Payments and plan changes are never simulated."}</p>{/if}
  <h2 class="section-title">{fr ? "Calcul IA ARO" : "ARO AI compute"}</h2><p>{fr ? "Le calcul est séparé de votre abonnement. Les modèles locaux et les clés personnelles ne consomment pas de crédit ARO." : "Compute is separate from your subscription. Local models and personal provider keys do not consume ARO credit."}</p>
  {#if snapshot}
    <div class="stats"><div><span>{fr ? "Disponible" : "Available"}</span><strong>{money(available)}</strong></div><div><span>{fr ? "Réservé" : "Reserved"}</span><strong>{money(snapshot.account.reservedMicros)}</strong></div><div><span>{fr ? "Ce mois · UTC" : "This month · UTC"}</span><strong>{money(snapshot.account.monthSpentMicros)}</strong></div></div>
    {#if snapshot.account.balanceMicros < 0}<p class="banner error">{fr ? "Solde négatif après remboursement ou litige. Le calcul est suspendu." : "Negative balance after a refund or dispute. Compute is suspended."}</p>{/if}
    {#if snapshot.canManage}
      <div class="topups">{#each [1000,2500,5000,10000] as cents}<button class="secondary" disabled={!editable || !snapshot.checkoutAvailable || !snapshot.computeAvailable} on:click={() => action(() => checkout("credit", cents))}>+ {money(cents * 10_000)} {fr ? "HT de crédit" : "credit, excl. tax"}</button>{/each}</div>
      <form class="budgets" on:submit|preventDefault={() => action(saveLimits)}><label>{fr ? "Plafond mensuel (€)" : "Monthly limit (€)"}<input inputmode="decimal" bind:value={monthly} disabled={!editable}/></label><label>{fr ? "Plafond par requête (€)" : "Per-request limit (€)"}<input inputmode="decimal" bind:value={perRequest} disabled={!editable}/></label><button disabled={!editable}>{fr ? "Enregistrer les budgets" : "Save budgets"}</button></form>
      <p class="quiet">{fr ? "Un plafond à zéro désactive le calcul payant. Les requêtes en cours comptent dans le budget. Une réservation incertaine reste bloquée jusqu’à vérification." : "A zero limit disables paid compute. Running requests count toward budgets. Uncertain reservations remain held for review."}</p>
      <h3>{fr ? "Clés de calcul" : "Compute keys"}</h3><form class="key-form" on:submit|preventDefault={() => action(createKey)}><input aria-label={fr ? "Nom de la clé" : "Key name"} placeholder={fr ? "Ex. Mon ordinateur" : "E.g. My computer"} maxlength="80" bind:value={keyName} disabled={!editable}/><button disabled={!editable || !snapshot.computeAvailable || !keyName.trim()}>{fr ? "Créer une clé" : "Create key"}</button></form>
      {#if keySecret}<div class="secret"><p>{fr ? "Copiez cette clé maintenant. Elle ne sera plus affichée après actualisation." : "Copy this key now. It will not be displayed after refresh."}</p><code>{keySecret}</code><button class="secondary" on:click={() => action(async () => { await navigator.clipboard.writeText(keySecret); notice = fr ? "Clé copiée." : "Key copied."; })}>{fr ? "Copier" : "Copy"}</button></div>{/if}
      {#each keys as key}<div class="key-row"><span>{key.name} <code>{key.prefix}…</code></span><button class="secondary" disabled={!editable} on:click={() => action(async () => { await billingRequest("DELETE", `/billing/compute-keys/${key.id}`); await refresh(); })}>{fr ? "Révoquer" : "Revoke"}</button></div>{/each}
      <p class="quiet">{fr ? "Dans Modèles, ajoutez un fournisseur OpenAI-compatible avec l’adresse /v1 du serveur ARO et cette clé. Elle autorise uniquement le calcul sur le budget de cet espace." : "In Models, add an OpenAI-compatible provider using your ARO server’s /v1 address and this key. The key only permits compute against this workspace budget."}</p>
    {/if}
    {#if !snapshot.computeAvailable}<p class="banner">{fr ? "Aucun modèle payant configuré sur ce serveur. Vos modèles locaux et clés personnelles restent disponibles." : "No paid model configured. Local models and personal keys remain available."}</p>{/if}
    {#if snapshot.ledger.length}<h3>{fr ? "Dernières opérations" : "Recent transactions"}</h3><div class="ledger"><table><thead><tr><th>Date</th><th>{fr ? "Opération" : "Operation"}</th><th>{fr ? "Montant" : "Amount"}</th></tr></thead><tbody>{#each snapshot.ledger as entry}<tr><td>{new Date(entry.createdAt).toLocaleString(language)}</td><td>{entry.kind}</td><td>{money(entry.amountMicros)}</td></tr>{/each}</tbody></table></div>{/if}
  {/if}
  <footer><strong>{fr ? "Vos données restent les vôtres." : "Your data remains yours."}</strong><p>{fr ? "L’accès aux données locales et leur export restent disponibles à la fin d’un abonnement. Le cœur ARO est sous PolyForm Noncommercial 1.0.0. Les usages non permis par cette licence nécessitent un accord commercial distinct." : "Local data access and export remain available after a subscription ends. ARO core uses PolyForm Noncommercial 1.0.0. Uses outside its permitted purposes require a separate commercial agreement."}</p></footer>
</section>

<style>
  .billing-page{max-width:1080px;margin:auto;padding:32px 28px 64px;color:inherit}.heading,.current-plan,.key-row{display:flex;align-items:center;justify-content:space-between;gap:20px}.eyebrow{font-size:11px;font-weight:650;letter-spacing:.09em;text-transform:uppercase;opacity:.55}h1{font-size:28px;letter-spacing:-.8px;margin:0}h2{font-size:19px;margin:0 0 10px}h3{font-size:16px;margin-top:30px}.quiet{opacity:.65;font-size:13px;line-height:1.6}.current-plan{margin:28px 0;padding:22px;border:1px solid #8883;border-radius:16px}.current-plan p{font-size:13px}.offers{display:grid;grid-template-columns:repeat(2,minmax(0,1fr));gap:14px;margin:20px 0}.offers article{display:flex;flex-direction:column;border:1px solid #8883;border-radius:16px;padding:23px}.featured{border-color:#3478f6!important}.price{font-size:27px;font-weight:650;letter-spacing:-.8px;margin:5px 0}.price small{font-size:12px;font-weight:400;display:block;letter-spacing:normal}.description{font-size:13px;line-height:1.65;flex:1;min-height:65px}button,.button{border:0;border-radius:8px;background:#2563eb;color:white;padding:10px 15px;font-size:13px;cursor:pointer;text-align:center;text-decoration:none;font-weight:550}button:disabled{opacity:.4;cursor:not-allowed}.secondary{background:#8881;color:inherit;border:1px solid #8883}button:focus-visible,a:focus-visible,input:focus-visible{outline:2px solid #3478f6;outline-offset:3px}.section-title{margin-top:38px}.stats{display:grid;grid-template-columns:repeat(3,minmax(0,1fr));gap:12px;margin:24px 0}.stats div{padding:20px;border:1px solid #8883;border-radius:13px}.stats span{font-size:12px;opacity:.65;display:block;margin-bottom:10px}.stats strong{font-size:24px}.topups{display:flex;flex-wrap:wrap;gap:10px}.budgets{display:flex;align-items:end;flex-wrap:wrap;gap:14px;margin-top:24px}label{font-size:12px}label input{display:block;margin:8px 0 12px}input{background:transparent;border:1px solid #8885;border-radius:8px;padding:10px;color:inherit;font-size:14px;min-width:0}.key-form{display:flex;gap:10px}.key-form input{flex:1}.key-row{padding:13px 0;border-bottom:1px solid #8882;font-size:13px}.key-row code{font-size:11px;opacity:.6;margin-left:10px}.secret,.banner{padding:14px 18px;border-radius:10px;background:#3478f610;font-size:13px;line-height:1.6;margin:18px 0}.secret code{display:block;overflow-wrap:anywhere;margin:12px 0}.error{background:#e5484d12;color:#c23940}.ledger{overflow:auto}table{border-collapse:collapse;width:100%;font-size:12px}td,th{text-align:left;padding:12px 8px;border-bottom:1px solid #8882}td:last-child,th:last-child{text-align:right}footer{border-top:1px solid #8883;padding-top:22px;margin-top:40px;font-size:12px;line-height:1.7;opacity:.7}
  @media(max-width:620px){.billing-page{padding:22px 16px}.heading,.current-plan{align-items:flex-start;flex-direction:column}.offers,.stats{grid-template-columns:1fr}.description{min-height:0}.budgets,.key-form{flex-direction:column;align-items:stretch}.budgets label input{width:100%;box-sizing:border-box}.key-row{align-items:flex-start}.key-row code{display:block;margin:6px 0}}
</style>
