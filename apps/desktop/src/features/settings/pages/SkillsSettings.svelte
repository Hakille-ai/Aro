<script lang="ts">
  import { onMount } from "svelte";
  import Check from "@lucide/svelte/icons/check";
  import Edit2 from "@lucide/svelte/icons/edit-2";
  import Loader2 from "@lucide/svelte/icons/loader-2";
  import Play from "@lucide/svelte/icons/play";
  import Plus from "@lucide/svelte/icons/plus";
  import Puzzle from "@lucide/svelte/icons/puzzle";
  import Search from "@lucide/svelte/icons/search";
  import Terminal from "@lucide/svelte/icons/terminal";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import X from "@lucide/svelte/icons/x";

  import type { UserSkill } from "../../skills/model";
  import {
    listInstalledPlugins,
    pluginSkillsToUserSkills,
    togglePlugin,
    invokePluginSkill,
  } from "../../../lib/api/plugins";

  type MaybeAsync = void | Promise<void>;

  export let theme: "light" | "dark" = "light";
  export let skillSearchQuery: string = "";
  export let selectedCategoryFilter: string = "Tous";
  export let filteredSkills: UserSkill[] = [];
  export let uniqueCategories: string[] = [];
  export let onOpenSkillForm: (skill: UserSkill | null) => MaybeAsync;
  export let onToggleSkill: (skillId: string) => MaybeAsync;
  export let onDeleteSkill: (skillId: string) => MaybeAsync;

  // Local state for standalone plugin skills loading fallback & testing
  let localPluginSkills: UserSkill[] = [];
  let isTestingSkillId: string | null = null;
  let testResult: { skillId: string; success: boolean; message: string } | null = null;

  $: isDark = theme === "dark";

  // Combine passed skills with local plugin skills fallback if App.svelte hasn't populated yet
  $: allSkillsList = (() => {
    const existingIds = new Set(filteredSkills.map((s) => s.id));
    const merged = [...filteredSkills];
    for (const ps of localPluginSkills) {
      if (!existingIds.has(ps.id)) {
        merged.push(ps);
      }
    }
    return merged;
  })();

  // Filter skills according to search and category
  $: displayedSkills = allSkillsList.filter((s) => {
    const query = (skillSearchQuery || "").toLowerCase().trim();
    const matchSearch =
      !query ||
      (s.name || "").toLowerCase().includes(query) ||
      (s.description || "").toLowerCase().includes(query) ||
      (s.pluginName || "").toLowerCase().includes(query) ||
      (s.tags || []).some((t) => t.toLowerCase().includes(query));

    const matchCategory =
      selectedCategoryFilter === "Tous" ||
      (selectedCategoryFilter === "Plugins" && s.isPlugin) ||
      s.category === selectedCategoryFilter;

    return matchSearch && matchCategory;
  });

  // Unique categories for filtering pills
  $: dynamicCategories = (() => {
    const cats = new Set<string>();
    for (const s of allSkillsList) {
      if (s.category && s.category !== "Plugins" && s.category !== "Tous") {
        cats.add(s.category);
      }
    }
    for (const c of uniqueCategories) {
      if (c !== "Tous" && c !== "Plugins") {
        cats.add(c);
      }
    }
    const result = ["Tous"];
    if (allSkillsList.some((s) => s.isPlugin)) {
      result.push("Plugins");
    }
    return [...result, ...Array.from(cats)];
  })();

  // Group displayed skills by category for organized rendering
  $: activeGroupCategories = (() => {
    if (selectedCategoryFilter !== "Tous") {
      return [selectedCategoryFilter];
    }
    const set = new Set<string>();
    for (const s of displayedSkills) {
      if (s.isPlugin) {
        set.add("Plugins");
      } else {
        set.add(s.category || "Général");
      }
    }
    const list = Array.from(set);
    if (list.includes("Plugins")) {
      return ["Plugins", ...list.filter((c) => c !== "Plugins")];
    }
    return list;
  })();

  $: totalCount = allSkillsList.length;
  $: pluginsCount = allSkillsList.filter((s) => s.isPlugin).length;
  $: customCount = allSkillsList.filter((s) => !s.isPlugin).length;

  function countForCategory(cat: string): number {
    if (cat === "Tous") return allSkillsList.length;
    if (cat === "Plugins") return allSkillsList.filter((s) => s.isPlugin).length;
    return allSkillsList.filter((s) => !s.isPlugin && (s.category || "Général") === cat).length;
  }

  async function handleToggle(skill: UserSkill) {
    if (skill.isPlugin && skill.pluginId) {
      const nextEnabled = !skill.enabled;
      skill.enabled = nextEnabled;
      allSkillsList = [...allSkillsList];
      try {
        await togglePlugin(skill.pluginId, nextEnabled);
      } catch (err) {
        console.warn("Error toggling plugin skill:", err);
      }
    }
    await onToggleSkill(skill.id);
  }

  async function handleTestSkill(skill: UserSkill) {
    if (!skill.pluginId) return;
    const cleanSkillId = skill.id.replace(/^plugin:[^:]+:/, "");
    isTestingSkillId = skill.id;
    testResult = null;

    try {
      const res = await invokePluginSkill(skill.pluginId, cleanSkillId, {
        prompt: "Verification test invocation from ARO Settings UI",
      });
      testResult = {
        skillId: skill.id,
        success: true,
        message:
          res?.result ||
          res?.message ||
          `Compétence '${skill.name}' opérationnelle avec succès.`,
      };
    } catch (err: any) {
      testResult = {
        skillId: skill.id,
        success: false,
        message: err?.message || "Le test d'invocation a échoué.",
      };
    } finally {
      isTestingSkillId = null;
      setTimeout(() => {
        if (testResult?.skillId === skill.id) {
          testResult = null;
        }
      }, 5000);
    }
  }

  onMount(async () => {
    try {
      const plugins = await listInstalledPlugins();
      localPluginSkills = pluginSkillsToUserSkills(plugins);
    } catch (err) {
      console.warn("Failed to load plugin skills fallback:", err);
    }
  });
