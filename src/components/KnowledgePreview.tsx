import type { ResultItemData } from "../types";

interface Props {
  item: ResultItemData;
  onPaste: () => void;
  onCopy: () => void;
  onOpen: () => void;
  onSave?: () => void;
  onTranslate?: () => void;
}

export function KnowledgePreview({ item, onPaste, onCopy, onOpen, onSave, onTranslate }: Props) {
  return (
    <div className="flex-1 min-w-0 flex flex-col border-l border-black/5 dark:border-white/10">
      <div className="px-4 pt-3 pb-2 border-b border-black/5 dark:border-white/10">
        <div className="text-[15px] font-semibold text-zinc-900 dark:text-zinc-50">{item.title}</div>
        <div className="text-[10px] uppercase tracking-wider text-indigo-500 mt-0.5">
          {item.action === "translate" ? "Smart Translate · dictionary & meanings"
            : item.action === "formula" ? "Formula · biến số & đơn vị"
              : item.action === "review" ? "Danh sách ôn tập"
                : item.action === "ocr" ? "Windows OCR · văn bản nhận dạng"
                  : "Wikipedia · xem nhanh"}
        </div>
      </div>
      <div className="flex-1 min-h-0 overflow-y-auto px-4 py-3 text-[13px] leading-relaxed select-text text-zinc-700 dark:text-zinc-200 whitespace-pre-wrap">
        {item.preview}
      </div>
      <div className="flex gap-2 px-3 py-2 border-t border-black/5 dark:border-white/10 text-[11px]">
        <button onClick={onPaste} className="px-3 py-1.5 rounded-md bg-blue-600 text-white">Dán</button>
        <button onClick={onCopy} className="px-3 py-1.5 rounded-md bg-black/5 dark:bg-white/10">Copy</button>
        {item.audio && <button onClick={() => void new Audio(item.audio).play()} className="px-3 py-1.5 rounded-md bg-black/5 dark:bg-white/10">🔊 Phát âm</button>}
        {onSave && item.action === "translate" && item.keyword && !/\s/.test(item.keyword) && <button onClick={onSave} className="px-3 py-1.5 rounded-md bg-emerald-600 text-white">Lưu từ</button>}
        {onTranslate && item.action === "ocr" && <button onClick={onTranslate} className="px-3 py-1.5 rounded-md bg-violet-600 text-white">Dịch</button>}
        {item.url && <button onClick={onOpen} className="px-3 py-1.5 rounded-md bg-black/5 dark:bg-white/10">Mở nguồn</button>}
      </div>
    </div>
  );
}
