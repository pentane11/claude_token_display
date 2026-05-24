// バンドラを使わない構成のため Tauri グローバル経由
const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;
const currentWindow = window.__TAURI__.window.getCurrentWindow();

const $ = (sel) => document.querySelector(sel);
const TEXT_SCALE_KEY = "token_display_text_scale";
const TEXT_SCALE_MIN = 0.75;
const TEXT_SCALE_MAX = 1.3;
const TEXT_SCALE_STEP = 0.1;
let isPinned = false;
let textScale = 1;

function levelOf(util) {
  if (util < 0.5) return "low";
  if (util < 0.85) return "mid";
  return "high";
}

function formatResetIn(isoString) {
  if (!isoString) return "—";
  const resets = new Date(isoString);
  const now = new Date();
  const diffMs = resets - now;
  if (diffMs <= 0) {
    return "リセット中";
  }
  const mins = Math.floor(diffMs / 60000);
  const hours = Math.floor(mins / 60);
  const remainMins = mins % 60;

  // 24h 以内は「X時間Y分後」、それ以上は曜日と時刻
  if (mins < 60 * 24) {
    if (hours === 0) return `${remainMins}分後にリセット`;
    return `${hours}時間${remainMins}分後にリセット`;
  }
  const wday = ["日", "月", "火", "水", "木", "金", "土"][resets.getDay()];
  const hh = String(resets.getHours()).padStart(2, "0");
  const mm = String(resets.getMinutes()).padStart(2, "0");
  return `${hh}:${mm} (${wday})にリセット`;
}

function renderMissingBucket(selector, hideWhenMissing) {
  const section = $(selector);
  if (!section) return;
  if (hideWhenMissing) {
    section.hidden = true;
    return;
  }

  section.hidden = false;
  section.querySelector("[data-pct]").textContent = "—";
  section.querySelector("[data-resets]").textContent = "取得待ち";
  const fill = section.querySelector("[data-fill]");
  fill.style.width = "0%";
  fill.dataset.level = "low";
}

function renderBucket(selector, bucket, { hideWhenMissing = true } = {}) {
  const section = $(selector);
  if (!section) return;
  if (!bucket) {
    renderMissingBucket(selector, hideWhenMissing);
    return;
  }

  section.hidden = false;
  const rawUtil = Number(bucket.utilization);
  const util = Number.isFinite(rawUtil) ? rawUtil : 0;
  const pct = Math.round(util * 100);
  section.querySelector("[data-pct]").textContent = `${pct}% 使用済み`;
  section.querySelector("[data-resets]").textContent = formatResetIn(bucket.resets_at);
  const fill = section.querySelector("[data-fill]");
  fill.style.width = `${Math.max(0, Math.min(100, pct))}%`;
  fill.dataset.level = levelOf(util);
}

function showError(message) {
  const el = $("#error");
  el.hidden = false;
  el.textContent = message;
}

function clearError() {
  const el = $("#error");
  el.hidden = true;
  el.textContent = "";
}

function render(result) {
  if (!result) return;
  if (result.kind === "err") {
    showError(result.message || "unknown error");
    return;
  }
  if (result.kind === "rate_limited") {
    const s = result.retry_after_secs;
    showError(
      `Rate limited by Anthropic API. ${s ? `Retrying in ${s}s.` : "Retrying shortly."}`
    );
    return;
  }
  clearError();
  renderBucket("#bucket-5h", result.five_hour, { hideWhenMissing: false });
  renderBucket("#bucket-7d", result.seven_day);
  renderBucket("#bucket-7d-sonnet", result.seven_day_sonnet);

  const fetchedAt = result.fetched_at ? new Date(result.fetched_at) : new Date();
  $("#updated-at").textContent = "updated " + fetchedAt.toLocaleTimeString();
}

function renderPinned(pinned) {
  isPinned = pinned;
  document.body.dataset.pinned = String(pinned);
  currentWindow.setResizable(true).catch(() => {});
  const button = $("#pin");
  if (!button) return;
  button.textContent = pinned ? "固定中" : "固定";
  button.title = pinned ? "固定解除" : "固定表示";
  button.setAttribute("aria-label", button.title);
  button.setAttribute("aria-pressed", String(pinned));
}

function clampTextScale(scale) {
  const normalized = Number.isFinite(scale) ? scale : 1;
  return Math.min(TEXT_SCALE_MAX, Math.max(TEXT_SCALE_MIN, normalized));
}

function renderTextScale(scale) {
  textScale = clampTextScale(scale);
  document.documentElement.style.setProperty("--text-scale", textScale.toFixed(2));
  $("#font-smaller").disabled = textScale <= TEXT_SCALE_MIN;
  $("#font-larger").disabled = textScale >= TEXT_SCALE_MAX;
}

function setTextScale(scale) {
  renderTextScale(scale);
  try {
    localStorage.setItem(TEXT_SCALE_KEY, textScale.toFixed(2));
  } catch {
    // localStorage が使えない環境では現在のセッションだけ反映する。
  }
}

function initTextScale() {
  try {
    renderTextScale(parseFloat(localStorage.getItem(TEXT_SCALE_KEY) || "1"));
  } catch {
    renderTextScale(1);
  }
}

async function setPinned(pinned) {
  try {
    const current = await invoke("set_popover_pinned", { pinned });
    renderPinned(current);
  } catch (err) {
    await initPinned();
    showError(String(err));
  }
}

async function initPinned() {
  try {
    renderPinned(await invoke("get_popover_pinned"));
  } catch {
    renderPinned(false);
  }
}

async function refresh() {
  try {
    const result = await invoke("get_usage");
    render(result);
  } catch (err) {
    showError(String(err));
  }
}

function startPinnedDrag(event) {
  if (!isPinned || event.button !== 0) return;
  if (event.target.closest("button, a, input, textarea, select")) return;
  event.preventDefault();
  currentWindow.startDragging().catch(() => {});
}

async function startResize(event) {
  if (event.button !== 0) return;
  event.preventDefault();
  event.stopPropagation();
  try {
    await invoke("suppress_popover_auto_hide");
  } catch {
    // 古いビルドでコマンドがない場合でもリサイズ操作自体は試す。
  }
  currentWindow.startResizeDragging("SouthEast").catch(() => {});
}

$("#refresh").addEventListener("click", refresh);
$("#font-smaller").addEventListener("click", () => {
  setTextScale(textScale - TEXT_SCALE_STEP);
});
$("#font-larger").addEventListener("click", () => {
  setTextScale(textScale + TEXT_SCALE_STEP);
});
$("#pin").addEventListener("click", () => {
  const currentlyPinned = $("#pin").getAttribute("aria-pressed") === "true";
  setPinned(!currentlyPinned);
});
$(".card__header").addEventListener("mousedown", startPinnedDrag);
$("#resize-handle").addEventListener("mousedown", startResize);

listen("usage-updated", (event) => {
  render(event.payload);
});

// 初期ロード時に API を叩かない（backend のポーラから event が来るのを待つ）。
// プレースホルダだけ出す。
initTextScale();
initPinned();
$("#updated-at").textContent = "waiting for data…";
