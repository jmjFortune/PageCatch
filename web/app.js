'use strict';

const $ = id => document.getElementById(id);
const tokenStorageKey = 'pagecatch.accessToken';
const themeStorageKey = 'pagecatch.theme';
const localeStorageKey = 'pagecatch.locale';

// ==========================================================================
// 1. Internationalization (i18n) Dictionary & Engine
// ==========================================================================

const i18n = {
  zh: {
    nav: {
      downloads: '下载任务',
      settings: '偏好设置'
    },
    downloads: {
      title: '下载任务',
      subtitle: '管理视频提取与分片下载队列'
    },
    toolbar: {
      newTask: '新建任务',
      refresh: '刷新'
    },
    filter: {
      all: '全部',
      active: '进行中',
      completed: '已完成',
      failed: '失败',
      dead: '失效'
    },
    empty: {
      loading: '正在载入任务…',
      noTasks: '暂无下载任务',
      noMatch: '无匹配的任务'
    },
    settings: {
      title: '偏好设置',
      subtitle: '系统运行参数与网络授权配置',
      tabDownload: '下载设置',
      tabNetwork: '网络与授权',
      tabAbout: '关于系统',
      outputLabel: '保存目录',
      outputHint: '按 YYYY-MM-DD (UTC+8) 日期子目录保存。Docker 环境请填写挂载路径。',
      workersLabel: '并发下载任务数',
      cpuLabel: 'CPU 核心限制',
      threadsLabel: '单个任务分片线程',
      retriesLabel: '最大重试次数',
      retriesHint: '包含首次尝试。',
      previewLabel: '下载模式',
      downloadPanelHint: '配置已持久保存在服务端；新任务立即应用。',
      tokenLabel: '访问令牌 (Token)',
      headersLabel: '自定义请求头',
      networkPanelHint: 'Token 保存于当前浏览器中；请求头仅在当前标签页有效。'
    },
    about: {
      version: '版本',
      serverAddress: '服务地址',
      engine: '下载引擎',
      videoProcessing: '视频处理与校验',
      browserParsing: '浏览器解析'
    },
    dialog: {
      addTitle: '新建下载任务',
      addDesc: '支持粘贴单条/批量链接或导入 TXT / CSV 清单',
      urlsLabel: '视频链接',
      importFile: '导入文件',
      dropHint: '释放 TXT 或 CSV 文件以自动导入',
      urlHint: 'TXT / CSV · 最大 1 MB · 确认后加入队列',
      addToQueue: '加入队列',
      logTitle: '任务日志',
      deleteTitle: '删除任务记录？',
      deleteDesc: '从任务列表中移除此项？已下载的视频文件将被保留。',
      confirmDelete: '删除记录'
    },
    actions: {
      save: '保存更改',
      discard: '放弃修改',
      cancel: '取消',
      copyLink: '复制链接',
      copyPath: '复制保存路径',
      viewLogs: '查看日志',
      cancelTask: '取消任务',
      retryTask: '重试任务',
      deleteRecord: '删除记录',
      markDead: '移至失效 (Move to Dead)'
    },
    notice: {
      accessSettings: '访问设置',
      linkCopied: '链接已复制到剪贴板',
      pathCopied: '文件路径已复制到剪贴板',
      recordDeleted: '任务记录已删除，文件已保留',
      saved: '设置已保存',
      adding: '正在加入下载队列…',
      markedDead: '任务已移入失效 (Dead) 列表'
    },
    status: {
      pending: '排队等待',
      resolving: '正在解析',
      downloading: '正在下载',
      verifying: '校验合成',
      ok: '已完成',
      duplicate: '已存在跳过',
      fail: '下载失败',
      cancelled: '已取消',
      dead: '失效 (Dead)'
    },
    connection: {
      connected: '服务正常',
      authRequired: '需要访问令牌',
      disconnected: '连接中断'
    }
  },
  en: {
    nav: {
      downloads: 'Downloads',
      settings: 'Settings'
    },
    downloads: {
      title: 'Downloads',
      subtitle: 'Manage video capture & download tasks'
    },
    toolbar: {
      newTask: 'New Task',
      refresh: 'Refresh'
    },
    filter: {
      all: 'All',
      active: 'Active',
      completed: 'Completed',
      failed: 'Failed',
      dead: 'Dead'
    },
    empty: {
      loading: 'Loading tasks…',
      noTasks: 'No tasks yet',
      noMatch: 'No matching tasks'
    },
    settings: {
      title: 'Settings',
      subtitle: 'System preferences & connection configuration',
      tabDownload: 'Downloads',
      tabNetwork: 'Network',
      tabAbout: 'About',
      outputLabel: 'Save location',
      outputHint: 'Automatically saved in YYYY-MM-DD folders (UTC+8). In Docker, use a mounted path.',
      workersLabel: 'Concurrent downloads',
      cpuLabel: 'CPU core limit',
      threadsLabel: 'Segment threads',
      retriesLabel: 'Maximum attempts',
      retriesHint: 'Includes the first attempt.',
      previewLabel: 'Download mode',
      downloadPanelHint: 'Saved on the server. Other settings apply to new tasks.',
      tokenLabel: 'Access token',
      headersLabel: 'Request headers',
      networkPanelHint: 'Token remembered in this browser. Headers kept for this tab only.'
    },
    about: {
      version: 'Version',
      serverAddress: 'Server address',
      engine: 'Download engine',
      videoProcessing: 'Video processing',
      browserParsing: 'Browser parsing'
    },
    dialog: {
      addTitle: 'New Task',
      addDesc: 'Enter video URLs or import TXT / CSV file',
      urlsLabel: 'Video URLs',
      importFile: 'Import File',
      dropHint: 'Drop TXT or CSV here to import',
      urlHint: 'TXT / CSV · Up to 1 MB · Review before adding',
      addToQueue: 'Add to Queue',
      logTitle: 'Task Logs',
      deleteTitle: 'Delete record?',
      deleteDesc: 'Remove this task from the list? Video files will be kept.',
      confirmDelete: 'Delete Record'
    },
    actions: {
      save: 'Save Changes',
      discard: 'Discard Changes',
      cancel: 'Cancel',
      copyLink: 'Copy link',
      copyPath: 'Copy save path',
      viewLogs: 'View logs',
      cancelTask: 'Cancel task',
      retryTask: 'Retry task',
      deleteRecord: 'Delete record',
      markDead: 'Move to Dead'
    },
    notice: {
      accessSettings: 'Access settings',
      linkCopied: 'Link copied to clipboard',
      pathCopied: 'Save path copied to clipboard',
      recordDeleted: 'Record deleted. Video files kept.',
      saved: 'Settings saved',
      adding: 'Adding to queue…',
      markedDead: 'Task moved to Dead list'
    },
    status: {
      pending: 'Queued',
      resolving: 'Resolving',
      downloading: 'Downloading',
      verifying: 'Verifying',
      ok: 'Completed',
      duplicate: 'Duplicate skipped',
      fail: 'Failed',
      cancelled: 'Cancelled',
      dead: 'Dead'
    },
    connection: {
      connected: 'Connected',
      authRequired: 'Authentication required',
      disconnected: 'Disconnected'
    }
  }
};

