import { invoke } from "@tauri-apps/api/core";
import {
  CircleCheck,
  Copy,
  FolderOpen,
  KeyRound,
  Play,
  Plus,
  RefreshCcw,
  Terminal,
  Trash2,
  UserRound,
} from "lucide-react";
import { FormEvent, useEffect, useMemo, useRef, useState } from "react";

type AuthMode = "oauth" | "apikey";

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

type CodexInstance = {
  id: string;
  name: string;
  codexHome: string;
  workingDir?: string | null;
  extraArgs: string;
  bindAccountId?: string | null;
  createdAt: number;
  lastLaunchedAt?: number | null;
  lastPid?: number | null;
  running: boolean;
  initialized: boolean;
  isDefault: boolean;
  launchCommand: string;
};

type AppState = {
  dataDir: string;
  defaultCodexHome: string;
  storePath: string;
  currentAccountId?: string | null;
  accounts: CodexAccount[];
  instances: CodexInstance[];
  codexCli: {
    path?: string | null;
    version?: string | null;
    source: string;
  };
  windowsTerminalAvailable: boolean;
};

type ApiKeyForm = {
  label: string;
  email: string;
  apiKey: string;
  apiBaseUrl: string;
};

type TokenForm = {
  label: string;
  idToken: string;
  accessToken: string;
  refreshToken: string;
  accountId: string;
};

type InstanceForm = {
  name: string;
  codexHome: string;
  workingDir: string;
  extraArgs: string;
  bindAccountId: string;
};

const emptyApiKeyForm: ApiKeyForm = {
  label: "",
  email: "",
  apiKey: "",
  apiBaseUrl: "",
};

const emptyTokenForm: TokenForm = {
  label: "",
  idToken: "",
  accessToken: "",
  refreshToken: "",
  accountId: "",
};

const emptyInstanceForm: InstanceForm = {
  name: "",
  codexHome: "",
  workingDir: "",
  extraArgs: "",
  bindAccountId: "",
};

