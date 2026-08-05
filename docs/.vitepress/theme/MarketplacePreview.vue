<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'

type PluginKind = 'dynamic' | 'static'
type FilterKey = 'all' | PluginKind

interface DriverInfo {
  driver: string
  scenes?: string[]
  outbound?: string[]
}

interface MarketplacePlugin {
  id: string
  name: string
  summary: string
  description: string
  kind: PluginKind
  repository: string
  repositoryUrl: string
  license: string
  authors: string[]
  categories: string[]
  keywords: string[]
  trust: string
  demo?: boolean
  github?: {
    stars?: number
    forks?: number
    openIssues?: number
    language?: string
    updatedAt?: string
  }
  latest: {
    version: string
    releasedAt: string
    channel: 'stable' | 'prerelease'
    qimenbot: string
    dynamicApi?: string
    changelog: string
    drivers: DriverInfo[]
    assets: Array<{
      target: string
      sizeBytes?: number
      minGlibc?: string
      githubAttestation?: boolean
    }>
  }
  versions: number
}

const activeFilter = ref<FilterKey>('all')
const searchTerm = ref('')
const currentPage = ref(1)
const pageSize = ref(6)
const selectedPlugin = ref<MarketplacePlugin | null>(null)
const catalogState = ref<'preview' | 'loading' | 'ready' | 'empty' | 'error'>('preview')
const livePlugins = ref<MarketplacePlugin[]>([])

const demoPlugins: MarketplacePlugin[] = [
  {
    id: 'message-toolkit',
    name: '消息工具箱',
    summary: '把常用的消息构建、回复和富媒体能力整理成一组可复用命令。',
    description: '用于预览商城卡片的信息层级。正式页面会替换为商城索引中的真实描述。',
    kind: 'dynamic',
    repository: 'lvyunqi/QimenBot',
    repositoryUrl: 'https://github.com/lvyunqi/QimenBot',
    license: 'MIT',
    authors: ['QimenBot Contributors'],
    categories: ['消息处理', '实用工具'],
    keywords: ['rich-message', 'reply'],
    trust: 'community',
    demo: true,
    github: { language: 'Rust' },
    latest: {
      version: '0.6.0',
      releasedAt: '预览版本',
      channel: 'stable',
      qimenbot: '>=0.6.0 <0.7.0',
      dynamicApi: '0.6',
      changelog: '演示版本信息，实际数据来自插件作者的商城 PR。',
      drivers: [
        { driver: 'qq-official', scenes: ['group', 'group-at'], outbound: ['reply', 'rich-message'] },
        { driver: 'onebot11', scenes: ['group', 'private'], outbound: ['reply'] },
      ],
      assets: [
        { target: 'x86_64-unknown-linux-gnu', sizeBytes: 1843200, minGlibc: '2.31', githubAttestation: true },
        { target: 'aarch64-unknown-linux-gnu', sizeBytes: 1720320, minGlibc: '2.31', githubAttestation: true },
      ],
    },
    versions: 3,
  },
  {
    id: 'schedule-center',
    name: '任务调度中心',
    summary: '为机器人增加可追踪的定时任务、执行记录和失败重试。',
    description: '用于预览不同插件类型和版本状态的展示方式。',
    kind: 'static',
    repository: 'lvyunqi/QimenBot',
    repositoryUrl: 'https://github.com/lvyunqi/QimenBot',
    license: 'MIT',
    authors: ['QimenBot Contributors'],
    categories: ['自动化', '系统'],
    keywords: ['schedule', 'jobs'],
    trust: 'official',
    demo: true,
    github: { language: 'Rust' },
    latest: {
      version: '0.4.2',
      releasedAt: '预览版本',
      channel: 'stable',
      qimenbot: '>=0.4.0 <0.7.0',
      changelog: '静态插件不需要单独的动态库资产，需随宿主一起构建。',
      drivers: [{ driver: 'qq-official', scenes: ['group', 'private'], outbound: ['reply', 'proactive'] }],
      assets: [],
    },
    versions: 5,
  },
  {
    id: 'model-monitor',
    name: '模型状态监控',
    summary: '集中查看模型分组、采集器健康度和脱敏后的错误分类。',
    description: '用于预览预发布版本、构建徽标和多驱动兼容信息。',
    kind: 'dynamic',
    repository: 'lvyunqi/QimenBot',
    repositoryUrl: 'https://github.com/lvyunqi/QimenBot',
    license: 'MIT',
    authors: ['QimenBot Contributors'],
    categories: ['监控', '运维'],
    keywords: ['monitoring', 'health'],
    trust: 'verified-build',
    demo: true,
    github: { language: 'Rust' },
    latest: {
      version: '0.6.0-beta.1',
      releasedAt: '预览版本',
      channel: 'prerelease',
      qimenbot: '>=0.6.0 <0.7.0',
      dynamicApi: '0.6',
      changelog: '预发布版本会在管理员开启接收预发布后参与版本选择。',
      drivers: [{ driver: 'onebot11', scenes: ['group', 'private'], outbound: ['reply', 'proactive'] }],
      assets: [{ target: 'x86_64-pc-windows-msvc', sizeBytes: 2162688, githubAttestation: true }],
    },
    versions: 2,
  },
]