let currentLocale = (() => {
  try {
    const saved = localStorage.getItem(localeStorageKey);
    if (saved && i18n[saved]) return saved;
  } catch {}
  return navigator.language && navigator.language.startsWith('zh') ? 'zh' : 'en';
})();

function t(path, fallback = '') {
  const parts = path.split('.');
  let current = i18n[currentLocale];
  for (const p of parts) {
    if (!current || current[p] === undefined) {
      current = null;
      break;
    }
    current = current[p];
  }
  return current || fallback || path;
}

function applyLocale(locale) {
  if (!i18n[locale]) return;
  currentLocale = locale;
  try { localStorage.setItem(localeStorageKey, locale); } catch {}
  document.documentElement.lang = locale === 'zh' ? 'zh-CN' : 'en';

  // Update elements with data-i18n
  document.querySelectorAll('[data-i18n]').forEach(el => {
    const key = el.getAttribute('data-i18n');
    el.textContent = t(key);
  });

  // Update language switcher buttons
  document.querySelectorAll('[data-lang-val]').forEach(btn => {
    btn.setAttribute('aria-pressed', String(btn.dataset.langVal === locale));
  });

  document.title = 'PageCatch · ' + ($('downloads-view').hidden ? t('nav.settings') : t('nav.downloads'));
  renderTasks();
  if (state.info) renderInfo();
}

// ==========================================================================
// 2. Appearance & Theme Manager (Auto / Light / Dark)
// ==========================================================================

let currentTheme = (() => {
  try {
    return localStorage.getItem(themeStorageKey) || 'auto';
  } catch {
    return 'auto';
  }
})();

function applyTheme(theme) {
  currentTheme = theme;
  try { localStorage.setItem(themeStorageKey, theme); } catch {}

  if (theme === 'auto') {
    document.documentElement.removeAttribute('data-theme');
  } else {
    document.documentElement.setAttribute('data-theme', theme);
  }

  document.querySelectorAll('[data-theme-val]').forEach(btn => {
    btn.setAttribute('aria-pressed', String(btn.dataset.themeVal === theme));
  });
}

// Listen to system changes when in 'auto' mode
window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', () => {
  if (currentTheme === 'auto') {
    // Re-trigger layout paints if needed
    renderTasks();
  }
});

// ==========================================================================
// 3. State & Tokens
// ==========================================================================

function rememberedToken() {
  try { return localStorage.getItem(tokenStorageKey) || ''; }
  catch { return ''; }
}

function rememberToken(token) {
  try {
    if (token) localStorage.setItem(tokenStorageKey, token);
    else localStorage.removeItem(tokenStorageKey);
    return true;
  } catch { return false; }
}

const state = {
  tasks: [],
  info: null,
  downloadSettings: null,
  filter: 'all',
  token: rememberedToken(),
  headers: {},
  epoch: 0,
  loading: true
};

const isActive = task => ['pending', 'resolving', 'downloading', 'verifying'].includes(task.status);
const isDone = task => ['ok', 'duplicate'].includes(task.status);

function completionTime(timestamp) {
  if (!Number.isFinite(timestamp) || timestamp <= 0) return '';
  const date = new Date((timestamp + 8 * 60 * 60) * 1000);
  return Number.isFinite(date.getTime()) ? date.toISOString().slice(0, 16).replace('T', ' ') : '';
}

let refreshing = false, savingNetwork = false, savingDownloads = false, submitting = false;
let logVersion = 0, noticeVersion = 0, logTaskId = null;
let deleteTask = null, deletingRecord = false;
let readingFile = false, importVersion = 0;

