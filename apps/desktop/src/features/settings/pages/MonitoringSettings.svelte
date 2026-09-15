<script lang="ts">
  interface PerformanceRecord {
    id: string;
    timestamp: string;
    conversationTitle: string;
    responseTime: number;
    tokens: number;
    speed: number;
  }

  interface ActivityDay {
    dateStr: string;
    count: number;
    level: number;
    dayName: string;
    dateFormatted: string;
    monthLabel: string;
  }

  export let labels: Record<string, string>;
  export let language: "fr" | "en";
  export let monCpu: number;
  export let monRam: number;
  export let monRamMax: number;
  export let monGpu: number;
  export let monGpuVram: number;
  export let monGpuVramMax: number;
  export let cpuHistory: number[];
  export let ramHistory: number[];
  export let gpuHistory: number[];
  export let lastResponseTime: number;
  export let avgResponseTime: number;
  export let lastTokenCount: number;
  export let lastTokenSpeed: number;
  export let totalTokensGenerated: number;
  export let performanceHistory: PerformanceRecord[];
  export let activityWeeks: ActivityDay[][];
</script>

<div class="settings-tab-panel">
                <div class="panel-header">
                  <h2>{labels.monitoringTabTitle}</h2>
                  <p>{labels.monitoringTabDesc}</p>
                </div>

                <!-- Hardware Resource Gauges Grid -->
                <div class="dashboard-grid">
                  <!-- CPU Card -->
                  <div class="dashboard-card">
                    <div class="card-header-stats">
                      <span class="card-title-stats">{labels.cpuUsage}</span>
                      <span class="card-value-stats text-primary">{monCpu}%</span>
                    </div>
                    <div class="gauge-and-chart">
                      <div class="gauge-container">
                        <svg viewBox="0 0 36 36" class="circular-chart blue-ring">
                          <path class="circle-bg"
                            d="M18 2.0845
                              a 15.9155 15.9155 0 0 1 0 31.831
                              a 15.9155 15.9155 0 0 1 0 -31.831"
                          />
                          <path class="circle"
                            stroke-dasharray="{monCpu}, 100"
                            d="M18 2.0845
                              a 15.9155 15.9155 0 0 1 0 31.831
                              a 15.9155 15.9155 0 0 1 0 -31.831"
                          />
                        </svg>
                      </div>
                      <div class="sparkline-container">
                        <svg class="sparkline" viewBox="0 0 100 30" preserveAspectRatio="none">
                          <path
                            d={cpuHistory.reduce((acc, val, i) => `${acc} ${i === 0 ? 'M' : 'L'} ${(i * 100) / 19} ${30 - (val * 30) / 100}`, '')}
                            fill="none"
                            stroke="#0a84ff"
                            stroke-width="1.5"
                          />
                        </svg>
                      </div>
                    </div>
                  </div>

                  <!-- RAM Card -->
                  <div class="dashboard-card">
                    <div class="card-header-stats">
                      <span class="card-title-stats">{labels.ramUsage}</span>
                      <span class="card-value-stats text-success">{monRam} GB <small style="opacity: 0.6;">/ {monRamMax} GB</small></span>
                    </div>
                    <div class="gauge-and-chart">
                      <div class="gauge-container">
                        <svg viewBox="0 0 36 36" class="circular-chart green-ring">
                          <path class="circle-bg"
                            d="M18 2.0845
                              a 15.9155 15.9155 0 0 1 0 31.831
                              a 15.9155 15.9155 0 0 1 0 -31.831"
                          />
                          <path class="circle"
                            stroke-dasharray="{(monRam / monRamMax) * 100}, 100"
                            d="M18 2.0845
                              a 15.9155 15.9155 0 0 1 0 31.831
                              a 15.9155 15.9155 0 0 1 0 -31.831"
                          />
                        </svg>
                      </div>
                      <div class="sparkline-container">
                        <svg class="sparkline" viewBox="0 0 100 30" preserveAspectRatio="none">
                          <path
                            d={ramHistory.reduce((acc, val, i) => `${acc} ${i === 0 ? 'M' : 'L'} ${(i * 100) / 19} ${30 - ((val - 3.8) / 6.0) * 30}`, '')}
                            fill="none"
                            stroke="#34a853"
                            stroke-width="1.5"
                          />
                        </svg>
                      </div>
                    </div>
                  </div>

                  <!-- GPU Card -->
                  <div class="dashboard-card">
                    <div class="card-header-stats">
                      <span class="card-title-stats">{labels.gpuUsage}</span>
                      <span class="card-value-stats text-purple">{monGpu}% <small style="opacity: 0.6;">({monGpuVram} / {monGpuVramMax} GB)</small></span>
                    </div>
                    <div class="gauge-and-chart">
                      <div class="gauge-container">
                        <svg viewBox="0 0 36 36" class="circular-chart purple-ring">
                          <path class="circle-bg"
                            d="M18 2.0845
                              a 15.9155 15.9155 0 0 1 0 31.831
                              a 15.9155 15.9155 0 0 1 0 -31.831"
                          />
                          <path class="circle"
                            stroke-dasharray="{monGpu}, 100"
                            d="M18 2.0845
                              a 15.9155 15.9155 0 0 1 0 31.831
                              a 15.9155 15.9155 0 0 1 0 -31.831"
                          />
                        </svg>
                      </div>
                      <div class="sparkline-container">
                        <svg class="sparkline" viewBox="0 0 100 30" preserveAspectRatio="none">
                          <path
                            d={gpuHistory.reduce((acc, val, i) => `${acc} ${i === 0 ? 'M' : 'L'} ${(i * 100) / 19} ${30 - (val * 30) / 100}`, '')}
                            fill="none"
                            stroke="#bf5af2"
                            stroke-width="1.5"
                          />
                        </svg>
                      </div>
                    </div>
                  </div>
                </div>

                <!-- Last Query Metrics -->
                <div class="metrics-grid">
                  <div class="metric-card">
                    <span class="metric-label">{labels.responseTime} ({labels.lastInference})</span>
                    <div class="metric-value-container">
                      <span class="metric-val">
                        {#if lastResponseTime > 0}
                          {lastResponseTime}<small> {labels.secAbbr}</small>
                        {:else}
                          --
                        {/if}
                      </span>
                      <span class="metric-sub">{labels.avgResponseTime}: {avgResponseTime > 0 ? `${avgResponseTime} ${labels.secAbbr}` : "--"}</span>
                    </div>
                  </div>

                  <div class="metric-card">
                    <span class="metric-label">{labels.tokensTitle} ({labels.lastInference})</span>
                    <div class="metric-value-container">
                      <span class="metric-val">
                        {#if lastTokenSpeed > 0}
                          {lastTokenSpeed}<small> {labels.tokensPerSec}</small>
                        {:else}
                          --
                        {/if}
                      </span>
                      <span class="metric-sub">{labels.totalTokens}: {totalTokensGenerated} ({lastTokenCount} labels)</span>
                    </div>
                  </div>
                </div>

                <!-- Inference Speed Trend Chart -->
                {#if performanceHistory.length > 1}
                  <div class="performance-chart-card">
                    <h3 class="panel-subtitle">{labels.tokensTitle} ({labels.tokensPerSec}) — Trend</h3>
                    <div class="trend-chart-container">
                      <svg class="trend-chart" viewBox="0 0 500 120" preserveAspectRatio="none">
                        <line x1="0" y1="20" x2="500" y2="20" stroke="rgba(255,255,255,0.05)" stroke-dasharray="3,3" />
                        <line x1="0" y1="60" x2="500" y2="60" stroke="rgba(255,255,255,0.05)" stroke-dasharray="3,3" />
                        <line x1="0" y1="100" x2="500" y2="100" stroke="rgba(255,255,255,0.05)" stroke-dasharray="3,3" />
                        
                        <path
                          d="M 0 120
                             {performanceHistory.slice().reverse().reduce((acc, r, i) => {
                               const maxSpeed = Math.max(...performanceHistory.map(ph => ph.speed), 10);
                               const x = (i * 500) / (performanceHistory.length - 1);
                               const y = 110 - (r.speed * 90) / maxSpeed;
                               return `${acc} L ${x} ${y}`;
                             }, '')}
                             L 500 120 Z"
                          fill="url(#trendGrad)"
                          opacity="0.15"
                        />
                        
                        <path
                          d={performanceHistory.slice().reverse().reduce((acc, r, i) => {
                            const maxSpeed = Math.max(...performanceHistory.map(ph => ph.speed), 10);
                            const x = (i * 500) / (performanceHistory.length - 1);
                            const y = 110 - (r.speed * 90) / maxSpeed;
                            return `${acc} ${i === 0 ? 'M' : 'L'} ${x} ${y}`;
                          }, '')}
                          fill="none"
                          stroke="#00f2fe"
                          stroke-width="2"
                        />

                        <defs>
                          <linearGradient id="trendGrad" x1="0%" y1="0%" x2="0%" y2="100%">
                            <stop offset="0%" stop-color="#00f2fe" />
                            <stop offset="100%" stop-color="#0071e3" stop-opacity="0" />
                          </linearGradient>
                        </defs>
                      </svg>
                    </div>
                  </div>
                {/if}

                <!-- User Activity Grid (GitHub style) -->
                <div class="activity-card">
                  <h3 class="panel-subtitle">{labels.activityTitle}</h3>
                  <p class="panel-desc-sub" style="font-size: 11px; color: #86868b; margin-top: -6px; margin-bottom: 15px;">
                    {labels.activitySubtitle}
                  </p>
                  <div class="activity-graph-wrapper">
                    <div class="month-labels">
                      {#each activityWeeks as week, wIdx}
                        {#if week[0].monthLabel}
                          <span class="month-label" style="grid-column: {wIdx + 1};">{week[0].monthLabel}</span>
                        {/if}
                      {/each}
                    </div>
                    
                    <div class="activity-grid-container">
                      <div class="day-labels">
                        <span></span>
                        <span>{language === 'fr' ? 'Lun' : 'Mon'}</span>
                        <span></span>
                        <span>{language === 'fr' ? 'Mer' : 'Wed'}</span>
                        <span></span>
                        <span>{language === 'fr' ? 'Ven' : 'Fri'}</span>
                        <span></span>
                      </div>
                      
                      <div class="activity-grid">
                        {#each activityWeeks as week}
                          <div class="activity-column">
                            {#each week as day}
                              <div 
                                class="activity-cell level-{day.level}"
                                title="{day.count} {day.count === 1 ? (language === 'fr' ? 'requête le' : 'prompt on') : (language === 'fr' ? 'requêtes le' : 'prompts on')} {day.dateFormatted}"
                              ></div>
                            {/each}
                          </div>
                        {/each}
                      </div>
                    </div>
                    
                    <div class="activity-legend">
                      <span>{language === 'fr' ? 'Moins' : 'Less'}</span>
                      <div class="legend-cells">
                        <div class="activity-cell level-0"></div>
                        <div class="activity-cell level-1"></div>
                        <div class="activity-cell level-2"></div>
                        <div class="activity-cell level-3"></div>
                        <div class="activity-cell level-4"></div>
                      </div>
                      <span>{language === 'fr' ? 'Plus' : 'More'}</span>
                    </div>
                  </div>
                </div>

                <!-- Performance History Log -->
                <div class="history-card">
                  <h3 class="panel-subtitle">{labels.perfHistory}</h3>
                  {#if performanceHistory.length === 0}
                    <div class="no-data-hint">{labels.noPerfData}</div>
                  {:else}
                    <div class="perf-table-wrapper">
                      <table class="perf-table">
                        <thead>
                          <tr>
                            <th>Heure / Time</th>
                            <th>Chat / Conversation</th>
                            <th>Temps / Latency</th>
                            <th>Tokens</th>
                            <th>Vitesse / Speed</th>
                          </tr>
                        </thead>
                        <tbody>
                          {#each performanceHistory as record}
                            <tr>
                              <td>{record.timestamp}</td>
                              <td class="col-title">{record.conversationTitle}</td>
                              <td>{record.responseTime} {labels.secAbbr}</td>
                              <td>{record.tokens}</td>
                              <td class="text-primary font-semibold">{record.speed} {labels.tokensPerSec}</td>
                            </tr>
                          {/each}
                        </tbody>
                      </table>
                    </div>
                  {/if}
                </div>
              </div>

