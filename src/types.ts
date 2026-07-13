export type ResultKind =
  | "app"
  | "file"
  | "folder"
  | "calc"
  | "web"
  | "system"
  | "terminal"
  | "snippet"
  | "snippet-add"
  | "fulltext"
  | "capacities"
  | "cap-token"
  | "clipboard"
  | "window"
  | "vscode"
  | "service"
  | "registry"
  | "generated"
  | "unit"
  | "time"
  | "url"
  | "workflow"
  | "wf-cmd"
  | "knowledge"
  | "settings"
  | "ocr-cmd"
  | "process"
  | "process-group"
  | "password";

export interface ResultItemData {
  id: string;
  title: string;
  subtitle?: string;
  kind: ResultKind;
  /** app/file/folder/fulltext: đường dẫn để mở */
  path?: string;
  /** web: URL để mở */
  url?: string;
  /** system: action id gửi xuống Rust */
  action?: string;
  /** calc: kết quả | snippet: nội dung | terminal: lệnh | snippet-add: payload */
  text?: string;
  /** snippet-add */
  keyword?: string;
  /** clipboard: id bản ghi trong SQLite */
  clipId?: number;
  /** icon thật của app (data URL PNG) */
  icon?: string;
  /** window walker: handle cửa sổ */
  hwnd?: number;
  /** process: PID để kill */
  pid?: number;
  /** password: username & mật khẩu (secret) để copy an toàn */
  secret?: string;
  preview?: string;
  audio?: string;
  processes?: ProcInfo[];
  isChild?: boolean;
  expanded?: boolean;
}

export interface ProcInfo {
  pid: number;
  name: string;
  exe: string;
  mem_mb: number;
  icon?: string | null;
}

export interface BrowserPassword {
  browser: string;
  url: string;
  username: string;
  password: string;
}

export interface OpenWindowInfo {
  hwnd: number;
  title: string;
  process: string;
  icon?: string | null;
}

export interface ServiceInfo {
  name: string;
  display: string;
  status: string;
}

export interface VsCodeEntry {
  name: string;
  path: string;
}

export interface RegKeyInfo {
  path: string;
  name: string;
}

export interface WorkflowInfo {
  name: string;
  keyword: string;
  dir: string;
  script_type: string;
  icon?: string | null;
}

export interface WorkflowItem {
  title: string;
  subtitle: string;
  arg: string;
}

export interface BackendSearchResult {
  title: string;
  subtitle: string;
  kind: "app" | "file" | "folder";
  path: string;
  score: number;
  icon?: string | null;
}

export interface CapacitiesHit {
  id: string;
  space_id: string;
  title: string;
  preview: string;
}

export interface KnowledgeHit {
  title: string;
  extract: string;
  url: string;
}

export interface TranslationEntry {
  part_of_speech: string;
  definition_en: string;
  definition_vi: string;
  example: string;
}

export interface QuickTranslation {
  translation: string;
  source_language: string;
  target_language: string;
}

export interface QuickAnswer {
  answer: string;
  source: string;
  url: string;
  related: string[];
}

export interface TranslationHit {
  translation: string;
  source_language: string;
  target_language: string;
  phonetic: string;
  audio_url: string;
  collocations: string[];
  synonyms: string[];
  antonyms: string[];
  entries: TranslationEntry[];
}

export interface StudyWord {
  id: number;
  word: string;
  translation: string;
  details: string;
  created_at: string;
}

export interface BackendSearchResponse {
  results: BackendSearchResult[];
  engine: "everything" | "internal";
}

export interface FullTextHit {
  name: string;
  path: string;
  preview: string;
}

export interface FullTextResponse {
  results: FullTextHit[];
  engine: "windows-search" | "everything-content" | "hybrid";
}

export interface Snippet {
  id: number;
  keyword: string;
  content: string;
}

export interface ClipItem {
  id: number;
  content: string;
  kind: "text" | "link" | "image" | "files";
  created_at: string;
  pinned: boolean;
  source_app: string;
  /** thumbnail data URL (chỉ có với ảnh) */
  thumb: string;
}

export interface AppSettings {
  search_hotkey: string;
  clipboard_hotkey: string;
  keywords: string; // JSON { featureId: keyword }
  feature_hotkeys: string; // JSON { featureId: global hotkey }
  max_clipboard_items: number;
  clipboard_retention_days: number;
  privacy_apps: string;
  auto_paste: boolean;
  serper_api_key: string;
  enable_browser_passwords: boolean;
  password_to_history: boolean;
  theme: string; // "system" | "light" | "dark"
  launch_at_startup: boolean;
}

export type UiMode = "search" | "clipboard";