function node(tag, className, text) {
  const element = document.createElement(tag);
  if (className) element.className = className;
  if (text !== undefined) element.textContent = text;
  return element;
}

function icon(name) {
  const svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
  svg.setAttribute('class', 'icon');
  svg.setAttribute('aria-hidden', 'true');
  const use = document.createElementNS('http://www.w3.org/2000/svg', 'use');
  use.setAttribute('href', '#i-' + name);
  svg.append(use);
  return svg;
}

// Robust clipboard copy fallback for HTTP over LAN
async function copyText(text) {
  if (window.isSecureContext && navigator.clipboard) {
    await navigator.clipboard.writeText(text);
    return;
  }
  const area = document.createElement('textarea');
  area.value = text;
  area.setAttribute('readonly', '');
  area.style.cssText = 'position:fixed;top:-1000px;left:-1000px;opacity:0';
  document.body.append(area);
  const selection = document.getSelection();
  const previous = selection && selection.rangeCount ? selection.getRangeAt(0) : null;
  area.select();
  area.setSelectionRange(0, text.length);
  const copied = document.execCommand('copy');
  area.remove();
  if (selection && previous) { selection.removeAllRanges(); selection.addRange(previous); }
  if (!copied) throw new Error('Browser clipboard access denied.');
}

async function api(path, options = {}) {
  const { token = state.token, ...request } = options;
  const headers = { ...request.headers };
  if (token) headers.Authorization = 'Bearer ' + token;
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), 15000);
  try {
    const response = await fetch('/api' + path, { ...request, headers, cache: 'no-store', signal: controller.signal });
    if (!response.ok) {
      const body = await response.text();
      let message = body || 'Request failed: ' + response.status;
      try { message = JSON.parse(body).error || message; } catch {}
      const error = new Error(message);
      error.status = response.status;
      throw error;
    }
    return response;
  } catch (error) {
    if (error.name === 'AbortError') throw new Error(currentLocale === 'zh' ? '请求超时，请检查服务连接。' : 'Request timed out. Check the server connection.');
    if (error instanceof TypeError) throw new Error(currentLocale === 'zh' ? '无法连接到服务，请确认服务已启动。' : 'Cannot connect to the server. Check that it is running.');
    throw error;
  } finally { clearTimeout(timer); }
}

async function jsonApi(path, options) { return (await api(path, options)).json(); }

function message(id, text, error = false) {
  $(id).textContent = text;
  $(id).classList.toggle('error', error);
  $(id).setAttribute('role', error ? 'alert' : 'status');
}

function showNotice(text, error = false, kind = 'operation', auth = false) {
  const version = ++noticeVersion;
  $('notice').hidden = !text;
  $('notice').classList.toggle('error', error);
  $('notice').dataset.kind = kind;
  $('notice').setAttribute('role', error ? 'alert' : 'status');
  $('notice-text').textContent = text;
  $('notice-settings').hidden = !auth;
  if (text && !error) setTimeout(() => { if (version === noticeVersion) showNotice(''); }, 6000);
}

function setConnection(connected, needsToken = false) {
  const statusText = connected ? t('connection.connected') : needsToken ? t('connection.authRequired') : t('connection.disconnected');
  $('connection').textContent = statusText;
  $('connection').classList.toggle('offline', !connected);
  if ($('sidebar-conn-status')) $('sidebar-conn-status').textContent = connected ? '●' : '○';
}

function setView(view) {
  const downloads = view === 'downloads';
  $('downloads-view').hidden = !downloads;
  $('settings-view').hidden = downloads;
  for (const [id, selected] of [['nav-downloads', downloads], ['nav-settings', !downloads]]) {
    if (selected) $(id).setAttribute('aria-current', 'page');
    else $(id).removeAttribute('aria-current');
  }
  document.title = 'PageCatch · ' + (downloads ? t('nav.downloads') : t('nav.settings'));
}

function setSettingsTab(tab) {
  document.querySelectorAll('[data-setting]').forEach(button => {
    const selected = button.dataset.setting === tab;
    button.setAttribute('aria-selected', String(selected));
    button.tabIndex = selected ? 0 : -1;
    $('panel-' + button.dataset.setting).hidden = !selected;
  });
}

function renderInfo() {
  if (!state.info) return;
  const info = state.info;
  $('cpu_cores').max = info.cpu_available;
  if (!savingDownloads) {
    const dirty = downloadDirty();
    state.downloadSettings = { output: info.output, workers: info.workers, threads: info.threads, retries: info.retries, cpu_cores: info.cpu_cores };
    if (!dirty) fillDownloadSettings();
    updateDownloadButtons();
  }
  $('preview-row').hidden = !info.preview;
  $('preview').textContent = info.preview ? (currentLocale === 'zh' ? '预览模式：仅下载前 ' + info.preview + ' 个分片' : 'Preview: first ' + info.preview + ' segments') : (currentLocale === 'zh' ? '完整下载' : 'Full download');
  $('version').textContent = 'PageCatch ' + info.version;
  $('sidebar-version').textContent = 'v' + info.version;
  $('browser').textContent = info.browser ? (currentLocale === 'zh' ? 'Chrome / Chromium 可用 (支持动态页面)' : 'Chrome / Chromium available') : (currentLocale === 'zh' ? '未找到可用浏览器 (仅静态清单解析)' : 'No browser found; static parsing only');
}