const plugins = computed(() => {
  if (livePlugins.value.length > 0) return livePlugins.value
  return import.meta.env.DEV && ['preview', 'empty', 'error'].includes(catalogState.value) ? demoPlugins : []
})

const filteredPlugins = computed(() => {
  const needle = searchTerm.value.trim().toLowerCase()
  return plugins.value.filter((plugin) => {
    const matchesFilter = activeFilter.value === 'all' || plugin.kind === activeFilter.value
    const haystack = [plugin.name, plugin.id, plugin.summary, plugin.repository, ...plugin.categories, ...plugin.keywords]
      .join(' ')
      .toLowerCase()
    return matchesFilter && (!needle || haystack.includes(needle))
  })
})

const totalPages = computed(() => Math.max(1, Math.ceil(filteredPlugins.value.length / pageSize.value)))

const pagedPlugins = computed(() => {
  const start = (currentPage.value - 1) * pageSize.value
  return filteredPlugins.value.slice(start, start + pageSize.value)
})

const pageItems = computed<Array<number | string>>(() => {
  const pages = totalPages.value
  if (pages <= 7) return Array.from({ length: pages }, (_, index) => index + 1)
  const start = Math.max(2, Math.min(currentPage.value - 1, pages - 3))
  const end = Math.min(pages - 1, start + 2)
  return [1, ...(start > 2 ? ['ellipsis-left'] : []), ...Array.from({ length: end - start + 1 }, (_, index) => start + index), ...(end < pages - 1 ? ['ellipsis-right'] : []), pages]
})

const counts = computed(() => ({
  all: plugins.value.length,
}))

watch([activeFilter, searchTerm], () => {
  currentPage.value = 1
})

watch(pageSize, () => {
  currentPage.value = 1
})

watch(totalPages, (pages) => {
  if (currentPage.value > pages) currentPage.value = pages
})

onMounted(async () => {
  catalogState.value = 'loading'
  try {
    const response = await fetch(`${import.meta.env.BASE_URL}marketplace/index.json`, { headers: { Accept: 'application/json' } })
    if (!response.ok) throw new Error(`catalog request failed: ${response.status}`)
    const data = await response.json()
    const parsed = (data.plugins ?? []).map(normalizeCatalogPlugin).filter(Boolean) as MarketplacePlugin[]
    livePlugins.value = parsed
    catalogState.value = parsed.length > 0 ? 'ready' : 'empty'
  } catch {
    catalogState.value = 'error'
  }
})

function normalizeCatalogPlugin(plugin: any): MarketplacePlugin | null {
  const manifest = plugin?.manifest ?? plugin
  const latest = [...(plugin?.versions ?? [])].sort((left, right) => compareVersions(left?.version, right?.version)).at(-1)
  if (!manifest?.id || !latest) return null
  return {
    id: manifest.id,
    name: manifest.name,
    summary: manifest.summary,
    description: manifest.description ?? manifest.summary,
    kind: manifest.type ?? manifest.kind,
    repository: manifest.repository,
    repositoryUrl: `https://github.com/${manifest.repository}`,
    license: manifest.license,
    authors: manifest.authors ?? [],
    categories: manifest.categories ?? [],
    keywords: manifest.keywords ?? [],
    trust: manifest.trust ?? 'community',
    github: manifest.github,
    latest: {
      version: latest.version,
      releasedAt: latest.released_at,
      channel: latest.channel,
      qimenbot: latest.qimenbot,
      dynamicApi: latest.dynamic_api,
      changelog: latest.changelog ?? '',
      drivers: latest.drivers ?? [],
      assets: (latest.assets ?? []).map((asset: any) => ({
        target: asset.target,
        sizeBytes: asset.size_bytes,
        minGlibc: asset.min_glibc,
        githubAttestation: asset.github_attestation,
      })),
    },
    versions: plugin.versions.length,
  }
}

function compareVersions(left = '', right = '') {
  return new Intl.Collator('en', { numeric: true, sensitivity: 'base' }).compare(left, right)
}

function driverLabel(driver: string) {
  return driver === 'qq-official' ? '官方 QQ Bot' : driver === 'onebot11' ? 'OneBot 11' : driver
}

function driverShortLabel(driver: string) {
  return driver === 'qq-official' ? 'QQ Bot' : 'OneBot'
}

function kindLabel(kind: PluginKind) {
  return kind === 'dynamic' ? '动态插件' : '静态模块'
}

