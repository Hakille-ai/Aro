<script lang="ts">
  import Lock from "@lucide/svelte/icons/lock";
  import Plus from "@lucide/svelte/icons/plus";
  import Search from "@lucide/svelte/icons/search";
  import Shield from "@lucide/svelte/icons/shield";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import User from "@lucide/svelte/icons/user";

  type MaybeAsync = void | Promise<void>;
  type OrganizationRole = "admin" | "member";
  type MemberRole = "admin" | "manager" | "member" | "guest";

  interface OrganizationInfo {
    name: string;
    domain: string;
    description: string;
  }

  interface OrgMember {
    id: string;
    userId?: string;
    cloudId?: string;
    invitationId?: string;
    invitationStatus?: "pending" | "delivered" | "failed" | "expired" | "accepted" | "revoked";
    invitationExpiresAt?: string;
    invitationDeliveryStatus?: "pending" | "processing" | "delivered" | "failed";
    name: string;
    email: string;
    role: MemberRole;
    status: "active" | "invited";
  }

  interface OrgTeam {
    id: string;
    cloudId?: string;
    name: string;
    description: string;
    memberIds: string[];
  }

  interface UserProfile {
    avatarColor: string;
  }

  export let labels: Record<string, string>;
  export let language: "fr" | "en";
  export let currentOrganizationRole: OrganizationRole;
  export let activeOrg: OrganizationInfo;
  export let userProfile: UserProfile;
  export let orgMembers: OrgMember[];
  export let filteredMembers: OrgMember[];
  export let orgTeams: OrgTeam[];
  export let memberSearchQuery: string;
  export let showInviteModal: boolean;
  export let showCreateTeamModal: boolean;
  export let teamFormMembers: string[];
  export let writeLocked: boolean;
  export let writeDisabledTitle: (action?: string) => string | null | undefined;
  export let ensureWriteAllowed: (action: string) => boolean;
  export let getInitials: (name: string) => string;
  export let isCurrentMember: (member: OrgMember) => boolean;
  export let invitationStatusLabel: (member: OrgMember) => string;
  export let onSaveOrgInfo: () => MaybeAsync;
  export let onUpdateMemberRole: (memberId: string, role: MemberRole) => MaybeAsync;
  export let onRemoveMember: (memberId: string) => MaybeAsync;
  export let onDeleteTeam: (teamId: string) => MaybeAsync;
</script>