function restoreTaskFocus(id, action) {
  const buttons = [...$('task-list').querySelectorAll('button')];
  const target = buttons.find(b => b.dataset.task === String(id) && b.dataset.action === action) || buttons.find(b => b.dataset.task === String(id));
  (target || $('refresh')).focus({ preventScroll: true });
}

// ==========================================================================
// 4. Inset Cards Task List Rendering
// ==========================================================================

function renderTasks() {
  if ($('log-dialog').open || $('delete-dialog').open) return;
  const focused = document.activeElement;
  const focusKey = focused?.closest('#task-list button') ? { id: focused.dataset.task, action: focused.dataset.action } : null;

  const counts = {
    all: state.tasks.length,
    active: state.tasks.filter(isActive).length,
    done: state.tasks.filter(isDone).length,
    fail: state.tasks.filter(t => t.status === 'fail').length,
    dead: state.tasks.filter(t => t.status === 'dead').length
  };

  $('nav-count').textContent = counts.all;
  for (const [name, count] of Object.entries(counts)) {
    $('count-' + name).textContent = count;
    $('count-' + name).hidden = name !== 'all' && count === 0;
  }

  const visible = state.tasks.filter(task => {
    if (state.filter === 'all') return true;
    if (state.filter === 'active') return isActive(task);
    if (state.filter === 'done') return isDone(task);
    if (state.filter === 'fail') return task.status === 'fail';
    if (state.filter === 'dead') return task.status === 'dead';
    return true;
  });

  const list = $('task-list');
  const fragment = document.createDocumentFragment();

  if (!visible.length) {
    const empty = node('div', 'empty');
    const emptyWrap = node('div', 'empty-icon-wrap');
    emptyWrap.append(icon('download'));
    empty.append(emptyWrap, node('p', '', state.loading ? t('empty.loading') : state.filter === 'all' ? t('empty.noTasks') : t('empty.noMatch')));
    fragment.append(empty);
  }

  for (const task of visible) {
    const row = node('article', 'task');
    row.dataset.taskId = String(task.id);

    // 1. File icon
    const fileIcon = node('div', 'file-icon');
    fileIcon.append(icon('video'));

    // 2. Content
    const content = node('div', 'task-content');
    const name = node('div', 'task-name', task.name || (currentLocale === 'zh' ? '视频任务' : 'video'));
    name.title = name.textContent;

    const source = node('div', 'task-url', task.url);
    source.title = task.url;
    content.append(name, source);

    // Active progress bar
    if (isActive(task)) {
      const progressWrap = node('div', 'task-progress-wrap');
      const track = node('div', 'task-progress-track');
      const bar = node('div', 'task-progress-bar');

      if (task.status === 'downloading') {
        // Active downloading bar
        bar.style.width = '100%';
        bar.classList.add('task-progress-indeterminate');
      } else {
        // Resolving / Verifying
        bar.style.width = '30%';
        bar.classList.add('task-progress-indeterminate');
      }
      track.append(bar);
      progressWrap.append(track);
      content.append(progressWrap);
    }

    // Status or custom message
    if (task.message && task.message !== (t('status.' + task.status) || task.status) && !['ok', 'duplicate'].includes(task.status)) {
      content.append(node('p', 'task-message' + (task.status === 'fail' ? ' error' : ''), task.message));
    }

    // Metadata
    const sizeStr = (task.bytes / 1024 / 1024).toFixed(1) + ' MB';
    const metadata = node('div', 'task-meta');
    metadata.append(document.createTextNode('#' + task.id + ' · ' + sizeStr));

    const completed = isDone(task) ? completionTime(task.updated) : '';
    if (completed) {
      const time = node('time', 'task-completed', completed);
      time.dateTime = new Date(task.updated * 1000).toISOString();
      time.title = currentLocale === 'zh' ? '完成时间 (UTC+8)' : 'Completed (UTC+8)';
      metadata.append(document.createTextNode(' · '), time);
    }

    if (task.attempt > 1) {
      metadata.append(document.createTextNode(' · ' + (currentLocale === 'zh' ? '重试: ' : 'Retries: ') + (task.attempt - 1)));
    }
    content.append(metadata);

    // Output file info
    if (task.output) {
      const file = node('div', 'task-file');
      file.title = task.output;
      file.append(icon('folder'), node('span', '', task.output.split(/[\\/]/).pop()));
      content.append(file);
    }

    // 3. Side Head & Actions
    const side = node('div', 'task-side');

    function action(label, symbol, type, callback) {
      const button = node('button', 'icon-button');
      button.type = 'button';
      button.title = label;
      button.setAttribute('aria-label', label);
      button.dataset.task = String(task.id);
      button.dataset.action = type;
      button.append(icon(symbol));
      button.onclick = async () => {
        const hadFocus = document.activeElement === button;
        button.disabled = true;
        try { await callback(); }
        catch (error) { showNotice(error.message, true); }
        finally {
          button.disabled = false;
          if (hadFocus && document.activeElement === document.body && !$('log-dialog').open && !$('downloads-view').hidden) {
            restoreTaskFocus(task.id, type);
          }
        }
      };
      return button;
    }

    const head = node('div', 'task-head');
    if (task.url) {
      head.append(action(t('actions.copyLink'), 'link', 'copy-link', async () => {
        await copyText(task.url);
        showNotice(t('notice.linkCopied'));
      }));
    }

    const badgeText = t('status.' + task.status) || task.status;
    head.append(node('span', 'badge ' + task.status, badgeText));
    side.append(head);

    const actions = node('div', 'task-actions');
    actions.append(action(t('actions.viewLogs'), 'log', 'log', () => openLog(task)));

    if (isActive(task)) {
      actions.append(action(t('actions.cancelTask'), 'close', 'cancel', async () => {
        await api('/tasks/' + task.id + '/cancel', { method: 'POST' });
        await refresh();
      }));
    }

    if (task.status === 'fail') {
      actions.append(action(t('actions.markDead'), 'dead', 'mark-dead', async () => {
        try {
          await api('/tasks/' + task.id + '/dead', { method: 'POST' });
        } catch {}
        task.status = 'dead';
        task.message = currentLocale === 'zh' ? '已标记为失效死链' : 'Marked as dead';
        showNotice(t('notice.markedDead'));
        renderTasks();
        await refresh();
      }));
    }

    if (['fail', 'cancelled', 'dead'].includes(task.status)) {
      actions.append(action(t('actions.retryTask'), 'retry', 'retry', async () => {
        await api('/tasks/' + task.id + '/retry', { method: 'POST' });
        await refresh();
      }));
    }

    if (task.output) {
      actions.append(action(t('actions.copyPath'), 'copy', 'copy', async () => {
        await copyText(task.output);
        showNotice(t('notice.pathCopied'));
      }));
    }

    if (['ok', 'duplicate', 'fail', 'cancelled', 'dead'].includes(task.status)) {
      actions.append(action(t('actions.deleteRecord'), 'trash', 'delete', () => openDeleteRecord(task)));
    }

    side.append(actions);
    row.append(fileIcon, content, side);
    fragment.append(row);
  }

  list.replaceChildren(fragment);

  if (focusKey) {
    const buttons = [...list.querySelectorAll('button')];
    const button = buttons.find(b => b.dataset.task === focusKey.id && b.dataset.action === focusKey.action) || buttons.find(b => b.dataset.task === focusKey.id);
    button?.focus({ preventScroll: true });
  }
}

