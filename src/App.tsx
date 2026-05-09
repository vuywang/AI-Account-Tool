import { invoke } from "@tauri-apps/api/core";
import {
  CircleCheck,
  Copy,
  FolderOpen,
  KeyRound,
  Plus,
  RefreshCcw,
  Trash2,
  UserRound,
} from "lucide-react";
import { FormEvent, useEffect, useMemo, useRef, useState } from "react";

type AuthMode = "oauth" | "apikey";

type CodexQuota = {
  hourlyPercentage: number;
  hourlyResetTime?: number | null;
  hourlyWindowMinutes?: number | null;
  weeklyPercentage: number;
  weeklyResetTime?: number | null;
  weeklyWindowMinutes?: number | null;
  codeReviewPercentage?: number | null;
  codeReviewResetTime?: number | null;
  codeReviewWindowMinutes?: number | null;
};

type CodexQuotaError = {
  message: string;
  timestamp: number;
};

type CodexAccount = {
  id: string;
  label: string;
  email: string;
  authMode: AuthMode;
  apiBaseUrl?: string | null;
  accountId?: string | null;
  organizationId?: string | null;
  planType?: string | null;
  subscriptionActiveUntil?: number | null;
  hasApiKey: boolean;
  hasRefreshToken: boolean;
  quota?: CodexQuota | null;
  quotaError?: CodexQuotaError | null;
  usageUpdatedAt?: number | null;
  createdAt: number;
  lastUsed: number;
};

type ApiChannel = {
  id: string;
  name: string;
  baseUrl: string;
  keyPreview: string;
  hasApiKey: boolean;
  createdAt: number;
  updatedAt: number;
  lastUsed?: number | null;
};

type AppState = {
  dataDir: string;
  defaultCodexHome: string;
  storePath: string;
  apiChannelsPath: string;
  currentAccountId?: string | null;
  currentApiChannelId?: string | null;
  accounts: CodexAccount[];
  apiChannels: ApiChannel[];
  codexCli: {
    path?: string | null;
    version?: string | null;
    source: string;
  };
  windowsTerminalAvailable: boolean;
};

type ApiChannelForm = {
  name: string;
  baseUrl: string;
  apiKey: string;
};

const emptyApiChannelForm: ApiChannelForm = {
  name: "",
  baseUrl: "",
  apiKey: "",
};

function formatDate(timestamp?: number | null) {
  if (!timestamp) return "尚未使用";
  return new Intl.DateTimeFormat("zh-CN", {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  }).format(new Date(timestamp * 1000));
}

function formatDateTime(timestamp?: number | null) {
  if (!timestamp) return "未知";
  return new Intl.DateTimeFormat("zh-CN", {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  }).format(new Date(timestamp * 1000));
}

function formatReset(timestamp?: number | null) {
  if (!timestamp) return "未知";
  return new Intl.DateTimeFormat("zh-CN", {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  }).format(new Date(timestamp * 1000));
}

function quotaTone(value?: number | null) {
  if (value == null) return "neutral";
  if (value <= 20) return "low";
  if (value <= 55) return "mid";
  return "high";
}

function windowLabel(minutes?: number | null, fallback = "5h") {
  if (!minutes) return fallback;
  if (minutes >= 60 * 24) return `${Math.round(minutes / 60 / 24)}d`;
  if (minutes >= 60) return `${Math.round(minutes / 60)}h`;
  return `${minutes}m`;
}

function authLabel(mode: AuthMode) {
  return mode === "apikey" ? "API Key" : "OAuth";
}

function compactPath(path?: string | null) {
  if (!path) return "未设置";
  if (path.length < 62) return path;
  return `${path.slice(0, 24)}...${path.slice(-30)}`;
}

