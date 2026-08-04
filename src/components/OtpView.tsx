import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import { useEffect, useRef, useState, type FormEvent, type ReactNode } from "react";
import type { OtpAccount, OtpHistoryItem, OtpPreview } from "../types";

type Tab = "active" | "archived" | "history";
type BackupMode = "export" | "import";

interface BackupResult {
  path: string;
  accounts: number;
  history: number;
  skipped: number;
}

interface Props {
  initialInput?: string;
  quickInput?: string | null;
  quickSubmitToken?: number;
  onConsumeInput?: () => void;
  onQuickComplete?: () => void;
}

interface QuickAddResult {
  account: OtpAccount;
  created: boolean;
  replaced: boolean;
  removed_duplicates: number;
}

function displayCode(code: string) {
  if (code.length === 6) return `${code.slice(0, 3)} ${code.slice(3)}`;
  if (code.length === 8) return `${code.slice(0, 4)} ${code.slice(4)}`;
  return code;
}

function timeLabel(epoch: number) {
  return new Date(epoch * 1000).toLocaleString("vi-VN", {
    hour: "2-digit",
    minute: "2-digit",
    day: "2-digit",
    month: "2-digit",
  });
}

function storedDate(value: string) {
  const parsed = new Date(value.includes("T") ? value : value.replace(" ", "T"));
  return Number.isNaN(parsed.getTime()) ? value : parsed.toLocaleString("vi-VN", {
    hour: "2-digit",
    minute: "2-digit",
    day: "2-digit",
    month: "2-digit",
    year: "numeric",
  });
}

function displaySecret(secret: string) {
  return secret.match(/.{1,4}/g)?.join(" ") ?? secret;
}

function IconButton({
  title,
  active,
  danger,
  onClick,
  children,
}: {
  title: string;
  active?: boolean;
  danger?: boolean;
  onClick: () => void;
  children: ReactNode;
}) {
  return (
    <button
      type="button"
      title={title}
      aria-label={title}
      onClick={onClick}
      className={`grid h-8 w-8 place-items-center rounded-lg border text-[14px] transition-colors
        ${danger
          ? "border-red-500/20 text-red-500 hover:bg-red-500/10"
          : active
            ? "border-blue-500/30 bg-blue-500/10 text-blue-500"
            : "border-black/10 text-zinc-500 hover:bg-black/5 dark:border-white/10 dark:text-zinc-400 dark:hover:bg-white/10"}`}
    >
      {children}
    </button>
  );
}