</script>

<div class="settings-tab-panel">
  <!-- Header -->
  <div class="panel-header" style="display: flex; justify-content: space-between; align-items: flex-start; width: 100%;">
    <div>
      <div style="display: flex; align-items: center; gap: 10px; margin-bottom: 4px;">
        <h2 style="margin: 0; font-size: 22px; font-weight: 600; letter-spacing: -0.5px; color: {isDark ? '#ffffff' : '#1d1d1f'};">
          Skills
        </h2>
        {#if pluginsCount > 0}
          <span
            style="font-size: 11px; font-weight: 600; padding: 2px 8px; border-radius: 980px; background: rgba(0, 113, 227, 0.1); color: #0071e3; border: 1px solid rgba(0, 113, 227, 0.2); display: inline-flex; align-items: center; gap: 4px;"
          >
            <Puzzle size={11} />
            {pluginsCount} via Plugins
          </span>
        {/if}
      </div>
      <p style="margin: 0; font-size: 13px; color: #86868b; line-height: 1.4; max-width: 680px;">
        Étendez les capacités de l'assistant via des compétences spécialisées issues d'Agent Plugins ou de créations locales personnalisées.
      </p>
    </div>
    <button
      class="primary-btn"
      style="background: linear-gradient(180deg, #2997ff 0%, #0071e3 100%); color: white; border: none; border-radius: 980px; padding: 8px 16px; font-size: 13px; font-weight: 500; cursor: pointer; box-shadow: 0 4px 12px rgba(0, 113, 227, 0.2); display: flex; align-items: center; gap: 6px; white-space: nowrap; flex-shrink: 0; transition: all 0.2s ease;"
      type="button"
      on:click={() => onOpenSkillForm(null)}
    >
      <Plus size={15} />
      <span>Créer un Skill</span>
    </button>
  </div>

  <!-- Search & Filter Controls -->
  <div class="skills-filter-container" style="margin-top: 24px; margin-bottom: 24px; display: flex; flex-direction: column; gap: 14px;">
    <!-- Search Bar -->
    <div class="settings-search-wrapper" style="position: relative; display: flex; align-items: center; width: 100%; max-width: 420px;">
      <Search size={14} style="position: absolute; left: 12px; color: #86868b;" />
      <input
        type="text"
        bind:value={skillSearchQuery}
        placeholder="Rechercher des compétences, plugins ou tags..."
        style="width: 100%; padding: 10px 32px 10px 34px; border-radius: 10px; border: 1px solid {isDark ? 'rgba(255,255,255,0.08)' : 'rgba(0,0,0,0.1)'}; background: {isDark ? 'rgba(255,255,255,0.04)' : 'rgba(0,0,0,0.03)'}; color: {isDark ? '#ffffff' : '#1d1d1f'}; font-size: 13px; outline: none; transition: all 0.2s ease;"
      />
      {#if skillSearchQuery}
        <button
          type="button"
          on:click={() => (skillSearchQuery = "")}
          style="position: absolute; right: 10px; background: none; border: none; color: #86868b; cursor: pointer; padding: 2px;"
        >
          <X size={13} />
        </button>
      {/if}
    </div>

    <!-- Category Pill Filters with Counts -->
    <div class="category-filters" style="display: flex; flex-wrap: wrap; gap: 8px;">
      {#each dynamicCategories as cat}
        {@const count = countForCategory(cat)}
        <button
          type="button"
          on:click={() => (selectedCategoryFilter = cat)}
          style="border: none; border-radius: 20px; padding: 6px 14px; font-size: 12px; font-weight: 500; cursor: pointer; transition: all 0.2s; outline: none; display: flex; align-items: center; gap: 6px;
            background: {selectedCategoryFilter === cat ? '#0071e3' : (isDark ? 'rgba(255,255,255,0.06)' : 'rgba(0,0,0,0.05)')};
            color: {selectedCategoryFilter === cat ? '#ffffff' : (isDark ? '#a1a1a6' : '#1d1d1f')};
            box-shadow: {selectedCategoryFilter === cat ? '0 2px 8px rgba(0, 113, 227, 0.3)' : 'none'};"
        >
          {#if cat === "Plugins"}
            <Puzzle size={12} />
          {/if}
          <span>{cat}</span>
          <span
            style="font-size: 10px; padding: 1px 6px; border-radius: 980px; font-weight: 600;
              background: {selectedCategoryFilter === cat ? 'rgba(255,255,255,0.2)' : (isDark ? 'rgba(255,255,255,0.08)' : 'rgba(0,0,0,0.06)')};
              color: {selectedCategoryFilter === cat ? '#ffffff' : '#86868b'};"
          >
            {count}
          </span>
        </button>
      {/each}
    </div>
  </div>

  <!-- Skills Grid / Categorized View -->
  {#if displayedSkills.length > 0}
    <div style="display: flex; flex-direction: column; gap: 32px;">
      {#each activeGroupCategories as cat}
        {@const groupSkills = displayedSkills.filter((s) => (cat === "Plugins" ? s.isPlugin : (s.category || "Général") === cat))}
        {#if groupSkills.length > 0}
          <div class="category-group-section">
            <!-- Group Header -->
            <div style="display: flex; align-items: center; gap: 8px; margin-bottom: 14px;">
              <h3 style="margin: 0; font-size: 11px; font-weight: 600; color: #86868b; text-transform: uppercase; letter-spacing: 1.2px; display: flex; align-items: center; gap: 6px;">
                {#if cat === "Plugins"}
                  <Puzzle size={12} style="color: #0071e3;" />
                {/if}
                <span>{cat === "Plugins" ? "Skills issus des Agent Plugins" : cat}</span>
              </h3>
              <span style="font-size: 10px; padding: 1px 6px; border-radius: 8px; background: {isDark ? 'rgba(255,255,255,0.06)' : 'rgba(0,0,0,0.05)'}; color: #86868b; font-weight: 600;">
                {groupSkills.length}
              </span>
            </div>

            <!-- Grid of Cards -->
            <div class="skills-grid" style="display: grid; grid-template-columns: repeat(auto-fill, minmax(300px, 1fr)); gap: 16px;">
              {#each groupSkills as skill (skill.id)}
                <div
                  class="skill-card"
                  style="display: flex; flex-direction: column; justify-content: space-between; padding: 18px; border-radius: 14px; border: 1px solid {isDark ? 'rgba(255,255,255,0.08)' : 'rgba(0,0,0,0.08)'}; background: {isDark ? '#242426' : '#ffffff'}; box-shadow: 0 2px 10px rgba(0,0,0,0.03); transition: all 0.2s;"
                >
                  <div>
                    <!-- Header of Card -->
                    <div style="display: flex; align-items: flex-start; justify-content: space-between; gap: 12px; margin-bottom: 10px;">
                      <div style="display: flex; align-items: center; gap: 12px;">
                        <div style="width: 40px; height: 40px; border-radius: 10px; background: {isDark ? '#2c2c2e' : '#f5f5f7'}; border: 1px solid {isDark ? 'rgba(255,255,255,0.06)' : 'rgba(0,0,0,0.06)'}; display: flex; align-items: center; justify-content: center; font-size: 20px; flex-shrink: 0; box-shadow: inset 0 1px 0 rgba(255,255,255,0.05);">
                          {skill.icon || '🧩'}
                        </div>
                        <div>
                          <h4 style="margin: 0; font-size: 15px; font-weight: 600; color: {isDark ? '#ffffff' : '#1d1d1f'}; line-height: 1.3;">
                            {skill.name}
                          </h4>
                          {#if skill.isPlugin && skill.pluginName}
                            <span style="font-size: 10px; font-weight: 600; color: #0071e3; background: rgba(0, 113, 227, 0.08); padding: 1px 6px; border-radius: 5px; display: inline-flex; align-items: center; gap: 3px; margin-top: 3px;">
                              <Puzzle size={10} />
                              {skill.pluginName}
                            </span>
                          {/if}
                        </div>
                      </div>
                    </div>

                    <!-- Description -->
                    <p style="margin: 0 0 12px 0; font-size: 13px; line-height: 1.45; color: #86868b; min-height: 38px; display: -webkit-box; -webkit-line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden;">
                      {skill.description}
                    </p>

                    <!-- Tags -->
                    {#if skill.tags && skill.tags.length > 0}
                      <div style="display: flex; flex-wrap: wrap; gap: 4px; margin-bottom: 14px;">
                        {#each skill.tags as tag}
                          <span style="font-size: 10px; padding: 2px 6px; border-radius: 4px; background: {isDark ? 'rgba(255,255,255,0.05)' : 'rgba(0,0,0,0.04)'}; color: #86868b; font-family: monospace;">
                            #{tag}
                          </span>
                        {/each}
                      </div>
                    {/if}

                    <!-- Test Result Notification if active -->
                    {#if testResult && testResult.skillId === skill.id}
                      <div
                        style="padding: 8px 10px; border-radius: 8px; font-size: 11px; margin-bottom: 12px; display: flex; align-items: center; gap: 6px;
                          background: {testResult.success ? 'rgba(52, 199, 89, 0.1)' : 'rgba(255, 69, 58, 0.1)'};
                          color: {testResult.success ? '#34c759' : '#ff453a'}; border: 1px solid {testResult.success ? 'rgba(52, 199, 89, 0.2)' : 'rgba(255, 69, 58, 0.2)'};"
                      >
                        {#if testResult.success}
                          <Check size={12} />
                        {:else}
                          <X size={12} />
                        {/if}
                        <span style="overflow: hidden; text-overflow: ellipsis; white-space: nowrap;">{testResult.message}</span>
                      </div>
                    {/if}
                  </div>

                  <!-- Footer: Toggle & Actions -->
                  <div style="display: flex; align-items: center; justify-content: space-between; border-top: 1px solid {isDark ? 'rgba(255,255,255,0.05)' : 'rgba(0,0,0,0.06)'}; padding-top: 14px; margin-top: auto;">
                    <!-- iOS Switch Toggle -->
                    <label class="switch-toggle" style="display: flex; align-items: center; gap: 8px; cursor: pointer; user-select: none;">
                      <input
                        type="checkbox"
                        checked={skill.enabled}
                        on:change={() => handleToggle(skill)}
                        style="display: none;"
                      />
                      <div
                        class="switch-slider"
                        style="width: 36px; height: 20px; border-radius: 10px; background: {skill.enabled ? '#34c759' : (isDark ? '#3a3a3c' : '#e5e5ea')}; position: relative; transition: all 0.2s ease;"
                      >
                        <div
                          class="switch-knob"
                          style="width: 16px; height: 16px; border-radius: 50%; background: white; position: absolute; top: 2px; left: {skill.enabled ? '18px' : '2px'}; transition: all 0.2s ease; box-shadow: 0 1px 3px rgba(0,0,0,0.25);"
                        ></div>
                      </div>
                      <span style="font-size: 12px; font-weight: 500; color: {skill.enabled ? (isDark ? '#f5f5f7' : '#1d1d1f') : '#86868b'};">
                        {skill.enabled ? 'Actif' : 'Inactif'}
                      </span>
                    </label>

                    <!-- Actions -->
                    <div style="display: flex; align-items: center; gap: 8px;">
                      {#if skill.isPlugin}
                        <button
                          type="button"
                          disabled={isTestingSkillId === skill.id}
                          on:click={() => handleTestSkill(skill)}
                          style="background: {isDark ? 'rgba(255,255,255,0.06)' : 'rgba(0,0,0,0.04)'}; border: 1px solid {isDark ? 'rgba(255,255,255,0.1)' : 'rgba(0,0,0,0.08)'}; color: {isDark ? '#ffffff' : '#1d1d1f'}; border-radius: 6px; padding: 4px 8px; font-size: 11px; font-weight: 500; cursor: pointer; display: flex; align-items: center; gap: 4px; transition: all 0.2s;"
                          title="Tester l'exécution du skill"
                        >
                          {#if isTestingSkillId === skill.id}
                            <Loader2 size={11} class="animate-spin" />
                            <span>Test...</span>
                          {:else}
                            <Play size={11} style="color: #0071e3;" />
                            <span>Tester</span>
                          {/if}
                        </button>
                      {:else}
                        <button
                          type="button"
                          on:click={() => onOpenSkillForm(skill)}
                          style="background: none; border: none; padding: 4px; color: #86868b; cursor: pointer; display: flex; align-items: center; justify-content: center; border-radius: 6px;"
                          title="Modifier"
                        >
                          <Edit2 size={14} />
                        </button>
                        <button
                          type="button"
                          on:click={() => onDeleteSkill(skill.id)}
                          style="background: none; border: none; padding: 4px; color: #ff453a; cursor: pointer; display: flex; align-items: center; justify-content: center; border-radius: 6px;"
                          title="Supprimer"
                        >
                          <Trash2 size={14} />
                        </button>
                      {/if}
                    </div>
                  </div>
                </div>
              {/each}
            </div>
          </div>
        {/if}
      {/each}
    </div>
  {:else}
    <!-- Empty State -->
    <div style="text-align: center; padding: 60px 20px; background: {isDark ? 'rgba(255,255,255,0.01)' : 'rgba(0,0,0,0.01)'}; border: 1px dashed {isDark ? 'rgba(255,255,255,0.08)' : 'rgba(0,0,0,0.08)'}; border-radius: 14px; margin-top: 10px;">
      <Terminal size={32} style="color: #86868b; margin-bottom: 12px; display: inline-block;" />
      <h4 style="margin: 0 0 4px; font-size: 15px; font-weight: 600; color: {isDark ? '#ffffff' : '#1d1d1f'};">
        {skillSearchQuery ? 'Aucun résultat pour cette recherche' : 'Aucun skill trouvé'}
      </h4>
      <p style="margin: 0; font-size: 13px; color: #86868b; max-width: 420px; margin: 0 auto 16px;">
        {skillSearchQuery ? 'Essayez un autre mot-clé ou réinitialisez le filtre.' : 'Installez un plugin depuis la bibliothèque ou créez un skill local.'}
      </p>
      {#if skillSearchQuery}
        <button
          type="button"
          on:click={() => (skillSearchQuery = "")}
          style="background: none; border: 1px solid #0071e3; color: #0071e3; border-radius: 980px; padding: 6px 16px; font-size: 12px; font-weight: 500; cursor: pointer;"
        >
          Effacer la recherche
        </button>
      {/if}
    </div>
  {/if}
</div>