function QuotaMeter({
  label,
  value,
  reset,
  windowMinutes,
  fallbackWindow,
}: {
  label: string;
  value?: number | null;
  reset?: number | null;
  windowMinutes?: number | null;
  fallbackWindow: string;
}) {
  const displayValue = value ?? 0;
  const tone = quotaTone(value);
  return (
    <div className={`quota-meter ${tone}`}>
      <div className="quota-line">
        <span>{label}</span>
        <strong>{value == null ? "--" : `${value}%`}</strong>
      </div>
      <div className="quota-track">
        <div style={{ width: `${Math.max(0, Math.min(100, displayValue))}%` }} />
      </div>
      <small>
        {windowLabel(windowMinutes, fallbackWindow)} · 重置 {formatReset(reset)}
      </small>
    </div>
  );
}

function AccountQuota({
  account,
  onRefresh,
}: {
  account: CodexAccount;
  onRefresh: () => void;
}) {
  if (account.authMode === "apikey") {
    return <p className="quota-note">API Key 账号请在 OpenAI 控制台查看用量。</p>;
  }

  if (!account.quota) {
    return (
      <div className="quota-empty">
        <span>{account.quotaError ? account.quotaError.message : "尚未刷新额度"}</span>
        <button className="button small ghost" onClick={onRefresh}>
          <RefreshCcw size={13} /> 刷新额度
        </button>
      </div>
    );
  }

  return (
    <div className="quota-panel">
      <QuotaMeter
        label="5h"
        value={account.quota.hourlyPercentage}
        reset={account.quota.hourlyResetTime}
        windowMinutes={account.quota.hourlyWindowMinutes}
        fallbackWindow="5h"
      />
      <QuotaMeter
        label="Weekly"
        value={account.quota.weeklyPercentage}
        reset={account.quota.weeklyResetTime}
        windowMinutes={account.quota.weeklyWindowMinutes}
        fallbackWindow="7d"
      />
      <div className="quota-actions">
        <span>更新 {formatDate(account.usageUpdatedAt)}</span>
        <button className="button small ghost" onClick={onRefresh}>
          <RefreshCcw size={13} /> 刷新
        </button>
      </div>
      {account.quotaError && <small className="quota-error">{account.quotaError.message}</small>}
    </div>
  );
}