// ==========================================================================
// 5. Data Refreshing & API
// ==========================================================================

async function refresh() {
  if (refreshing) return;
  refreshing = true;
  $('refresh').setAttribute('aria-busy', 'true');
  const epoch = state.epoch;
  try {
    const [tasks, info] = await Promise.all([jsonApi('/tasks'), jsonApi('/info')]);
    if (epoch !== state.epoch) return;
    state.tasks = tasks;
    state.info = info;
    state.loading = false;
    setConnection(true);
    if ($('notice').dataset.kind === 'connection') showNotice('');
    renderTasks();
    renderInfo();
  } catch (error) {
    if (epoch !== state.epoch) return;
    state.loading = false;
    setConnection(false, error.status === 401);
    showNotice(error.message, true, 'connection', error.status === 401);
    renderTasks();
  } finally {
    refreshing = false;
    $('refresh').removeAttribute('aria-busy');
  }
}

// ==========================================================================
// 6. Delete Dialog
// ==========================================================================

function openDeleteRecord(task) {
  deleteTask = task;
  $('delete-name').textContent = task.name || (currentLocale === 'zh' ? '视频任务' : 'video');
  message('delete-feedback', '');
  $('delete-dialog').showModal();
  $('cancel-delete').focus();
}

$('cancel-delete').onclick = () => $('delete-dialog').close();
$('delete-dialog').oncancel = event => { if (deletingRecord) event.preventDefault(); };
$('delete-dialog').onclose = () => {
  const id = deleteTask?.id;
  deleteTask = null;
  renderTasks();
  if (!$('downloads-view').hidden) restoreTaskFocus(id, 'delete');
};

$('delete-form').onsubmit = async event => {
  event.preventDefault();
  if (!deleteTask || deletingRecord) return;
  const id = deleteTask.id;
  deletingRecord = true;
  $('confirm-delete').disabled = $('cancel-delete').disabled = true;
  $('delete-form').setAttribute('aria-busy', 'true');
  message('delete-feedback', currentLocale === 'zh' ? '正在删除记录…' : 'Deleting record…');
  try {
    await api('/tasks/' + id, { method: 'DELETE' });
    state.epoch++;
    state.tasks = state.tasks.filter(task => task.id !== id);
    $('delete-dialog').close();
    renderTasks();
    showNotice(t('notice.recordDeleted'));
    await refresh();
  } catch (error) {
    message('delete-feedback', error.message, true);
  } finally {
    deletingRecord = false;
    $('confirm-delete').disabled = $('cancel-delete').disabled = false;
    $('delete-form').setAttribute('aria-busy', 'false');
  }
};

// ==========================================================================
// 7. Add Task Dialog & Drag-and-Drop File Import
// ==========================================================================

function openAdd() {
  message('add-feedback', '');
  $('add-dialog').showModal();
}

function rejectionReason(item) {
  try {
    new URL(item.url);
    return item.reason;
  } catch {
    return currentLocale === 'zh' ? '无效的链接，请输入完整的 HTTP/HTTPS 地址。' : 'Invalid URL. Enter a complete HTTP/HTTPS URL.';
  }
}