<div class="settings-tab-panel">
                <div class="current-role-badge-container">
                  <span class="current-role-label">{labels.currentRoleLabel}:</span>
                  <span
                    class="current-role-badge"
                    class:admin={currentOrganizationRole === "admin"}
                    class:member={currentOrganizationRole === "member"}
                  >
                    {#if currentOrganizationRole === "admin"}
                      <Shield size={10} style="margin-right: 3px;" />
                      {labels.roleAdminLabel}
                    {:else}
                      <User size={10} style="margin-right: 3px;" />
                      {labels.roleMemberLabel}
                    {/if}
                  </span>
                </div>

                <div class="panel-header">
                  <h2>{labels.orgTabTitle}</h2>
                  <p>{labels.orgTabDesc}</p>
                </div>

                <!-- Organization details card (locked if not Admin) -->
                <div class="org-details-card" class:locked-fields={currentOrganizationRole !== "admin"}>
                  {#if currentOrganizationRole !== "admin"}
                    <div class="lock-overlay">
                      <Lock size={16} />
                      <span>{labels.adminOnlyLocked}</span>
                    </div>
                  {/if}

                  <div class="settings-group">
                    <div class="option-row flex-column">
                      <label for="org-name" class="option-label">{labels.orgName}</label>
                      <input 
                        id="org-name"
                        type="text" 
                        class="settings-input" 
                        bind:value={activeOrg.name}
                        disabled={writeLocked || currentOrganizationRole !== "admin"}
                        on:input={onSaveOrgInfo}
                      />
                    </div>

                    <div class="option-row flex-column">
                      <label for="org-domain" class="option-label">{labels.orgDomain}</label>
                      <input 
                        id="org-domain"
                        type="text" 
                        class="settings-input" 
                        bind:value={activeOrg.domain}
                        disabled={writeLocked || currentOrganizationRole !== "admin"}
                        on:input={onSaveOrgInfo}
                      />
                    </div>

                    <div class="option-row flex-column">
                      <label for="org-desc" class="option-label">{labels.orgDesc}</label>
                      <textarea 
                        id="org-desc"
                        rows="2"
                        class="settings-textarea" 
                        bind:value={activeOrg.description}
                        disabled={writeLocked || currentOrganizationRole !== "admin"}
                        on:input={onSaveOrgInfo}
                      ></textarea>
                    </div>
                  </div>
                </div>

                <!-- Members Management Section -->
                <div class="org-members-card">
                  <div class="card-header-stats flex-row justify-between align-center" style="margin-bottom: 12px;">
                    <div>
                      <h3 class="panel-subtitle" style="margin-bottom: 2px;">{labels.membersTitle}</h3>
                    </div>
                    {#if currentOrganizationRole === "admin"}
                      <button 
                        type="button"
                        class="apple-btn primary"
                        style="padding: 8px 12px; font-size: 11px; height: 32px; display: flex; align-items: center; gap: 4px;"
                        disabled={writeLocked}
                        title={writeDisabledTitle("inviter un membre") ?? labels.inviteMemberBtn}
                        on:click={() => (showInviteModal = true)}
                      >
                        <Plus size={12} />
                        <span>{labels.inviteMemberBtn}</span>
                      </button>
                    {/if}
                  </div>

                  <!-- Member Search bar -->
                  <div class="member-search-container" style="margin-bottom: 15px;">
                    <div class="search-input-wrapper">
                      <Search size={14} class="search-icon" />
                      <input 
                        type="text" 
                        class="settings-input search-input" 
                        placeholder={labels.searchMembersPlaceholder}
                        bind:value={memberSearchQuery}
                      />
                    </div>
                  </div>

                  <!-- Members table -->
                  <div class="members-table-wrapper">
                    <table class="members-table">
                      <thead>
                        <tr>
                          <th>Collaborateur</th>
                          <th>E-mail</th>
                          <th>Rôle</th>
                          {#if currentOrganizationRole === "admin"}
                            <th style="width: 60px; text-align: center;">Actions</th>
                          {/if}
                        </tr>
                      </thead>
                      <tbody>
                        {#each filteredMembers as member}
                          <tr>
                            <td>
                              <div class="member-user-cell">
                                <div class="member-initials" style="background: {isCurrentMember(member) ? userProfile.avatarColor : '#e1e3e6'}">
                                  <span>{getInitials(member.name)}</span>
                                </div>
                                <div class="member-user-info">
                                  <span class="member-name-text">{member.name}</span>
                                  {#if member.status === "invited"}
                                    <span
                                      class="status-badge invited"
                                      title={member.invitationExpiresAt
                                        ? `${language === "fr" ? "Expire" : "Expires"}: ${new Date(member.invitationExpiresAt).toLocaleString()}`
                                        : undefined}
                                    >{invitationStatusLabel(member)}</span>
                                  {/if}
                                </div>
                              </div>
                            </td>
                            <td class="member-email-cell">{member.email}</td>
                            <td>
                              {#if currentOrganizationRole === "admin" && !isCurrentMember(member) && !member.invitationId}
                                <div class="select-role-wrapper">
                                  <select 
                                    class="role-select" 
                                    value={member.role}
                                    disabled={writeLocked}
                                    on:change={(e) => onUpdateMemberRole(member.id, (e.target as HTMLSelectElement).value as any)}
                                  >
                                    <option value="admin">{labels.roleAdminLabel}</option>
                                    <option value="manager">{labels.roleManagerLabel}</option>
                                    <option value="member">{labels.roleMemberLabel}</option>
                                    <option value="guest">{labels.roleGuestLabel}</option>
                                  </select>
                                </div>
                              {:else}
                                <span class="role-badge {member.role}">
                                  {#if member.role === "admin"}{labels.adminBadge}
                                  {:else}
                                    {member.role === "manager" ? labels.managerBadge : (member.role === "member" ? labels.memberBadge : labels.guestBadge)}
                                  {/if}
                                </span>
                              {/if}
                            </td>
                            {#if currentOrganizationRole === "admin"}
                              <td style="text-align: center;">
                                {#if !isCurrentMember(member)}
                                  <button 
                                    type="button"
                                    class="member-delete-btn" 
                                    disabled={writeLocked}
                                    title={writeDisabledTitle("supprimer un membre") ?? "Retirer"}
                                    on:click={() => onRemoveMember(member.id)}
                                  >
                                    <Trash2 size={12} />
                                  </button>
                                {/if}
                              </td>
                            {/if}
                          </tr>
                        {/each}
                      </tbody>
                    </table>
                  </div>
                </div>

                <!-- Teams Management Section -->
                <div class="org-teams-card">
                  <div class="card-header-stats flex-row justify-between align-center" style="margin-bottom: 12px;">
                    <div>
                      <h3 class="panel-subtitle" style="margin-bottom: 2px;">{labels.teamsTitle}</h3>
                    </div>
                    {#if currentOrganizationRole === "admin"}
                      <button 
                        type="button"
                        class="apple-btn secondary"
                        style="padding: 8px 12px; font-size: 11px; height: 32px; display: flex; align-items: center; gap: 4px;"
                        disabled={writeLocked}
                        title={writeDisabledTitle("creer une equipe") ?? labels.createTeamBtn}
                        on:click={() => {
                          if (!ensureWriteAllowed("creer une equipe")) return;
                          showCreateTeamModal = true;
                          teamFormMembers = [];
                        }}
                      >
                        <Plus size={12} />
                        <span>{labels.createTeamBtn}</span>
                      </button>
                    {/if}
                  </div>

                  <div class="teams-grid">
                    {#each orgTeams as team}
                      <div class="team-card">
                        <div class="team-card-header">
                          <h4>{team.name}</h4>
                          {#if currentOrganizationRole === "admin"}
                            <button 
                              type="button"
                              class="team-delete-btn"
                              disabled={writeLocked}
                              title={writeDisabledTitle("supprimer une equipe") ?? "Delete Team"}
                              on:click={() => onDeleteTeam(team.id)}
                            >
                              <Trash2 size={12} />
                            </button>
                          {/if}
                        </div>
                        <p class="team-desc">{team.description}</p>
                        
                        <!-- Member avatars in the team -->
                        <div class="team-members-row">
                          <div class="team-avatars-stack">
                            {#each team.memberIds as mId}
                              {@const member = orgMembers.find(m => m.id === mId)}
                                {#if member}
                                  <div 
                                    class="team-member-avatar" 
                                    style="background: {mId === '1' ? userProfile.avatarColor : '#e1e3e6'}"
                                    title={member.name}
                                  >
                                    {getInitials(member.name)}
                                  </div>
                                {/if}
                              {/each}
                            </div>
                            <span class="team-count-text">{team.memberIds.length} membre{team.memberIds.length > 1 ? 's' : ''}</span>
                        </div>
                      </div>
                    {/each}
                    {#if orgTeams.length === 0}
                      <div class="no-teams-hint">{labels.noTeamsYet}</div>
                    {/if}
                  </div>
                </div>
              </div>