export default function App() {
  const [state, setState] = useState<AppState | null>(null);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [apiChannelForm, setApiChannelForm] = useState<ApiChannelForm>(emptyApiChannelForm);
  const [editingApiChannelId, setEditingApiChannelId] = useState<string | null>(null);
  const loginPollRef = useRef<number | null>(null);

  const currentAccount = useMemo(
    () => state?.accounts.find((account) => account.id === state.currentAccountId) ?? null,
    [state],
  );
  const currentApiChannel = useMemo(
    () => state?.apiChannels.find((channel) => channel.id === state.currentApiChannelId) ?? null,
    [state],
  );
  const currentIdentityLabel = currentApiChannel
    ? `API 中转 · ${currentApiChannel.name}`
    : currentAccount
      ? currentAccount.label
      : "未选择";

  async function runAction<T>(label: string, action: () => Promise<T>, success?: string) {
    setBusy(label);
    setError(null);
    setMessage(null);
    try {
      const result = await action();
      if (success) setMessage(success);
      return result;
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
      throw err;
    } finally {
      setBusy(null);
    }
  }

  async function refresh() {
    const next = await invoke<AppState>("get_app_state");
    setState(next);
  }

  function clearLoginPoll() {
    if (loginPollRef.current != null) {
      window.clearInterval(loginPollRef.current);
      loginPollRef.current = null;
    }
  }

  function watchLoginImport(codexHome: string) {
    clearLoginPoll();
    let importing = false;

    const tryImport = async () => {
      if (importing) return;
      importing = true;
      try {
        const next = await invoke<AppState>("import_current_codex_account", {
          codexHome,
          label: null,
        });
        setState(next);
        setError(null);
        setMessage("登录完成，已自动导入账号");
        clearLoginPoll();
      } catch {
        // 登录可能还在浏览器里进行中，继续等待 auth.json。
      } finally {
        importing = false;
      }
    };

    loginPollRef.current = window.setInterval(tryImport, 1000);
    void tryImport();
  }

  useEffect(() => {
    refresh()
      .catch((err) => setError(err instanceof Error ? err.message : String(err)))
      .finally(() => setLoading(false));
  }, []);

  useEffect(() => () => clearLoginPoll(), []);

  async function importLocal() {
    await runAction(
      "import",
      async () => {
        const next = await invoke<AppState>("import_current_codex_account", {
          codexHome: null,
          label: null,
        });
        setState(next);
      },
      "已从默认 auth.json 导入账号",
    );
  }

  async function loginAndImport() {
    await runAction("login-import", async () => {
      const loginHome = await invoke<string>("start_codex_login", {
        codexHome: null,
      });
      setMessage("已打开 Codex 登录，登录成功后会自动导入账号");
      watchLoginImport(loginHome);
    });
  }

  async function saveApiChannel(event: FormEvent) {
    event.preventDefault();
    if (editingApiChannelId) {
      await runAction(
        `update-api-channel-${editingApiChannelId}`,
        async () => {
          const next = await invoke<AppState>("update_api_channel", {
            params: {
              channelId: editingApiChannelId,
              name: apiChannelForm.name,
              baseUrl: apiChannelForm.baseUrl,
              apiKey: apiChannelForm.apiKey || null,
            },
          });
          setState(next);
          setApiChannelForm(emptyApiChannelForm);
          setEditingApiChannelId(null);
        },
        "已更新 API 中转",
      );
      return;
    }

    await runAction(
      "add-api-channel",
      async () => {
        const next = await invoke<AppState>("add_api_channel", { params: apiChannelForm });
        setState(next);
        setApiChannelForm(emptyApiChannelForm);
      },
      "已保存 API 中转",
    );
  }

  function editApiChannel(channel: ApiChannel) {
    setEditingApiChannelId(channel.id);
    setApiChannelForm({
      name: channel.name,
      baseUrl: channel.baseUrl,
      apiKey: "",
    });
  }

  function cancelApiChannelEdit() {
    setEditingApiChannelId(null);
    setApiChannelForm(emptyApiChannelForm);
  }

  async function switchApiChannel(channel: ApiChannel) {
    await runAction(
      `switch-api-channel-${channel.id}`,
      async () => {
        const next = await invoke<AppState>("switch_api_channel", {
          channelId: channel.id,
          codexHome: null,
        });
        setState(next);
      },
      `已切换到 API 中转 ${channel.name}`,
    );
  }

  async function deleteApiChannel(channel: ApiChannel) {
    if (!window.confirm(`删除 API 中转 ${channel.name}？已写入 Codex 的 auth.json/config.toml 不会被自动还原。`)) {
      return;
    }
    await runAction(`delete-api-channel-${channel.id}`, async () => {
      const next = await invoke<AppState>("delete_api_channel", { channelId: channel.id });
      setState(next);
      if (editingApiChannelId === channel.id) {
        cancelApiChannelEdit();
      }
    });
  }

  async function switchAccount(account: CodexAccount) {
    await runAction(
      `switch-${account.id}`,
      async () => {
        const next = await invoke<AppState>("switch_account", {
          accountId: account.id,
          codexHome: null,
        });
        setState(next);
      },
      `已切换到 ${account.label}`,
    );
  }

  async function renameAccount(account: CodexAccount) {
    const label = window.prompt("新的账号名称", account.label);
    if (!label) return;
    await runAction(`rename-${account.id}`, async () => {
      const next = await invoke<AppState>("update_account_label", {
        accountId: account.id,
        label,
      });
      setState(next);
    });
  }

  async function deleteAccount(account: CodexAccount) {
    if (!window.confirm(`删除账号 ${account.label}？本操作只删除工具内保存的账号，不会删除 Codex 数据目录。`)) {
      return;
    }
    await runAction(`delete-${account.id}`, async () => {
      const next = await invoke<AppState>("delete_account", { accountId: account.id });
      setState(next);
    });
  }

  async function refreshQuota(account: CodexAccount) {
    await runAction(
      `quota-${account.id}`,
      async () => {
        const next = await invoke<AppState>("refresh_account_quota", { accountId: account.id });
        setState(next);
        const updated = next.accounts.find((item) => item.id === account.id);
        if (updated?.quotaError) throw new Error(updated.quotaError.message);
      },
      `已刷新 ${account.label} 的额度`,
    );
  }

  async function refreshAllQuotas() {
    await runAction(
      "quota-all",
      async () => {
        const next = await invoke<AppState>("refresh_all_quotas");
        setState(next);
        const failed = next.accounts.find((item) => item.quotaError);
        if (failed?.quotaError) throw new Error(`${failed.label}: ${failed.quotaError.message}`);
      },
      "已刷新全部 OAuth 账号额度",
    );
  }

  async function openPath(path: string) {
    await runAction("open-path", () => invoke("open_path", { path }));
  }

  async function copyText(text: string, done = "已复制") {
    await navigator.clipboard.writeText(text);
    setMessage(done);
  }

  if (loading) {
    return (
      <main className="loading-screen">
        <img src="/codex.svg" alt="Codex" />
        <p>正在载入 Codex 工作台</p>
      </main>
    );
  }

  if (!state) {
    return (
      <main className="loading-screen">
        <p>工作台初始化失败</p>
        {error && <pre>{error}</pre>}
      </main>
    );
  }

  return (
    <main className="app-shell">
      <div className="shell-inner">
        <header className="topbar">
          <div className="brand">
            <img src="/codex.svg" alt="Codex" />
            <div>
              <h1>AI Account Tool</h1>
              <p>Codex 账号与 API 中转切换</p>
            </div>
          </div>
          <div className="topbar-actions">
            <button className="button ghost" onClick={() => openPath(state.dataDir)}>
              <FolderOpen size={16} /> 数据目录
            </button>
            <button className="button ghost" onClick={() => runAction("refresh", refresh)}>
              <RefreshCcw size={16} /> 刷新
            </button>
          </div>
        </header>

        {(message || error) && (
          <div className={`notice ${error ? "error" : "ok"}`}>
            {error ? error : message}
          </div>
        )}

        <section className="status-band">
          <div>
            <span>当前使用</span>
            <strong>{currentIdentityLabel}</strong>
          </div>
          <div>
            <span>默认 CODEX_HOME</span>
            <strong title={state.defaultCodexHome}>{compactPath(state.defaultCodexHome)}</strong>
          </div>
          <div>
            <span>Codex CLI</span>
            <strong>{state.codexCli.version ?? "未检测到"}</strong>
          </div>
        </section>

        <section className="workspace-section accounts-section">
          <div className="section-heading">
            <div>
              <h2>账号</h2>
              <p>导入 Codex 登录账号，查看额度并一键切换。</p>
            </div>
            <div className="heading-actions">
              <button className="button small ghost" onClick={refreshAllQuotas} disabled={busy === "quota-all"}>
                <RefreshCcw size={13} /> 刷新额度
              </button>
              <span className="count">{state.accounts.length}</span>
            </div>
          </div>

          <div className="quick-actions">
            <button className="button primary" onClick={importLocal} disabled={busy === "import"}>
              <UserRound size={16} /> 导入本机账号
            </button>
            <button className="button primary" onClick={loginAndImport} disabled={busy === "login-import"}>
              <UserRound size={16} /> 登录并导入
            </button>
          </div>

          <div className="account-list">
            {state.accounts.length === 0 && (
              <div className="empty-state">
                <KeyRound size={20} />
                <p>还没有保存账号。可以先导入本机账号，或登录后自动导入。</p>
              </div>
            )}
            {state.accounts.map((account) => (
              <article
                className={`list-row account-row ${state.currentAccountId === account.id ? "selected" : ""}`}
                key={account.id}
              >
                <div className="row-main">
                  <div className="row-title">
                    <strong>{account.label}</strong>
                    {state.currentAccountId === account.id && (
                      <span className="pill active"><CircleCheck size={13} /> 当前</span>
                    )}
                    <span className="pill">{authLabel(account.authMode)}</span>
                  </div>
                  <p>{account.email}</p>
                  <small>
                    {account.planType ?? "Codex"}
                    {account.subscriptionActiveUntil
                      ? ` · 订阅到期 ${formatDateTime(account.subscriptionActiveUntil)}`
                      : ""}
                    {" · "}最近使用 {formatDate(account.lastUsed)}
                    {account.apiBaseUrl ? ` · ${account.apiBaseUrl}` : ""}
                  </small>
                  <AccountQuota account={account} onRefresh={() => refreshQuota(account)} />
                </div>
                <div className="row-actions">
                  <button className="button small" onClick={() => switchAccount(account)}>
                    切换
                  </button>
                  <button className="button small ghost" onClick={() => renameAccount(account)}>
                    改名
                  </button>
                  <button className="button small danger" onClick={() => deleteAccount(account)}>
                    <Trash2 size={14} />
                  </button>
                </div>
              </article>
            ))}
          </div>
        </section>

        <section className="workspace-section api-channel-section">
          <div className="section-heading">
            <div>
              <h2>API 中转</h2>
              <p>保存 URL 和 API Key，切换时自动写入默认 CODEX_HOME。</p>
            </div>
            <div className="heading-actions">
              <button
                className="button small ghost"
                onClick={() => copyText(state.apiChannelsPath, "API 中转文件路径已复制")}
                type="button"
              >
                <Copy size={13} /> 复制路径
              </button>
              <span className="count">{state.apiChannels.length}</span>
            </div>
          </div>

          <form className="api-channel-form" onSubmit={saveApiChannel}>
            <label>
              <span>名称</span>
              <input
                value={apiChannelForm.name}
                onChange={(event) => setApiChannelForm({ ...apiChannelForm, name: event.target.value })}
                placeholder="inroi"
                required
              />
            </label>
            <label>
              <span>Base URL</span>
              <input
                value={apiChannelForm.baseUrl}
                onChange={(event) => setApiChannelForm({ ...apiChannelForm, baseUrl: event.target.value })}
                placeholder="https://www.inroi.shop"
                required
              />
            </label>
            <label>
              <span>API Key</span>
              <input
                value={apiChannelForm.apiKey}
                onChange={(event) => setApiChannelForm({ ...apiChannelForm, apiKey: event.target.value })}
                type="password"
                placeholder={editingApiChannelId ? "留空则保留原 Key" : "sk-..."}
                required={!editingApiChannelId}
              />
            </label>
            <div className="api-channel-buttons">
              <button className="button primary">
                <Plus size={16} /> {editingApiChannelId ? "更新中转" : "保存中转"}
              </button>
              {editingApiChannelId && (
                <button type="button" className="button ghost" onClick={cancelApiChannelEdit}>
                  取消
                </button>
              )}
            </div>
          </form>

          <div className="api-channel-list">
            {state.apiChannels.length === 0 && (
              <div className="empty-state">
                <KeyRound size={20} />
                <p>还没有 API 中转。保存一个 URL 和 API Key 后，就可以一键切换。</p>
              </div>
            )}
            {state.apiChannels.map((channel) => (
              <article
                className={`list-row api-channel-row ${state.currentApiChannelId === channel.id ? "selected" : ""}`}
                key={channel.id}
              >
                <div className="row-main">
                  <div className="row-title">
                    <strong>{channel.name}</strong>
                    {state.currentApiChannelId === channel.id && (
                      <span className="pill active"><CircleCheck size={13} /> 当前</span>
                    )}
                    <span className="pill">openai</span>
                  </div>
                  <p>{channel.baseUrl}</p>
                  <small>
                    Key {channel.keyPreview} · 最近使用 {formatDate(channel.lastUsed)}
                  </small>
                </div>
                <div className="row-actions">
                  <button className="button small primary" onClick={() => switchApiChannel(channel)}>
                    切换
                  </button>
                  <button className="button small ghost" onClick={() => editApiChannel(channel)}>
                    编辑
                  </button>
                  <button className="button small danger" onClick={() => deleteApiChannel(channel)}>
                    <Trash2 size={14} />
                  </button>
                </div>
              </article>
            ))}
          </div>
        </section>
      </div>
    </main>
  );
}