function inputLines() { return PageCatchImport.extract($('urls').value).urls; }

function updateAddControls() {
  $('submit').disabled = submitting || readingFile;
  $('import-file').disabled = submitting || readingFile;
  $('urls').disabled = submitting || readingFile;
  $('add-form').setAttribute('aria-busy', String(submitting || readingFile));
}

$('urls').oninput = () => {
  const count = inputLines().length;
  $('url-count').textContent = count + ' / 200 ' + (currentLocale === 'zh' ? '条链接' : 'URLs');
  $('url-count').classList.toggle('error', count > 200);
};

$('urls').onpaste = event => {
  const text = event.clipboardData?.getData('text/plain');
  if (!text) return;
  if (new TextEncoder().encode(text).length > PageCatchImport.maxFileBytes) {
    event.preventDefault();
    message('add-feedback', currentLocale === 'zh' ? '文本过大，单次最多粘贴 1 MB。' : 'Text is too large. Paste up to 1 MB at a time.', true);
    return;
  }
  const result = PageCatchImport.extract(text);
  if (!result.urls.length) return;
  event.preventDefault();
  const input = $('urls');
  const combined = PageCatchImport.extract(input.value.slice(0, input.selectionStart) + '\n' + text + '\n' + input.value.slice(input.selectionEnd));
  input.value = combined.urls.join('\n');
  input.oninput();
  input.setSelectionRange(input.value.length, input.value.length);
  message('add-feedback', (currentLocale === 'zh' ? '已自动提取有效链接。' : 'Links extracted. Review before adding.') + (combined.duplicates ? (currentLocale === 'zh' ? ' 跳过重复: ' : ' Duplicates skipped: ') + combined.duplicates + '.' : ''));
};

async function processImportedFile(file) {
  if (!file || submitting || readingFile) return;
  const version = ++importVersion;
  readingFile = true;
  updateAddControls();
  message('add-feedback', currentLocale === 'zh' ? '正在读取文件…' : 'Reading file…');
  try {
    const result = await PageCatchImport.readFile(file);
    if (version !== importVersion || !$('add-dialog').open) return;
    const merged = PageCatchImport.extract([...inputLines(), ...result.urls].join('\n'));
    $('urls').value = merged.urls.join('\n');
    $('urls').oninput();
    const duplicates = result.duplicates + merged.duplicates;
    message('add-feedback', file.name + ': ' + (currentLocale === 'zh' ? '提取到 ' + result.urls.length + ' 条链接。' : result.urls.length + ' URLs found.') + (duplicates ? (currentLocale === 'zh' ? ' 跳过重复: ' + duplicates + '。' : ' Duplicates skipped: ' + duplicates + '.') : '') + (currentLocale === 'zh' ? ' 请确认后加入队列。' : ' Review before adding.'));
    if (merged.urls.length > 200) {
      message('add-feedback', (currentLocale === 'zh' ? '共解析出 ' + merged.urls.length + ' 条唯一链接。每次最多提交 200 条；未截断。' : 'Found ' + merged.urls.length + ' unique URLs. Keep up to 200 for this batch; no links were dropped.'), true);
    }
  } catch (error) {
    if (version === importVersion && $('add-dialog').open) message('add-feedback', error.message, true);
  } finally {
    if (version === importVersion) {
      readingFile = false;
      updateAddControls();
      $('url-file').value = '';
    }
  }
}

$('import-file').onclick = () => { $('url-file').value = ''; $('url-file').click(); };
$('url-file').onchange = () => processImportedFile($('url-file').files[0]);

// Drag and drop file onto textarea area
const dropTarget = $('drop-target');
['dragenter', 'dragover'].forEach(eventName => {
  dropTarget.addEventListener(eventName, e => {
    e.preventDefault();
    dropTarget.classList.add('drag-over');
  }, false);
});
['dragleave', 'drop'].forEach(eventName => {
  dropTarget.addEventListener(eventName, e => {
    e.preventDefault();
    dropTarget.classList.remove('drag-over');
  }, false);
});
dropTarget.addEventListener('drop', e => {
  const dt = e.dataTransfer;
  const file = dt.files[0];
  if (file) processImportedFile(file);
});

$('add-dialog').onclose = () => {
  importVersion++;
  readingFile = false;
  $('url-file').value = '';
  updateAddControls();
};

$('urls').onkeydown = event => {
  if ((event.metaKey || event.ctrlKey) && event.key === 'Enter') {
    event.preventDefault();
    $('add-form').requestSubmit();
  }
};