function trustLabel(trust: string) {
  return trust === 'official' ? '官方维护' : trust === 'verified-build' ? '构建已验证' : '社区插件'
}

function formatNumber(value?: number) {
  return typeof value === 'number' ? new Intl.NumberFormat('zh-CN', { notation: 'compact', maximumFractionDigits: 1 }).format(value) : '待同步'
}

function formatSize(bytes?: number) {
  if (!bytes) return '宿主构建'
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`
}

function formatDate(value: string) {
  if (!value || value === '预览版本') return value
  const date = new Date(value)
  return Number.isNaN(date.valueOf()) ? value : new Intl.DateTimeFormat('zh-CN', { year: 'numeric', month: 'short', day: 'numeric' }).format(date)
}

function openDetail(plugin: MarketplacePlugin) {
  selectedPlugin.value = plugin
}

function closeDetail() {
  selectedPlugin.value = null
}

function goToPage(page: number) {
  currentPage.value = Math.min(Math.max(page, 1), totalPages.value)
}
</script>

<template>
  <main class="marketplace-preview">
    <section class="marketplace-hero">
      <div class="hero-copy">
        <div class="eyebrow"><span class="eyebrow-mark">Q</span><span>OPEN SOURCE / QimenBot Plugin Directory</span></div>
        <h1 class="hero-title"><span>开源：以众智为基，</span><em>筑即时交互之塔。</em></h1>
        <p class="hero-description"><strong>代码公开，能力共建。</strong>从开源仓库到可安装版本，每一个插件都带着兼容性、构建和来源信息，方便你在动手之前做出判断。</p>
        <div class="community-proof"><strong>{{ counts.all.toString().padStart(2, '0') }}</strong><span><b>个插件，正在共同构建</b><small>从源码到版本，开放协作持续发生</small></span></div>
        <div class="open-source-points"><span>源码可审阅</span><span>社区可参与</span><span>版本可追溯</span></div>
        <div class="hero-actions">
          <a class="button button-primary" href="#catalog">浏览插件 <span aria-hidden="true">→</span></a>
          <a class="button button-quiet" href="https://github.com/lvyunqi/QimenBot" target="_blank" rel="noreferrer">查看开源仓库 <span aria-hidden="true">↗</span></a>
        </div>
      </div>
    </section>

    <section id="catalog" class="catalog-section">
      <div class="section-heading">
        <div><span class="section-kicker">CATALOG / 01</span><h2>插件目录</h2></div>
        <p>{{ filteredPlugins.length }} 个结果<span v-if="catalogState !== 'ready'" class="demo-tag">演示数据</span></p>
      </div>
      <div class="catalog-toolbar">
        <label class="search-box"><span class="search-symbol" aria-hidden="true">⌕</span><input v-model="searchTerm" type="search" placeholder="搜索插件、仓库或能力" aria-label="搜索插件" /><kbd>/</kbd></label>
        <div class="filter-tabs" role="tablist" aria-label="插件类型筛选">
          <button v-for="filter in [{ key: 'all', label: '全部' }, { key: 'dynamic', label: '动态插件' }, { key: 'static', label: '静态模块' }]" :key="filter.key" class="filter-tab" :class="{ active: activeFilter === filter.key }" role="tab" :aria-selected="activeFilter === filter.key" @click="activeFilter = filter.key as FilterKey">{{ filter.label }}</button>
        </div>
      </div>

      <div v-if="filteredPlugins.length" class="plugin-grid">
        <article v-for="(plugin, index) in pagedPlugins" :key="plugin.id" class="plugin-card" :style="{ '--card-index': index }" tabindex="0" @click="openDetail(plugin)" @keydown.enter="openDetail(plugin)">
          <div class="card-topline"><span class="kind-label" :class="`kind-${plugin.kind}`"><span class="kind-pip"></span>{{ kindLabel(plugin.kind) }}</span><span class="trust-label">{{ trustLabel(plugin.trust) }}</span><a v-if="!plugin.demo" class="github-link" :href="plugin.repositoryUrl" target="_blank" rel="noreferrer" @click.stop>GitHub <span aria-hidden="true">↗</span></a><span v-else class="github-link is-demo">预览卡片</span></div>
          <div class="repository-line"><div class="github-glyph github-glyph-small" aria-hidden="true">GH</div><div class="repository-name"><span>{{ plugin.repository }}</span><strong>{{ plugin.id }}</strong></div></div>
          <h3>{{ plugin.name }}</h3>
          <p class="plugin-summary">{{ plugin.summary }}</p>
          <div class="plugin-tags"><span v-for="driver in plugin.latest.drivers" :key="driver.driver" class="tag tag-driver">{{ driverShortLabel(driver.driver) }}</span><span v-for="category in plugin.categories.slice(0, 2)" :key="category" class="tag">{{ category }}</span></div>
          <div class="card-divider"></div>
          <div class="card-metrics"><span><small>最新版本</small><strong>{{ plugin.latest.version }}</strong></span><span><small>维护者</small><strong>{{ plugin.authors[0] ?? '社区贡献者' }}</strong></span><span><small>GitHub</small><strong>{{ formatNumber(plugin.github?.stars) }} stars</strong></span></div>
          <div class="card-footer"><span>{{ plugin.latest.channel === 'prerelease' ? '预发布版本' : formatDate(plugin.latest.releasedAt) }}</span><button class="detail-button" type="button" @click.stop="openDetail(plugin)">查看详情 <span aria-hidden="true">→</span></button></div>
        </article>
      </div>
      <div v-else class="catalog-empty"><span class="empty-index">00</span><div><strong>没有匹配的插件</strong><p>试试更短的关键词，或切换到“全部”。</p></div></div>
      <nav v-if="filteredPlugins.length" class="pagination" aria-label="插件列表分页">
        <div class="pagination-summary"><span>PLUGIN DIRECTORY</span><strong>第 {{ currentPage }} / {{ totalPages }} 页</strong><small>共 {{ filteredPlugins.length }} 个插件</small></div>
        <div class="pagination-controls">
          <label class="page-size">每页 <select v-model.number="pageSize" aria-label="每页显示数量"><option :value="6">6</option><option :value="12">12</option><option :value="24">24</option></select></label>
          <button class="page-arrow" type="button" aria-label="上一页" :disabled="currentPage === 1" @click="goToPage(currentPage - 1)">←</button>
          <template v-for="item in pageItems" :key="item">
            <span v-if="typeof item === 'string'" class="page-ellipsis">…</span>
            <button v-else class="page-number" type="button" :class="{ active: currentPage === item }" :aria-current="currentPage === item ? 'page' : undefined" @click="goToPage(item)">{{ item }}</button>
          </template>
          <button class="page-arrow" type="button" aria-label="下一页" :disabled="currentPage === totalPages" @click="goToPage(currentPage + 1)">→</button>
        </div>
      </nav>
    </section>

    <footer class="marketplace-footer"><span>索引来自 GitHub Pages，版本资产保留在插件作者的 Release 中。</span><a href="/QimenBot/plugin/marketplace">阅读安装与审核规则 <span aria-hidden="true">→</span></a></footer>

    <div v-if="selectedPlugin" class="detail-backdrop" role="presentation" @click.self="closeDetail">
      <section class="detail-sheet" role="dialog" aria-modal="true" :aria-label="`${selectedPlugin.name} 插件详情`">
        <button class="close-detail" type="button" aria-label="关闭详情" @click="closeDetail">×</button>
        <div class="detail-heading"><span class="section-kicker">PLUGIN / {{ selectedPlugin.kind.toUpperCase() }}</span><h2>{{ selectedPlugin.name }}</h2><p>{{ selectedPlugin.summary }}</p></div>
        <div class="detail-repo"><div class="github-glyph">GH</div><div><span>开源仓库</span><strong>{{ selectedPlugin.repository }}</strong></div><a v-if="!selectedPlugin.demo" :href="selectedPlugin.repositoryUrl" target="_blank" rel="noreferrer">打开 GitHub ↗</a><span v-else class="detail-demo">预览数据</span></div>
        <div class="detail-grid"><div><span>当前版本</span><strong>{{ selectedPlugin.latest.version }}</strong></div><div><span>兼容 QimenBot</span><strong>{{ selectedPlugin.latest.qimenbot }}</strong></div><div><span>许可证</span><strong>{{ selectedPlugin.license }}</strong></div><div><span>版本记录</span><strong>{{ selectedPlugin.versions }} 个版本</strong></div></div>
        <div class="detail-block"><span class="detail-label">驱动兼容</span><div class="detail-driver-list"><div v-for="driver in selectedPlugin.latest.drivers" :key="driver.driver" class="detail-driver"><strong>{{ driverLabel(driver.driver) }}</strong><span>{{ driver.scenes?.join(' · ') || '场景由插件声明' }}</span></div></div></div>
        <div v-if="selectedPlugin.latest.assets.length" class="detail-block"><span class="detail-label">构建产物</span><div class="asset-list"><div v-for="asset in selectedPlugin.latest.assets" :key="asset.target" class="asset-row"><span>{{ asset.target }}</span><span>{{ formatSize(asset.sizeBytes) }}<template v-if="asset.minGlibc"> · glibc {{ asset.minGlibc }}</template></span><span v-if="asset.githubAttestation" class="attestation">构建已验证</span></div></div></div>
        <div class="detail-note"><span class="note-mark">i</span><p>{{ selectedPlugin.latest.changelog }}</p></div>
      </section>
    </div>
  </main>
</template>

<style scoped>
:global(.VPDoc:has(.marketplace-preview) .content) { width: 100% !important; max-width: none !important; }
:global(.VPDoc:has(.marketplace-preview) .aside) { display: none; }
:global(.VPDoc:has(.marketplace-preview) .container) { max-width: 1280px !important; }
:global(.VPDoc:has(.marketplace-preview) .content-container) { width: 100% !important; max-width: 1280px !important; }
:global(.VPDoc:has(.marketplace-preview) .VPDocFooter) { display: none; }

.marketplace-preview {
  --ink: #17191c;
  --muted: #68717c;
  --faint: #8f98a3;
  --line: #dfe4e9;
  --paper: #f7f9fb;
  --panel: #ffffff;
  --purple: #6d42d9;
  --purple-soft: #eeeafd;
  --green: #1c8255;
  --green-soft: #e6f4eb;
  --blue: #2f6fcb;
  --amber: #aa6b17;
  color: var(--ink);
  margin: -32px -32px 0;
  overflow: hidden;
  background: var(--paper);
}

.marketplace-hero { max-width: 1280px; margin: 0 auto; padding: 88px 64px 78px; background: radial-gradient(circle at 18% 20%, rgba(109, 66, 217, .08), transparent 36%), var(--paper); }
.hero-copy { max-width: 820px; margin: 0 auto; text-align: center; }
.eyebrow, .card-topline, .section-kicker, .detail-label, .detail-grid span, .detail-repo span { letter-spacing: .08em; text-transform: uppercase; font-size: 11px; font-weight: 700; }
.eyebrow { display: flex; align-items: center; justify-content: center; gap: 10px; color: var(--purple); }
.eyebrow-mark { display: grid; width: 24px; height: 24px; place-items: center; border: 1px solid currentColor; border-radius: 7px; font-size: 13px; }
h1 { max-width: 820px; margin: 24px auto 20px; font-size: clamp(42px, 5.2vw, 74px); line-height: .98; letter-spacing: -.045em; font-weight: 760; text-wrap: balance; }
.hero-title span, .hero-title em { display: block; white-space: nowrap; }
.hero-title em { margin-top: 7px; color: var(--purple); font-style: normal; }
.hero-description { max-width: 680px; margin: 0 auto; color: var(--muted); font-size: 17px; line-height: 1.75; text-wrap: pretty; }
.hero-description strong { display: block; margin-bottom: 7px; color: var(--ink); font-size: 21px; letter-spacing: -.03em; }
.hero-actions { display: flex; flex-wrap: wrap; justify-content: center; gap: 12px; margin-top: 32px; }
.button { display: inline-flex; align-items: center; gap: 12px; min-height: 44px; padding: 0 17px; border-radius: 8px; font-size: 13px; font-weight: 700; text-decoration: none; transition: transform .18s ease, box-shadow .18s ease, background .18s ease; }
.button:hover { transform: translateY(-2px); }
.button-primary { color: #fff; background: var(--ink); box-shadow: 0 8px 20px rgba(23, 25, 28, .14); }
.button-primary:hover { box-shadow: 0 12px 28px rgba(23, 25, 28, .2); }
.button-quiet { color: var(--ink); border: 1px solid var(--line); background: rgba(255,255,255,.55); }
.button-quiet:hover { border-color: #bcc4cc; background: #fff; }

.community-proof { display: flex; align-items: baseline; justify-content: center; gap: 13px; margin-top: 28px; text-align: left; }
.community-proof > strong { color: var(--purple); font-family: ui-monospace, SFMono-Regular, Consolas, monospace; font-size: 50px; letter-spacing: -.08em; line-height: .9; }
.community-proof span { display: flex; flex-direction: column; gap: 6px; }
.community-proof b { font-size: 14px; }
.community-proof small { color: var(--faint); font-size: 11px; }
.github-glyph { display: grid; flex: none; width: 40px; height: 40px; place-items: center; border-radius: 11px; color: #fff; background: #20252b; font-family: ui-monospace, SFMono-Regular, Consolas, monospace; font-size: 11px; font-weight: 800; letter-spacing: -.04em; }
.github-glyph-small { width: 30px; height: 30px; border-radius: 8px; font-size: 9px; }
.repository-name span { display: block; color: var(--faint); font-size: 11px; }
.repository-name strong { display: block; margin-top: 4px; font-family: ui-monospace, SFMono-Regular, Consolas, monospace; font-size: 14px; letter-spacing: -.02em; }
.open-source-points { display: flex; flex-wrap: wrap; justify-content: center; gap: 8px 18px; margin-top: 20px; color: var(--faint); font-family: ui-monospace, SFMono-Regular, Consolas, monospace; font-size: 10px; }
.open-source-points span { display: inline-flex; align-items: center; gap: 7px; }
.open-source-points span::before { width: 5px; height: 5px; border-radius: 50%; background: var(--green); content: ''; }

.catalog-section { max-width: 1280px; margin: 0 auto; padding: 78px 64px 80px; }
.section-heading { display: flex; align-items: end; justify-content: space-between; gap: 20px; }
.section-kicker { display: block; color: var(--purple); }
.section-heading h2 { margin: 11px 0 0; font-size: 35px; letter-spacing: -.04em; }
.section-heading p { margin: 0 0 5px; color: var(--muted); font-family: ui-monospace, SFMono-Regular, Consolas, monospace; font-size: 12px; }
.demo-tag { display: inline-flex; margin-left: 10px; padding: 4px 7px; border: 1px solid #d9d0fb; border-radius: 999px; color: var(--purple); background: var(--purple-soft); font-family: inherit; font-size: 10px; }
.catalog-toolbar { display: flex; align-items: center; justify-content: space-between; gap: 20px; margin: 30px 0 24px; }
.search-box { display: flex; align-items: center; gap: 10px; flex: 1; max-width: 490px; min-height: 44px; padding: 0 12px; border: 1px solid var(--line); border-radius: 8px; background: #fff; transition: border .18s ease, box-shadow .18s ease; }
.search-box:focus-within { border-color: #b5a4ed; box-shadow: 0 0 0 4px rgba(109, 66, 217, .1); }
.search-symbol { color: var(--faint); font-size: 21px; line-height: 1; }
.search-box input { min-width: 0; flex: 1; border: 0; outline: 0; color: var(--ink); background: transparent; font: inherit; font-size: 13px; }
.search-box input::placeholder { color: #9aa3ad; }
kbd { padding: 3px 6px; border: 1px solid var(--line); border-radius: 4px; color: var(--faint); background: #f7f8fa; font-family: ui-monospace, SFMono-Regular, Consolas, monospace; font-size: 11px; }
.filter-tabs { display: flex; align-items: center; gap: 3px; padding: 3px; border: 1px solid var(--line); border-radius: 8px; background: #eef1f4; }
.filter-tab { min-height: 36px; padding: 0 13px; border: 0; border-radius: 6px; color: var(--muted); background: transparent; font: inherit; font-size: 12px; font-weight: 700; cursor: pointer; transition: color .18s ease, background .18s ease, box-shadow .18s ease; }
.filter-tab:hover { color: var(--ink); }
.filter-tab.active { color: var(--ink); background: #fff; box-shadow: 0 2px 7px rgba(32, 45, 58, .08); }
.plugin-grid { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 16px; }
.plugin-card { display: flex; min-height: 360px; flex-direction: column; padding: 22px; border: 1px solid var(--line); border-radius: 9px; background: var(--panel); box-shadow: 0 8px 24px rgba(32,45,58,.03); cursor: pointer; opacity: 0; animation: card-in .5s cubic-bezier(.2,.8,.2,1) forwards; animation-delay: calc(var(--card-index) * 55ms); transition: transform .2s ease, border-color .2s ease, box-shadow .2s ease; }
.plugin-card:hover, .plugin-card:focus-visible { border-color: #b7a7ea; box-shadow: 0 18px 36px rgba(32,45,58,.09); transform: translateY(-4px); outline: 0; }
.card-topline { display: flex; align-items: center; gap: 8px; min-height: 18px; color: var(--faint); letter-spacing: .03em; text-transform: none; font-size: 10px; }
.kind-label { display: inline-flex; align-items: center; gap: 6px; color: var(--purple); font-weight: 750; }
.kind-static { color: var(--blue); }
.kind-pip { width: 6px; height: 6px; border-radius: 50%; background: currentColor; }
.trust-label { padding-left: 8px; border-left: 1px solid var(--line); }
.github-link { margin-left: auto; color: var(--blue); font-size: 11px; font-weight: 700; text-decoration: none; }
.github-link:hover { text-decoration: underline; }
.github-link.is-demo { color: var(--faint); cursor: default; }
.repository-line { display: flex; align-items: center; gap: 11px; margin: 28px 0 20px; }
.repository-name { min-width: 0; }
.repository-name span, .repository-name strong { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.plugin-card h3 { margin: 0; font-size: 23px; letter-spacing: -.03em; line-height: 1.2; }
.plugin-summary { min-height: 52px; margin: 10px 0 16px; color: var(--muted); font-size: 13px; line-height: 1.65; text-wrap: pretty; }
.plugin-tags { display: flex; flex-wrap: wrap; gap: 6px; }
.tag { padding: 5px 8px; border: 1px solid #e4e7eb; border-radius: 5px; color: var(--muted); background: #fafbfc; font-size: 10px; line-height: 1; }
.tag-driver { border-color: #d4e4f7; color: var(--blue); background: #f2f7fd; }
.card-divider { height: 1px; margin: auto 0 17px; background: var(--line); }
.card-metrics { display: grid; grid-template-columns: 1.05fr 1fr .9fr; gap: 10px; }
.card-metrics span { min-width: 0; }
.card-metrics small { display: block; margin-bottom: 5px; color: var(--faint); font-size: 10px; }
.card-metrics strong { display: block; overflow: hidden; color: var(--ink); font-family: ui-monospace, SFMono-Regular, Consolas, monospace; font-size: 11px; font-weight: 650; text-overflow: ellipsis; white-space: nowrap; }
.card-footer { display: flex; align-items: center; justify-content: space-between; gap: 12px; margin-top: 20px; color: var(--faint); font-family: ui-monospace, SFMono-Regular, Consolas, monospace; font-size: 10px; }
.detail-button { padding: 0; border: 0; color: var(--purple); background: transparent; font: inherit; font-weight: 700; cursor: pointer; }
.detail-button:hover { color: #4f28b5; }
.catalog-empty { display: flex; align-items: center; gap: 18px; min-height: 180px; padding: 30px; border: 1px dashed var(--line); border-radius: 9px; background: rgba(255,255,255,.5); }
.empty-index { color: #c6ccd3; font-family: ui-monospace, SFMono-Regular, Consolas, monospace; font-size: 44px; font-weight: 700; }
.catalog-empty strong { font-size: 16px; }
.catalog-empty p { margin: 6px 0 0; color: var(--muted); font-size: 13px; }
.pagination { display: flex; align-items: center; justify-content: space-between; gap: 20px; margin-top: 28px; padding-top: 18px; border-top: 1px solid var(--line); }
.pagination-summary { display: flex; align-items: baseline; gap: 12px; min-width: 0; }
.pagination-summary span { color: var(--faint); font-family: ui-monospace, SFMono-Regular, Consolas, monospace; font-size: 10px; letter-spacing: .08em; }
.pagination-summary strong { font-size: 12px; }
.pagination-summary small { color: var(--muted); font-size: 11px; }
.pagination-controls { display: flex; align-items: center; gap: 5px; }
.page-size { display: inline-flex; align-items: center; gap: 6px; margin-right: 10px; color: var(--muted); font-size: 11px; }
.page-size select { min-height: 31px; padding: 0 22px 0 8px; border: 1px solid var(--line); border-radius: 6px; color: var(--ink); background: #fff; font: inherit; cursor: pointer; }
.page-arrow, .page-number { display: grid; width: 32px; height: 32px; place-items: center; border: 1px solid transparent; border-radius: 6px; color: var(--muted); background: transparent; font-family: ui-monospace, SFMono-Regular, Consolas, monospace; font-size: 12px; cursor: pointer; transition: color .18s ease, border-color .18s ease, background .18s ease; }
.page-arrow:hover:not(:disabled), .page-number:hover { border-color: var(--line); color: var(--ink); background: #fff; }
.page-arrow:disabled { color: #c4cbd2; cursor: not-allowed; }
.page-number.active { border-color: #c9bdf0; color: var(--purple); background: var(--purple-soft); font-weight: 700; }
.page-ellipsis { width: 20px; color: var(--faint); text-align: center; }
.marketplace-footer { display: flex; align-items: center; justify-content: space-between; gap: 20px; max-width: 1280px; margin: 0 auto; padding: 20px 64px 38px; border-top: 1px solid var(--line); color: var(--faint); font-size: 11px; }
.marketplace-footer a { color: var(--purple); font-weight: 700; text-decoration: none; }
.marketplace-footer a:hover { text-decoration: underline; }

.detail-backdrop { position: fixed; z-index: 100; inset: 0; display: grid; place-items: center; padding: 24px; background: rgba(19, 23, 28, .46); backdrop-filter: blur(8px); }
.detail-sheet { position: relative; width: min(640px, 100%); max-height: min(760px, calc(100vh - 48px)); overflow: auto; padding: 35px; border: 1px solid rgba(255,255,255,.45); border-radius: 12px; background: #fff; box-shadow: 0 28px 80px rgba(9, 13, 18, .25); animation: sheet-in .25s cubic-bezier(.2,.8,.2,1); }
.close-detail { position: absolute; top: 17px; right: 19px; width: 34px; height: 34px; border: 1px solid var(--line); border-radius: 7px; color: var(--muted); background: #fff; font-size: 22px; line-height: 1; cursor: pointer; }
.close-detail:hover { color: var(--ink); border-color: #b8c0c8; }
.detail-heading h2 { margin: 10px 0 8px; font-size: 33px; letter-spacing: -.04em; }
.detail-heading p { max-width: 530px; margin: 0; color: var(--muted); line-height: 1.6; }
.detail-repo { display: flex; align-items: center; gap: 12px; margin: 30px 0 25px; padding: 15px; border: 1px solid var(--line); border-radius: 8px; background: #fafbfc; }
.detail-repo strong { display: block; margin-top: 4px; font-family: ui-monospace, SFMono-Regular, Consolas, monospace; font-size: 13px; }
.detail-repo a { margin-left: auto; color: var(--blue); font-size: 12px; font-weight: 700; text-decoration: none; }
.detail-demo { margin-left: auto; color: var(--faint); font-family: ui-monospace, SFMono-Regular, Consolas, monospace; font-size: 11px; }
.detail-grid { display: grid; grid-template-columns: repeat(4, 1fr); gap: 1px; margin-bottom: 28px; border: 1px solid var(--line); background: var(--line); }
.detail-grid div { min-width: 0; padding: 14px; background: #fff; }
.detail-grid span { display: block; margin-bottom: 8px; color: var(--faint); letter-spacing: .05em; text-transform: none; }
.detail-grid strong { display: block; overflow: hidden; font-family: ui-monospace, SFMono-Regular, Consolas, monospace; font-size: 12px; text-overflow: ellipsis; white-space: nowrap; }
.detail-block { margin-top: 24px; }
.detail-label { display: block; margin-bottom: 10px; color: var(--faint); letter-spacing: .05em; text-transform: none; }
.detail-driver-list, .asset-list { display: grid; gap: 8px; }
.detail-driver, .asset-row { display: flex; align-items: center; justify-content: space-between; gap: 16px; padding: 12px 13px; border: 1px solid var(--line); border-radius: 7px; background: #fafbfc; font-size: 12px; }
.detail-driver strong, .asset-row span:first-child { font-family: ui-monospace, SFMono-Regular, Consolas, monospace; }
.detail-driver span, .asset-row span:nth-child(2) { color: var(--muted); text-align: right; }
.asset-row { flex-wrap: wrap; }
.attestation { padding: 4px 6px; border-radius: 4px; color: var(--green); background: var(--green-soft); font-size: 10px; font-weight: 700; }
.detail-note { display: flex; gap: 10px; margin-top: 28px; padding: 14px; border-radius: 7px; color: var(--muted); background: #f5f3fd; font-size: 12px; line-height: 1.6; }
.note-mark { display: grid; flex: none; width: 18px; height: 18px; place-items: center; border: 1px solid #c5b9ef; border-radius: 50%; color: var(--purple); font-family: ui-monospace, SFMono-Regular, Consolas, monospace; font-size: 11px; font-weight: 700; }
.detail-note p { margin: 0; }

@keyframes card-in { from { opacity: 0; transform: translateY(8px); } to { opacity: 1; transform: translateY(0); } }
@keyframes sheet-in { from { opacity: 0; transform: translateY(12px) scale(.985); } to { opacity: 1; transform: translateY(0) scale(1); } }

@media (max-width: 1100px) {
  .marketplace-hero { padding-top: 64px; }
  .plugin-grid { grid-template-columns: repeat(2, minmax(0, 1fr)); }
}

@media (max-width: 760px) {
  .marketplace-preview { margin: -24px -24px 0; }
  .marketplace-hero, .catalog-section { padding-right: 22px; padding-left: 22px; }
  .marketplace-hero { padding-top: 52px; padding-bottom: 52px; }
  h1 { font-size: clamp(31px, 9.6vw, 44px); }
  .hero-description { font-size: 15px; }
  .catalog-section { padding-top: 55px; padding-bottom: 52px; }
  .section-heading { align-items: start; flex-direction: column; gap: 10px; }
  .catalog-toolbar { align-items: stretch; flex-direction: column; gap: 12px; }
  .search-box { max-width: none; }
  .filter-tabs { overflow-x: auto; }
  .filter-tab { flex: 1; white-space: nowrap; }
  .plugin-grid { grid-template-columns: 1fr; }
  .plugin-card { min-height: 340px; }
  .pagination { align-items: flex-start; flex-direction: column; gap: 14px; }
  .pagination-controls { width: 100%; justify-content: flex-end; }
  .marketplace-footer { align-items: flex-start; flex-direction: column; padding: 20px 22px 30px; }
  .detail-backdrop { padding: 12px; }
  .detail-sheet { max-height: calc(100vh - 24px); padding: 27px 20px 22px; }
  .detail-heading h2 { font-size: 28px; }
  .detail-grid { grid-template-columns: repeat(2, 1fr); }
  .detail-driver, .asset-row { align-items: flex-start; flex-direction: column; gap: 6px; }
  .detail-driver span, .asset-row span:nth-child(2) { text-align: left; }
  .asset-row .attestation { align-self: flex-start; }
}

@media (prefers-reduced-motion: reduce) {
  .plugin-card, .detail-sheet { animation: none; transition: none; }
  .button:hover, .plugin-card:hover, .plugin-card:focus-visible { transform: none; }
}
</style>