function formatDate(timestamp?: number | null) {
  if (!timestamp) return "尚未启动";
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
  if (path.length < 58) return path;
  return `${path.slice(0, 22)}...${path.slice(-28)}`;
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
      <small>{windowLabel(windowMinutes, fallbackWindow)} · 重置 {formatReset(reset)}</small>
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
  const [importHome, setImportHome] = useState("");
  const [importLabel, setImportLabel] = useState("");
  const [apiKeyForm, setApiKeyForm] = useState<ApiKeyForm>(emptyApiKeyForm);
  const [tokenForm, setTokenForm] = useState<TokenForm>(emptyTokenForm);
  const [instanceForm, setInstanceForm] = useState<InstanceForm>(emptyInstanceForm);
  const loginPollRef = useRef<number | null>(null);

  const currentAccount = useMemo(
    () => state?.accounts.find((account) => account.id === state.currentAccountId) ?? null,
    [state],
  );

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

  function watchLoginImport(codexHome: string, label: string | null) {
    clearLoginPoll();
    let importing = false;

    const tryImport = async () => {
      if (importing) return;
      importing = true;
      try {
        const next = await invoke<AppState>("import_current_codex_account", {
          codexHome,
          label,
        });
        setState(next);
        setImportLabel("");
        setError(null);
        setMessage("登录完成，已自动导入账号");
        clearLoginPoll();
      } catch {
        // 登录可能还在浏览器里进行中；失败不打扰界面，继续等 auth.json。
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

  async function importLocal(event: FormEvent) {
    event.preventDefault();
    await runAction(
      "import",
      async () => {
        const next = await invoke<AppState>("import_current_codex_account", {
          codexHome: importHome || null,
          label: importLabel || null,
        });
        setState(next);
        setImportLabel("");
      },
      "已从 auth.json 导入账号",
    );
  }

  async function loginAndImport() {
    await runAction(
      "login-import",
      async () => {
        const label = importLabel || null;
        const loginHome = await invoke<string>("start_codex_login", {
          codexHome: importHome || null,
        });
        setImportHome(loginHome);
        setMessage("已打开 Codex 登录，登录成功后会自动导入账号");
        watchLoginImport(loginHome, label);
      },
    );
  }

  async function addApiKey(event: FormEvent) {
    event.preventDefault();
    await runAction(
      "add-api-key",
      async () => {
        const next = await invoke<AppState>("add_api_key_account", { params: apiKeyForm });
        setState(next);
        setApiKeyForm(emptyApiKeyForm);
      },
      "已添加 API Key 账号",
    );
  }

  async function addToken(event: FormEvent) {
    event.preventDefault();
    await runAction(
      "add-token",
      async () => {
        const next = await invoke<AppState>("add_token_account", { params: tokenForm });
        setState(next);
        setTokenForm(emptyTokenForm);
      },
      "已添加 OAuth Token 账号",
    );
  }

  async function createInstance(event: FormEvent) {
    event.preventDefault();
    await runAction(
      "create-instance",
      async () => {
        const next = await invoke<AppState>("create_instance", {
          params: {
            name: instanceForm.name,
            codexHome: instanceForm.codexHome || null,
            workingDir: instanceForm.workingDir || null,
            extraArgs: instanceForm.extraArgs || null,
            bindAccountId: instanceForm.bindAccountId || null,
          },
        });
        setState(next);
        setInstanceForm(emptyInstanceForm);
      },
      "已创建 Codex 实例",
    );
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

  async function launchInstance(instance: CodexInstance) {
    await runAction(
      `launch-${instance.id}`,
      async () => {
        await invoke("launch_instance", { instanceId: instance.id });
        await refresh();
      },
      `已启动 ${instance.name}`,
    );
  }

  async function stopInstance(instance: CodexInstance) {
    await runAction(`stop-${instance.id}`, async () => {
      const next = await invoke<AppState>("stop_instance", { instanceId: instance.id });
      setState(next);
    });
  }

  async function deleteInstance(instance: CodexInstance) {
    if (!window.confirm(`删除实例 ${instance.name}？实例目录不会被删除。`)) return;
    await runAction(`delete-instance-${instance.id}`, async () => {
      const next = await invoke<AppState>("delete_instance", { instanceId: instance.id });
      setState(next);
    });
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
      <header className="topbar">
        <div className="brand">
          <img src="/codex.svg" alt="Codex" />
          <div>
            <h1>AI Account Tool</h1>
            <p>Codex 账号和 Windows 多实例</p>
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
          <span>当前账号</span>
          <strong>{currentAccount ? currentAccount.label : "未选择"}</strong>
        </div>
        <div>
          <span>默认 CODEX_HOME</span>
          <strong title={state.defaultCodexHome}>{compactPath(state.defaultCodexHome)}</strong>
        </div>
        <div>
          <span>Codex CLI</span>
          <strong>{state.codexCli.version ?? "未检测到"}</strong>
        </div>
        <div>
          <span>终端</span>
          <strong>{state.windowsTerminalAvailable ? "Windows Terminal" : "cmd.exe"}</strong>
        </div>
      </section>

      <div className="workspace-grid">
        <section className="workspace-section accounts-section">
          <div className="section-heading">
            <div>
              <h2>账号</h2>
              <p>从现有 Codex 登录导入，或保存 API Key / OAuth Token。</p>
            </div>
            <div className="heading-actions">
              <button className="button small ghost" onClick={refreshAllQuotas} disabled={busy === "quota-all"}>
                <RefreshCcw size={13} /> 刷新额度
              </button>
              <span className="count">{state.accounts.length}</span>
            </div>
          </div>

          <form className="inline-form" onSubmit={importLocal}>
            <label>
              <span>CODEX_HOME</span>
              <input
                value={importHome}
                onChange={(event) => setImportHome(event.target.value)}
                placeholder={state.defaultCodexHome}
              />
            </label>
            <label>
              <span>名称</span>
              <input
                value={importLabel}
                onChange={(event) => setImportLabel(event.target.value)}
                placeholder="可选"
              />
            </label>
            <button className="button primary" disabled={busy === "import"}>
              <UserRound size={16} /> 导入本机账号
            </button>
            <button type="button" className="button primary" onClick={loginAndImport} disabled={busy === "login-import"}>
              <UserRound size={16} /> 登录并导入
            </button>
            <p className="form-hint">留空 CODEX_HOME 时会使用独立登录目录，登录成功后自动保存 refresh_token。</p>
          </form>

          <div className="account-list">
            {state.accounts.length === 0 && (
              <div className="empty-state">
                <KeyRound size={20} />
                <p>还没有保存账号。先导入默认 `auth.json`，或者添加一个 API Key。</p>
              </div>
            )}
            {state.accounts.map((account) => (
              <article className="list-row" key={account.id}>
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

          <details className="form-disclosure">
            <summary>添加 API Key 账号</summary>
            <form className="stack-form" onSubmit={addApiKey}>
              <label>
                <span>名称</span>
                <input value={apiKeyForm.label} onChange={(event) => setApiKeyForm({ ...apiKeyForm, label: event.target.value })} placeholder="工作账号" />
              </label>
              <label>
                <span>显示邮箱</span>
                <input value={apiKeyForm.email} onChange={(event) => setApiKeyForm({ ...apiKeyForm, email: event.target.value })} placeholder="可选" />
              </label>
              <label>
                <span>OPENAI_API_KEY</span>
                <input value={apiKeyForm.apiKey} onChange={(event) => setApiKeyForm({ ...apiKeyForm, apiKey: event.target.value })} type="password" required />
              </label>
              <label>
                <span>Base URL</span>
                <input value={apiKeyForm.apiBaseUrl} onChange={(event) => setApiKeyForm({ ...apiKeyForm, apiBaseUrl: event.target.value })} placeholder="https://api.openai.com/v1" />
              </label>
              <button className="button primary"><Plus size={16} /> 保存 API Key</button>
            </form>
          </details>

          <details className="form-disclosure">
            <summary>手动添加 OAuth Token</summary>
            <form className="stack-form" onSubmit={addToken}>
              <label>
                <span>名称</span>
                <input value={tokenForm.label} onChange={(event) => setTokenForm({ ...tokenForm, label: event.target.value })} placeholder="可选" />
              </label>
              <label>
                <span>id_token</span>
                <textarea value={tokenForm.idToken} onChange={(event) => setTokenForm({ ...tokenForm, idToken: event.target.value })} required />
              </label>
              <label>
                <span>access_token</span>
                <textarea value={tokenForm.accessToken} onChange={(event) => setTokenForm({ ...tokenForm, accessToken: event.target.value })} required />
              </label>
              <label>
                <span>refresh_token</span>
                <textarea value={tokenForm.refreshToken} onChange={(event) => setTokenForm({ ...tokenForm, refreshToken: event.target.value })} />
              </label>
              <label>
                <span>account_id</span>
                <input value={tokenForm.accountId} onChange={(event) => setTokenForm({ ...tokenForm, accountId: event.target.value })} placeholder="可选" />
              </label>
              <button className="button primary"><Plus size={16} /> 保存 Token</button>
            </form>
          </details>
        </section>

        <section className="workspace-section instances-section">
          <div className="section-heading">
            <div>
              <h2>实例</h2>
              <p>每个实例使用独立 CODEX_HOME，可绑定不同账号并行启动。</p>
            </div>
            <span className="count">{state.instances.length}</span>
          </div>

          <form className="instance-form" onSubmit={createInstance}>
            <label>
              <span>实例名称</span>
              <input
                value={instanceForm.name}
                onChange={(event) => setInstanceForm({ ...instanceForm, name: event.target.value })}
                placeholder="客户 A / 测试环境"
                required
              />
            </label>
            <label>
              <span>绑定账号</span>
              <select
                value={instanceForm.bindAccountId}
                onChange={(event) => setInstanceForm({ ...instanceForm, bindAccountId: event.target.value })}
              >
                <option value="">不绑定</option>
                {state.accounts.map((account) => (
                  <option value={account.id} key={account.id}>{account.label}</option>
                ))}
              </select>
            </label>
            <label>
              <span>CODEX_HOME</span>
              <input
                value={instanceForm.codexHome}
                onChange={(event) => setInstanceForm({ ...instanceForm, codexHome: event.target.value })}
                placeholder="留空自动创建"
              />
            </label>
            <label>
              <span>工作目录</span>
              <input
                value={instanceForm.workingDir}
                onChange={(event) => setInstanceForm({ ...instanceForm, workingDir: event.target.value })}
                placeholder="可选"
              />
            </label>
            <label>
              <span>启动参数</span>
              <input
                value={instanceForm.extraArgs}
                onChange={(event) => setInstanceForm({ ...instanceForm, extraArgs: event.target.value })}
                placeholder="例如 --model gpt-5.4"
              />
            </label>
            <button className="button primary"><Plus size={16} /> 新建实例</button>
          </form>

          <div className="instance-list">
            {state.instances.map((instance) => {
              const bound = state.accounts.find((account) => account.id === instance.bindAccountId);
              return (
                <article className="list-row instance-row" key={instance.id}>
                  <div className="row-main">
                    <div className="row-title">
                      <strong>{instance.name}</strong>
                      {instance.isDefault && <span className="pill">默认</span>}
                      {instance.running && <span className="pill active">运行中</span>}
                      {!instance.initialized && <span className="pill warn">未初始化</span>}
                    </div>
                    <p title={instance.codexHome}>{compactPath(instance.codexHome)}</p>
                    <small>
                      账号 {bound?.label ?? "未绑定"} · {instance.workingDir ? `工作目录 ${compactPath(instance.workingDir)}` : "未设置工作目录"} · {formatDate(instance.lastLaunchedAt)}
                    </small>
                    <code>{instance.launchCommand}</code>
                  </div>
                  <div className="row-actions">
                    <button className="button small primary" onClick={() => launchInstance(instance)}>
                      <Play size={14} /> 启动
                    </button>
                    {!instance.isDefault && (
                      <button className="button small ghost" onClick={() => stopInstance(instance)}>
                        停止
                      </button>
                    )}
                    <button className="button small ghost" onClick={() => openPath(instance.codexHome)}>
                      <FolderOpen size={14} />
                    </button>
                    <button className="button small ghost" onClick={() => copyText(instance.launchCommand, "启动命令已复制")}>
                      <Copy size={14} />
                    </button>
                    {!instance.isDefault && (
                      <button className="button small danger" onClick={() => deleteInstance(instance)}>
                        <Trash2 size={14} />
                      </button>
                    )}
                  </div>
                </article>
              );
            })}
          </div>
        </section>
      </div>

      <footer className="footer-bar">
        <span title={state.codexCli.path ?? ""}>
          <Terminal size={15} /> {state.codexCli.path ? compactPath(state.codexCli.path) : "未找到 codex.cmd"}
        </span>
        <button className="button small ghost" onClick={() => copyText(state.storePath, "数据文件路径已复制")}>
          复制数据文件路径
        </button>
      </footer>
    </main>
  );
}