export function OtpView({
  initialInput = "",
  quickInput = null,
  quickSubmitToken = 0,
  onConsumeInput,
  onQuickComplete,
}: Props) {
  const [tab, setTab] = useState<Tab>("active");
  const [accounts, setAccounts] = useState<OtpAccount[]>([]);
  const [history, setHistory] = useState<OtpHistoryItem[]>([]);
  const [showAdd, setShowAdd] = useState(Boolean(initialInput));
  const [name, setName] = useState("");
  const [note, setNote] = useState("");
  const [secret, setSecret] = useState(initialInput);
  const [showSecret, setShowSecret] = useState(false);
  const [preview, setPreview] = useState<OtpPreview>();
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const [copiedId, setCopiedId] = useState<number>();
  const [secretCopiedId, setSecretCopiedId] = useState<number>();
  const [revealedSecrets, setRevealedSecrets] = useState<Record<number, string>>({});
  const [secretBusyId, setSecretBusyId] = useState<number>();
  const [editingId, setEditingId] = useState<number>();
  const [editName, setEditName] = useState("");
  const [editNote, setEditNote] = useState("");
  const [deleteId, setDeleteId] = useState<number>();
  const [clearConfirm, setClearConfirm] = useState(false);
  const [backup, setBackup] = useState<{ mode: BackupMode; path: string }>();
  const [backupPassword, setBackupPassword] = useState("");
  const [backupConfirm, setBackupConfirm] = useState("");
  const [backupBusy, setBackupBusy] = useState(false);
  const [backupMessage, setBackupMessage] = useState("");
  const [quickMessage, setQuickMessage] = useState("");
  const consumed = useRef(false);
  const handledQuickSubmit = useRef(0);
  const secretRef = useRef<HTMLInputElement>(null);

  const loadAccounts = async (archived = tab === "archived") => {
    const next = await invoke<OtpAccount[]>("list_otp_accounts", { archived });
    setAccounts(next);
  };

  const loadHistory = async () => {
    const next = await invoke<OtpHistoryItem[]>("list_otp_history", { limit: 200 });
    setHistory(next);
  };

  useEffect(() => {
    if (!initialInput || consumed.current) return;
    consumed.current = true;
    setSecret(initialInput);
    setShowAdd(true);
    onConsumeInput?.();
    requestAnimationFrame(() => secretRef.current?.focus());
  }, [initialInput, onConsumeInput]);

  useEffect(() => {
    if (quickInput === null) return;
    setSecret(quickInput);
    setShowAdd(true);
    setQuickMessage(quickInput ? "Nhấn Enter để lưu và copy OTP ngay." : "Dán secret sau dấu + rồi nhấn Enter.");
  }, [quickInput]);

  useEffect(() => {
    if (!quickSubmitToken || handledQuickSubmit.current === quickSubmitToken || !quickInput) return;
    handledQuickSubmit.current = quickSubmitToken;
    setBusy(true);
    setError("");
    void invoke<QuickAddResult>("quick_add_otp_account", { input: quickInput })
      .then(async (result) => {
        const current = await invoke<OtpPreview>("copy_otp_code", { id: result.account.id });
        setCopiedId(result.account.id);
        const merged = result.removed_duplicates
          ? ` · đã gộp ${result.removed_duplicates} bản trùng`
          : "";
        setQuickMessage(`${result.created ? "Đã lưu" : "Đã có sẵn, đã đưa lên đầu"}${merged} · OTP ${displayCode(current.code)} đã được copy.`);
        setSecret("");
        setName("");
        setNote("");
        setPreview(undefined);
        setShowAdd(false);
        await loadAccounts(false);
        onQuickComplete?.();
      })
      .catch((value) => {
        setError(String(value));
        setQuickMessage("");
      })
      .finally(() => setBusy(false));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [quickSubmitToken]);

  useEffect(() => {
    setError("");
    if (tab === "history") {
      void loadHistory().catch((value) => setError(String(value)));
      return;
    }
    void loadAccounts(tab === "archived").catch((value) => setError(String(value)));
    const timer = window.setInterval(() => {
      void loadAccounts(tab === "archived").catch(() => {});
    }, 1000);
    return () => window.clearInterval(timer);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [tab]);

  useEffect(() => {
    setPreview(undefined);
    setError("");
    if (!secret.trim()) return;
    const timer = window.setTimeout(() => {
      invoke<OtpPreview>("preview_otp", { input: secret })
        .then((value) => {
          setPreview(value);
          if (!name.trim()) setName(value.account_name || value.issuer || "");
        })
        .catch((value) => setError(String(value)));
    }, 240);
    return () => window.clearTimeout(timer);
  }, [secret]);

  const addAccount = async (event: FormEvent) => {
    event.preventDefault();
    if (!secret.trim() || busy) return;
    setBusy(true);
    setError("");
    try {
      const result = await invoke<QuickAddResult>("add_otp_account", { name, note, input: secret });
      const merged = result.removed_duplicates
        ? ` và gộp ${result.removed_duplicates} bản trùng dư`
        : "";
      setQuickMessage(result.created
        ? `Đã lưu “${result.account.name}”.`
        : `Đã thay bản cũ bằng “${result.account.name}”, đưa lên đầu${merged}.`);
      setSecret("");
      setName("");
      setNote("");
      setPreview(undefined);
      setShowAdd(false);
      setTab("active");
      await loadAccounts(false);
    } catch (value) {
      setError(String(value));
    } finally {
      setBusy(false);
    }
  };

  const copyAccount = async (account: OtpAccount) => {
    try {
      await invoke("copy_otp_code", { id: account.id });
      setCopiedId(account.id);
      window.setTimeout(() => setCopiedId((id) => id === account.id ? undefined : id), 1300);
      await loadAccounts(account.archived);
    } catch (value) {
      setError(String(value));
    }
  };

  const toggleSecret = async (account: OtpAccount) => {
    if (revealedSecrets[account.id] !== undefined) {
      setRevealedSecrets((current) => {
        const next = { ...current };
        delete next[account.id];
        return next;
      });
      return;
    }
    if (secretBusyId === account.id) return;
    setSecretBusyId(account.id);
    setError("");
    try {
      const value = await invoke<string>("get_otp_secret", { id: account.id });
      setRevealedSecrets((current) => ({ ...current, [account.id]: value }));
    } catch (value) {
      setError(String(value));
    } finally {
      setSecretBusyId(undefined);
    }
  };

  const copyStoredSecret = async (account: OtpAccount) => {
    setError("");
    try {
      await invoke("copy_otp_secret", { id: account.id });
      setSecretCopiedId(account.id);
      window.setTimeout(
        () => setSecretCopiedId((id) => id === account.id ? undefined : id),
        1300,
      );
    } catch (value) {
      setError(String(value));
    }
  };

  const rename = async (id: number) => {
    if (!editName.trim()) return;
    await invoke("rename_otp_account", { id, name: editName, note: editNote });
    setEditingId(undefined);
    await loadAccounts(tab === "archived");
  };

  const togglePin = async (id: number) => {
    await invoke("toggle_otp_pin", { id });
    await loadAccounts(tab === "archived");
  };

  const archive = async (id: number, archived: boolean) => {
    await invoke("set_otp_archived", { id, archived });
    setRevealedSecrets((current) => {
      const next = { ...current };
      delete next[id];
      return next;
    });
    await loadAccounts(tab === "archived");
  };

  const remove = async (id: number) => {
    if (deleteId !== id) {
      setDeleteId(id);
      window.setTimeout(() => setDeleteId((value) => value === id ? undefined : value), 3000);
      return;
    }
    await invoke("delete_otp_account", { id });
    setRevealedSecrets((current) => {
      const next = { ...current };
      delete next[id];
      return next;
    });
    setDeleteId(undefined);
    await loadAccounts(tab === "archived");
  };

  const clearHistory = async () => {
    if (!clearConfirm) {
      setClearConfirm(true);
      window.setTimeout(() => setClearConfirm(false), 3000);
      return;
    }
    await invoke("clear_otp_history");
    setClearConfirm(false);
    setHistory([]);
  };

  const chooseBackup = async (mode: BackupMode) => {
    const filters = [{ name: "HeaSpot 2FA Backup", extensions: ["heaspot2fa"] }];
    const selected = mode === "export"
      ? await save({ filters, defaultPath: `HeaSpot-2FA-${new Date().toISOString().slice(0, 10)}.heaspot2fa` })
      : await open({ filters, multiple: false, directory: false });
    if (!selected || Array.isArray(selected)) return;
    setBackup({ mode, path: String(selected) });
    setBackupPassword("");
    setBackupConfirm("");
    setBackupMessage("");
  };

  const runBackup = async (event: FormEvent) => {
    event.preventDefault();
    if (!backup || backupBusy) return;
    if (backupPassword.length < 8) {
      setBackupMessage("Mật khẩu cần ít nhất 8 ký tự.");
      return;
    }
    if (backup.mode === "export" && backupPassword !== backupConfirm) {
      setBackupMessage("Hai lần nhập mật khẩu chưa khớp.");
      return;
    }
    setBackupBusy(true);
    setBackupMessage("");
    try {
      const result = await invoke<BackupResult>(
        backup.mode === "export" ? "export_otp_backup" : "import_otp_backup",
        { path: backup.path, password: backupPassword },
      );
      if (backup.mode === "export") {
        setBackupMessage(`Đã xuất ${result.accounts} tài khoản và ${result.history} bản history.`);
      } else {
        setBackupMessage(`Đã nhập ${result.accounts} tài khoản, bỏ qua ${result.skipped} tài khoản trùng.`);
        await loadAccounts(tab === "archived");
        if (tab === "history") await loadHistory();
      }
    } catch (value) {
      setBackupMessage(String(value));
    } finally {
      setBackupBusy(false);
    }
  };

  return (
    <div className="relative flex min-h-0 flex-1 flex-col border-t border-black/5 dark:border-white/10">
      <div className="flex h-12 shrink-0 items-center justify-between px-4">
        <div className="flex items-center gap-1 rounded-xl bg-black/[0.04] p-1 dark:bg-white/[0.06]">
          {(["active", "archived", "history"] as const).map((value) => (
            <button
              key={value}
              type="button"
              onClick={() => setTab(value)}
              className={`rounded-lg px-3 py-1.5 text-[12px] font-medium transition-colors ${
                tab === value
                  ? "bg-white text-zinc-900 shadow-sm dark:bg-zinc-700 dark:text-white"
                  : "text-zinc-500 hover:text-zinc-800 dark:text-zinc-400 dark:hover:text-zinc-200"
              }`}
            >
              {value === "active" ? "Đang dùng" : value === "archived" ? "Lưu trữ" : "Lịch sử copy"}
            </button>
          ))}
        </div>
        {tab !== "history" ? (
          <div className="flex items-center gap-1.5">
            <button type="button" onClick={() => void chooseBackup("import")} className="rounded-lg border border-black/10 px-2.5 py-1.5 text-[11px] font-medium text-zinc-500 hover:bg-black/5 dark:border-white/10 dark:text-zinc-400 dark:hover:bg-white/10">Nhập backup</button>
            <button type="button" onClick={() => void chooseBackup("export")} className="rounded-lg border border-black/10 px-2.5 py-1.5 text-[11px] font-medium text-zinc-500 hover:bg-black/5 dark:border-white/10 dark:text-zinc-400 dark:hover:bg-white/10">Xuất backup</button>
            <button
              type="button"
              onClick={() => {
                setShowAdd((value) => !value);
                setError("");
                requestAnimationFrame(() => secretRef.current?.focus());
              }}
              className="rounded-lg bg-blue-600 px-3 py-1.5 text-[12px] font-semibold text-white hover:bg-blue-500"
            >
              {showAdd ? "Đóng" : "+ Thêm 2FA"}
            </button>
          </div>
        ) : (
          <button
            type="button"
            onClick={() => void clearHistory()}
            className={`rounded-lg px-3 py-1.5 text-[12px] font-medium ${clearConfirm ? "bg-red-600 text-white" : "text-red-500 hover:bg-red-500/10"}`}
          >
            {clearConfirm ? "Bấm lần nữa để xoá" : "Xoá lịch sử"}
          </button>
        )}
      </div>

      {quickMessage && (
        <div className={`mx-4 mb-2 rounded-lg px-3 py-1.5 text-[11px] ${quickMessage.includes("đã được copy") || quickMessage.startsWith("Đã lưu") || quickMessage.includes("không tạo bản trùng") ? "bg-emerald-500/10 text-emerald-500" : "bg-blue-500/10 text-blue-500"}`}>
          {quickMessage}
        </div>
      )}

      {showAdd && tab !== "history" && (
        <form onSubmit={addAccount} className="mx-4 mb-3 rounded-xl border border-blue-500/20 bg-blue-500/[0.04] p-3">
          <div className="flex gap-2">
            <input
              value={name}
              onChange={(event) => setName(event.target.value)}
              placeholder="Tên gợi nhớ (VD: Mail công ty)"
              className="w-[230px] rounded-lg border border-black/10 bg-white/80 px-3 py-2 text-[12px] outline-none focus:border-blue-500 dark:border-white/10 dark:bg-zinc-800"
            />
            <div className="relative min-w-0 flex-1">
              <input
                ref={secretRef}
                type={showSecret ? "text" : "password"}
                value={secret}
                autoComplete="off"
                spellCheck={false}
                onChange={(event) => setSecret(event.target.value)}
                placeholder="Dán secret Base32 có/không có dấu cách, hoặc URI otpauth://"
                className="w-full rounded-lg border border-black/10 bg-white/80 py-2 pl-3 pr-14 font-mono text-[12px] outline-none focus:border-blue-500 dark:border-white/10 dark:bg-zinc-800"
              />
              <button
                type="button"
                onClick={() => setShowSecret((value) => !value)}
                className="absolute right-2 top-1/2 -translate-y-1/2 text-[10px] font-semibold text-zinc-400 hover:text-zinc-700 dark:hover:text-zinc-200"
              >
                {showSecret ? "ẨN" : "HIỆN"}
              </button>
            </div>
            <button
              type="submit"
              disabled={!preview || busy}
              className="rounded-lg bg-blue-600 px-4 text-[12px] font-semibold text-white disabled:cursor-not-allowed disabled:opacity-40"
            >
              {busy ? "Đang lưu…" : "Lưu"}
            </button>
          </div>
          <input
            value={note}
            onChange={(event) => setNote(event.target.value)}
            placeholder="Chú thích (VD: mail công ty, phòng ban, mục đích sử dụng…)"
            className="mt-2 w-full rounded-lg border border-black/10 bg-white/80 px-3 py-2 text-[12px] outline-none focus:border-blue-500 dark:border-white/10 dark:bg-zinc-800"
          />
          <div className="mt-2 flex min-h-5 items-center justify-between text-[11px]">
            {preview ? (
              <div className="flex items-center gap-2 text-zinc-500 dark:text-zinc-400">
                <span className="rounded bg-emerald-500/10 px-1.5 py-0.5 font-semibold text-emerald-600 dark:text-emerald-400">Hợp lệ</span>
                <span>{preview.provider}</span>
                <span>{preview.otp_type.toUpperCase()} · {preview.algorithm} · {preview.digits} số{preview.otp_type === "totp" ? ` · ${preview.period}s` : ""}</span>
                <span className="font-mono text-[14px] font-semibold text-zinc-800 dark:text-zinc-100">{displayCode(preview.code)}</span>
              </div>
            ) : (
              <span className={error ? "text-red-500" : "text-zinc-400"}>{error || "Hỗ trợ Google, Microsoft OATH và mọi URI otpauth chuẩn."}</span>
            )}
            <span className="text-zinc-400">Secret được mã hoá bằng Windows DPAPI, chỉ user Windows này đọc được.</span>
          </div>
        </form>
      )}

      {error && preview && <div className="mx-4 mb-2 text-[11px] text-red-500">{error}</div>}

      {tab === "history" ? (
        <div className="min-h-0 flex-1 overflow-y-auto px-4 pb-3">
          {history.length === 0 ? (
            <div className="grid h-full place-items-center text-[12px] text-zinc-400">Chưa có lịch sử — chỉ mã bạn bấm Copy mới được ghi lại.</div>
          ) : history.map((item) => (
            <div key={item.id} className="mb-1.5 flex items-center gap-3 rounded-xl border border-black/[0.06] px-3 py-2 dark:border-white/[0.07]">
              <div className="min-w-0 flex-1">
                <div className="truncate text-[12px] font-medium text-zinc-800 dark:text-zinc-100">{item.account_name}</div>
                <div className="text-[10.5px] text-zinc-400">
                  Copy lúc {timeLabel(item.generated_at)} · {item.valid_until ? `hết hạn ${timeLabel(item.valid_until)}` : "HOTP không hết hạn theo thời gian"}
                </div>
              </div>
              <div className="font-mono text-[17px] font-semibold tracking-wider text-zinc-700 dark:text-zinc-200">{displayCode(item.code)}</div>
              <button
                type="button"
                onClick={() => void invoke("copy_secret", { text: item.code })}
                className="rounded-lg border border-black/10 px-2.5 py-1.5 text-[11px] text-zinc-500 hover:bg-black/5 dark:border-white/10 dark:hover:bg-white/10"
              >
                Copy lại
              </button>
            </div>
          ))}
        </div>
      ) : (
        <div className="min-h-0 flex-1 overflow-y-auto px-4 pb-3">
          {accounts.length === 0 ? (
            <div className="grid h-full place-items-center text-center text-[12px] text-zinc-400">
              <div>
                <div className="mb-1 text-[28px]">⌁</div>
                {tab === "active" ? "Chưa có tài khoản 2FA. Bấm “+ Thêm 2FA” để bắt đầu." : "Chưa có tài khoản được lưu trữ."}
              </div>
            </div>
          ) : accounts.map((account) => {
            const progress = account.otp_type === "totp" ? Math.max(0, Math.min(100, account.remaining / account.period * 100)) : 100;
            const urgent = account.otp_type === "totp" && account.remaining <= 5;
            return (
              <div key={account.id} className="group relative mb-2 overflow-hidden rounded-xl border border-black/[0.07] bg-white/45 dark:border-white/[0.08] dark:bg-white/[0.025]">
                <div className="flex items-center gap-3 px-3 py-2.5">
                  <div className={`grid h-9 w-9 shrink-0 place-items-center rounded-xl text-[12px] font-bold ${account.provider === "Microsoft" ? "bg-sky-500/10 text-sky-500" : account.provider === "Google" ? "bg-red-500/10 text-red-500" : "bg-violet-500/10 text-violet-500"}`}>
                    {account.provider === "Microsoft" ? "MS" : account.provider === "Google" ? "G" : "2F"}
                  </div>
                  <div className="min-w-0 flex-1">
                    {editingId === account.id ? (
                      <form className="flex max-w-[390px] gap-1" onSubmit={(event) => { event.preventDefault(); void rename(account.id); }}>
                        <input autoFocus value={editName} onChange={(event) => setEditName(event.target.value)} placeholder="Tên" className="w-[140px] rounded border border-blue-500 bg-transparent px-2 py-0.5 text-[12px] outline-none" />
                        <input value={editNote} onChange={(event) => setEditNote(event.target.value)} placeholder="Chú thích" className="min-w-0 flex-1 rounded border border-blue-500 bg-transparent px-2 py-0.5 text-[12px] outline-none" />
                        <button className="text-[11px] font-semibold text-blue-500">Lưu</button>
                      </form>
                    ) : (
                      <div className="flex items-center gap-1.5">
                        {account.pinned && <span className="text-[11px] text-blue-500">◆</span>}
                        <span className="truncate text-[13px] font-semibold text-zinc-800 dark:text-zinc-100">{account.name}</span>
                      </div>
                    )}
                    <div className="truncate text-[10.5px] text-zinc-400">
                      {account.note ? `${account.note} · ` : ""}{account.provider}{account.issuer && account.issuer !== account.provider ? ` · ${account.issuer}` : ""} · {account.otp_type.toUpperCase()} · {account.algorithm}
                    </div>
                    <div className="truncate text-[9.5px] text-zinc-400/80">
                      Thêm {storedDate(account.created_at)}{account.updated_at !== account.created_at ? ` · cập nhật ${storedDate(account.updated_at)}` : ""}
                    </div>
                  </div>
                  <button type="button" onClick={() => void copyAccount(account)} className={`min-w-[150px] rounded-xl px-4 py-1.5 font-mono text-[21px] font-bold tracking-wider transition-colors ${copiedId === account.id ? "bg-emerald-500 text-white" : urgent ? "bg-amber-500/10 text-amber-600 dark:text-amber-400" : "bg-blue-500/10 text-blue-600 hover:bg-blue-500/15 dark:text-blue-400"}`}>
                    {copiedId === account.id ? "ĐÃ COPY" : displayCode(account.code)}
                  </button>
                  <div className={`w-9 text-center text-[11px] font-semibold ${urgent ? "text-amber-500" : "text-zinc-400"}`}>
                    {account.otp_type === "totp" ? `${account.remaining}s` : "HOTP"}
                  </div>
                  <div className="flex gap-1">
                    <IconButton title={account.pinned ? "Bỏ ghim" : "Ghim tài khoản"} active={account.pinned} onClick={() => void togglePin(account.id)}>◆</IconButton>
                    <IconButton title={revealedSecrets[account.id] !== undefined ? "Ẩn secret key" : "Hiện secret key"} active={revealedSecrets[account.id] !== undefined} onClick={() => void toggleSecret(account)}>{secretBusyId === account.id ? "…" : "S"}</IconButton>
                    <IconButton title="Copy secret key" active={secretCopiedId === account.id} onClick={() => void copyStoredSecret(account)}>{secretCopiedId === account.id ? "✓" : "⧉"}</IconButton>
                    <IconButton title="Sửa tên và chú thích" onClick={() => { setEditingId(account.id); setEditName(account.name); setEditNote(account.note); }}>✎</IconButton>
                    <IconButton title={account.archived ? "Khôi phục" : "Lưu trữ"} onClick={() => void archive(account.id, !account.archived)}>{account.archived ? "↥" : "⌑"}</IconButton>
                    <IconButton title={deleteId === account.id ? "Bấm lần nữa để xoá vĩnh viễn" : "Xoá"} danger={deleteId === account.id} onClick={() => void remove(account.id)}>{deleteId === account.id ? "!" : "×"}</IconButton>
                  </div>
                </div>
                {revealedSecrets[account.id] !== undefined && (
                  <div className="mx-3 mb-2 flex items-center gap-2 rounded-lg border border-amber-500/20 bg-amber-500/[0.06] px-3 py-2">
                    <span className="shrink-0 text-[10px] font-semibold uppercase tracking-wide text-amber-600 dark:text-amber-400">Secret key</span>
                    <code className="min-w-0 flex-1 select-all break-all font-mono text-[11.5px] text-zinc-700 dark:text-zinc-200">
                      {displaySecret(revealedSecrets[account.id])}
                    </code>
                    <button type="button" onClick={() => void copyStoredSecret(account)} className="shrink-0 rounded-md bg-amber-500/10 px-2 py-1 text-[10.5px] font-semibold text-amber-700 hover:bg-amber-500/20 dark:text-amber-300">
                      {secretCopiedId === account.id ? "Đã copy" : "Copy secret"}
                    </button>
                  </div>
                )}
                <div className="h-0.5 bg-black/[0.04] dark:bg-white/[0.04]"><div className={`h-full transition-[width] duration-500 ${urgent ? "bg-amber-500" : "bg-blue-500"}`} style={{ width: `${progress}%` }} /></div>
              </div>
            );
          })}
        </div>
      )}

      <div className="flex shrink-0 items-center justify-between border-t border-black/5 px-4 py-2 text-[10.5px] text-zinc-400 dark:border-white/10">
        <span>Lưu cả secret gốc · Microsoft OATH‑TOTP tương thích; push/number matching vẫn cần Microsoft Authenticator.</span>
        <span>S hiện/ẩn · ⧉ copy secret · Secret không vào clipboard history</span>
      </div>

      {backup && (
        <div className="absolute inset-0 z-30 grid place-items-center bg-white/[0.97] px-8 dark:bg-zinc-900/[0.97]">
          <form onSubmit={runBackup} className="w-full max-w-[480px] rounded-2xl border border-black/10 bg-zinc-50 p-5 shadow-2xl dark:border-white/10 dark:bg-zinc-800">
            <div className="mb-1 text-[16px] font-semibold text-zinc-900 dark:text-zinc-50">
              {backup.mode === "export" ? "Xuất backup 2FA được mã hóa" : "Nhập backup 2FA"}
            </div>
            <div className="mb-4 truncate text-[11px] text-zinc-400" title={backup.path}>{backup.path}</div>
            <label className="mb-1 block text-[11px] font-medium text-zinc-500 dark:text-zinc-300">Mật khẩu backup</label>
            <input
              autoFocus
              type="password"
              value={backupPassword}
              onChange={(event) => setBackupPassword(event.target.value)}
              placeholder="Ít nhất 8 ký tự"
              className="mb-2 w-full rounded-lg border border-black/10 bg-white px-3 py-2 text-[13px] outline-none focus:border-blue-500 dark:border-white/10 dark:bg-zinc-900"
            />
            {backup.mode === "export" && (
              <input
                type="password"
                value={backupConfirm}
                onChange={(event) => setBackupConfirm(event.target.value)}
                placeholder="Nhập lại mật khẩu"
                className="mb-2 w-full rounded-lg border border-black/10 bg-white px-3 py-2 text-[13px] outline-none focus:border-blue-500 dark:border-white/10 dark:bg-zinc-900"
              />
            )}
            <div className={`min-h-8 text-[11px] ${backupMessage.startsWith("Đã ") ? "text-emerald-500" : "text-zinc-400"}`}>
              {backupMessage || (backup.mode === "export"
                ? "File chứa đầy đủ secret, cấu hình, ghim, lưu trữ và history; được khóa bằng Argon2id + AES‑256‑GCM."
                : "Tài khoản trùng sẽ được bỏ qua; dữ liệu import được mã hóa lại bằng DPAPI của máy này.")}
            </div>
            <div className="mt-3 flex justify-end gap-2">
              <button type="button" disabled={backupBusy} onClick={() => setBackup(undefined)} className="rounded-lg px-3 py-2 text-[12px] text-zinc-500 hover:bg-black/5 dark:hover:bg-white/10">Đóng</button>
              <button type="submit" disabled={backupBusy} className="rounded-lg bg-blue-600 px-4 py-2 text-[12px] font-semibold text-white disabled:opacity-50">
                {backupBusy ? "Đang xử lý…" : backup.mode === "export" ? "Mã hóa & xuất" : "Giải mã & nhập"}
              </button>
            </div>
          </form>
        </div>
      )}
    </div>
  );
}