$('add-form').onsubmit = async event => {
  event.preventDefault();
  if (submitting || readingFile) return;
  const lines = inputLines();
  if (!lines.length || lines.length > 200) {
    message('add-feedback', currentLocale === 'zh' ? '请输入 1–200 条有效的 HTTP/HTTPS 链接。支持粘贴聊天记录或导入 TXT/CSV。' : 'Enter 1–200 valid HTTP/HTTPS URLs. You can paste chat history or import TXT / CSV.', true);
    return;
  }
  const body = JSON.stringify({ urls: lines.join('\n'), headers: state.headers });
  if (new TextEncoder().encode(body).length > 128 * 1024) {
    message('add-feedback', currentLocale === 'zh' ? '本次提交超出请求体积上限，请减少单次链接数量。' : 'This batch exceeds the server request size. Submit fewer or shorter URLs.', true);
    return;
  }
  submitting = true;
  updateAddControls();
  message('add-feedback', t('notice.adding'));
  try {
    const result = await jsonApi('/tasks', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body });
    const summary = currentLocale === 'zh' ? '成功添加 ' + result.added.length + ' 个下载任务' + (result.skipped.length ? '，跳过已存在 ' + result.skipped.length + ' 个' : '') : 'Added ' + result.added.length + (result.added.length === 1 ? ' task' : ' tasks') + (result.skipped.length ? ', duplicates skipped: ' + result.skipped.length : '');
    if (result.rejected.length) {
      $('urls').value = result.rejected.map(item => item.url).join('\n');
      $('urls').oninput();
      message('add-feedback', summary + '. ' + result.rejected.map(rejectionReason).join('; '), true);
    } else {
      $('urls').value = '';
      $('urls').oninput();
      $('add-dialog').close();
      showNotice(summary);
    }
    await refresh();
  } catch (error) {
    message('add-feedback', error.message, true);
  } finally {
    submitting = false;
    updateAddControls();
    if ($('add-dialog').open && document.activeElement === document.body) $('urls').focus();
  }
};

// ==========================================================================
// 8. Logs Dialog
// ==========================================================================

async function openLog(task) {
  const version = ++logVersion;
  logTaskId = task.id;
  $('log-title').textContent = (currentLocale === 'zh' ? '任务日志' : 'Task Logs') + ' · #' + task.id;
  $('log-content').textContent = currentLocale === 'zh' ? '正在载入日志…' : 'Loading logs…';
  $('log-dialog').showModal();
  try {
    const response = await api('/tasks/' + task.id + '/log');
    const text = await response.text();
    if (version === logVersion && $('log-dialog').open) $('log-content').textContent = text;
  } catch (error) {
    if (version === logVersion && $('log-dialog').open) $('log-content').textContent = error.message;
  }
}

// ==========================================================================
// 9. Settings Handling
// ==========================================================================

function headerText() { return Object.keys(state.headers).length ? JSON.stringify(state.headers, null, 2) : ''; }

function downloadDraft() {
  return {
    output: $('output').value.trim(),
    workers: Number($('workers').value),
    threads: Number($('threads').value),
    retries: Number($('retries').value),
    cpu_cores: Number($('cpu_cores').value)
  };
}

function downloadDirty() {
  if (!state.downloadSettings) return false;
  const draft = downloadDraft();
  return Object.keys(draft).some(key => draft[key] !== state.downloadSettings[key]);
}

function fillDownloadSettings() {
  if (!state.downloadSettings) return;
  for (const [key, value] of Object.entries(state.downloadSettings)) $(key).value = value;
  $('output').title = state.downloadSettings.output;
}

function updateDownloadButtons() {
  const disabled = savingDownloads || !state.downloadSettings;
  for (const id of ['output', 'workers', 'threads', 'retries']) $(id).disabled = disabled;
  $('cpu_cores').disabled = disabled || !state.info?.cpu_limit_supported;
  $('download-form').setAttribute('aria-busy', String(savingDownloads));
  $('save-download').disabled = disabled || !downloadDirty();
  $('reset-download').disabled = disabled || !downloadDirty();
}

for (const id of ['output', 'workers', 'threads', 'retries', 'cpu_cores']) {
  $(id).oninput = () => { message('download-feedback', ''); updateDownloadButtons(); };
}

$('reset-download').onclick = () => {
  fillDownloadSettings();
  message('download-feedback', '');
  updateDownloadButtons();
  $('output').focus();
};

$('download-form').onsubmit = async event => {
  event.preventDefault();
  if (savingDownloads || !state.downloadSettings) return;
  const cpuChanged = downloadDraft().cpu_cores !== state.downloadSettings.cpu_cores;
  savingDownloads = true;
  updateDownloadButtons();
  message('download-feedback', currentLocale === 'zh' ? '正在保存…' : 'Saving…');
  try {
    const settings = await jsonApi('/settings', { method: 'PUT', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(downloadDraft()) });
    state.downloadSettings = settings;
    state.info = { ...state.info, ...settings };
    state.epoch++;
    fillDownloadSettings();
    message('download-feedback', cpuChanged ? (currentLocale === 'zh' ? '设置已保存。CPU 限制已即时应用，其余参数对新任务生效。' : 'Saved. CPU limit applied; other settings apply to new tasks.') : (currentLocale === 'zh' ? '设置已保存，新任务将使用此配置。' : 'Saved. Other settings apply to new tasks.'));
    await refresh();
  } catch (error) {
    message('download-feedback', error.message, true);
  } finally {
    savingDownloads = false;
    updateDownloadButtons();
    if (!$('settings-view').hidden && !$('panel-download').hidden && document.activeElement === document.body) $('output').focus();
  }
};

function networkDirty() { return $('token').value.trim() !== state.token || $('headers').value !== headerText(); }

function updateNetworkButtons() {
  const dirty = networkDirty();
  $('token').disabled = savingNetwork;
  $('headers').disabled = savingNetwork;
  $('network-form').setAttribute('aria-busy', String(savingNetwork));
  $('save-network').disabled = savingNetwork || !dirty;
  $('reset-network').disabled = savingNetwork || !dirty;
}

function resetNetwork() {
  $('token').value = state.token;
  $('headers').value = headerText();
  message('network-feedback', '');
  updateNetworkButtons();
}

