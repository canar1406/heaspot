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
  | "wf-cmd";

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

export interface BackendSearchResponse {
  results: BackendSearchResult[];
  engine: "everything" | "internal";
}

export interface FullTextHit {
  name: string;
  path: string;
  preview: string;
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

export type UiMode = "search" | "clipboard";