for (const id of ['token', 'headers']) {
  $(id).oninput = () => { message('network-feedback', ''); updateNetworkButtons(); };
}

$('network-form').onsubmit = async event => {
  event.preventDefault();
  if (savingNetwork) return;
  let headers;
  try {
    headers = $('headers').value.trim() ? JSON.parse($('headers').value) : {};
    if (!headers || Array.isArray(headers) || typeof headers !== 'object' || Object.values(headers).some(value => typeof value !== 'string')) {
      throw new Error(currentLocale === 'zh' ? '请求头必须是包含字符串值的合法 JSON 对象。' : 'Headers must be a JSON object with string values.');
    }
  } catch (error) {
    message('network-feedback', error.message, true);
    return;
  }
  const token = $('token').value.trim();
  savingNetwork = true;
  updateNetworkButtons();
  message('network-feedback', currentLocale === 'zh' ? '正在校验连接…' : 'Checking connection…');
  try {
    const info = await jsonApi('/info', { token });
    state.token = token;
    state.headers = headers;
    state.info = info;
    state.epoch++;
    $('token').value = state.token;
    $('headers').value = headerText();
    renderInfo();
    setConnection(true);
    if ($('notice').dataset.kind === 'connection') showNotice('');
    const remembered = rememberToken(token);
    message('network-feedback', remembered ? (currentLocale === 'zh' ? 'Token 已记忆在当前浏览器中。请求头仅保留在当前标签页。' : 'Token remembered in this browser. Headers kept for this tab only.') : (currentLocale === 'zh' ? '已连接，但浏览器未能持久保存 Token。' : 'Connected, but this browser could not remember the token. Re-enter it after reloading.'), !remembered);
    await refresh();
  } catch (error) {
    message('network-feedback', error.message, true);
  } finally {
    savingNetwork = false;
    updateNetworkButtons();
    if (!$('settings-view').hidden && !$('panel-network').hidden && document.activeElement === document.body) $('token').focus();
  }
};

$('reset-network').onclick = resetNetwork;

// ==========================================================================
// 10. Navigation & Global Event Handlers
// ==========================================================================

$('nav-downloads').onclick = () => setView('downloads');
$('nav-settings').onclick = () => setView('settings');
$('back-downloads').onclick = () => setView('downloads');
$('brand').onclick = event => { event.preventDefault(); setView('downloads'); };
$('notice-settings').onclick = () => { setView('settings'); setSettingsTab('network'); $('token').focus(); };
$('dismiss-notice').onclick = () => showNotice('');

$('collapse-nav').onclick = () => {
  const collapsed = $('app-shell').classList.toggle('is-collapsed');
  $('collapse-nav').setAttribute('aria-expanded', String(!collapsed));
  const label = collapsed ? (currentLocale === 'zh' ? '展开导航栏' : 'Expand sidebar') : (currentLocale === 'zh' ? '折叠导航栏' : 'Collapse sidebar');
  $('collapse-nav').setAttribute('aria-label', label);
  $('collapse-nav').title = label;
};

$('new-task').onclick = openAdd;
$('refresh').onclick = refresh;

document.querySelectorAll('[data-filter]').forEach(button => button.onclick = () => {
  state.filter = button.dataset.filter;
  document.querySelectorAll('[data-filter]').forEach(b => b.setAttribute('aria-pressed', String(b === button)));
  renderTasks();
});

const settingsTabs = [...document.querySelectorAll('[data-setting]')];
settingsTabs.forEach((button, index) => {
  button.onclick = () => setSettingsTab(button.dataset.setting);
  button.onkeydown = event => {
    let target;
    if (event.key === 'ArrowRight') target = settingsTabs[(index + 1) % settingsTabs.length];
    if (event.key === 'ArrowLeft') target = settingsTabs[(index - 1 + settingsTabs.length) % settingsTabs.length];
    if (event.key === 'Home') target = settingsTabs[0];
    if (event.key === 'End') target = settingsTabs[settingsTabs.length - 1];
    if (target) { event.preventDefault(); setSettingsTab(target.dataset.setting); target.focus(); }
  };
});

document.querySelectorAll('[data-close-dialog]').forEach(button => {
  button.onclick = () => $(button.dataset.closeDialog).close();
});

for (const id of ['add-dialog', 'log-dialog', 'delete-dialog']) {
  $(id).onclick = event => {
    const rect = $(id).getBoundingClientRect();
    if (event.target === $(id) && (event.clientX < rect.left || event.clientX > rect.right || event.clientY < rect.top || event.clientY > rect.bottom)) {
      $(id).close();
    }
  };
}

$('log-dialog').onclose = () => {
  logVersion++;
  renderTasks();
  restoreTaskFocus(logTaskId, 'log');
};

// Theme switcher clicks
document.querySelectorAll('[data-theme-val]').forEach(btn => {
  btn.onclick = () => applyTheme(btn.dataset.themeVal);
});

// Language switcher clicks
document.querySelectorAll('[data-lang-val]').forEach(btn => {
  btn.onclick = () => applyLocale(btn.dataset.langVal);
});

// ==========================================================================
// 11. Initialization
// ==========================================================================

$('service-address').textContent = location.origin;
applyTheme(currentTheme);
applyLocale(currentLocale);
resetNetwork();
setInterval(refresh, 3000);
refresh();
