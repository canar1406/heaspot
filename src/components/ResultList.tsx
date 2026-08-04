import { useEffect, useRef } from "react";
import type { ResultItemData } from "../types";
import { ResultItem } from "./ResultItem";

interface Props {
  items: ResultItemData[];
  selectedIndex: number;
  onExecute: (item: ResultItemData) => void;
  onHover: (index: number) => void;
}

export function ResultList({ items, selectedIndex, onExecute, onHover }: Props) {
  const listRef = useRef<HTMLDivElement>(null);

  // Giữ item được chọn luôn nằm trong vùng nhìn thấy
  useEffect(() => {
    const el = listRef.current?.children[selectedIndex] as HTMLElement | undefined;
    el?.scrollIntoView({ block: "nearest" });
  }, [selectedIndex]);

  if (items.length === 0) return null;

  return (
    <div
      ref={listRef}
      className="max-h-[404px] overflow-x-hidden overflow-y-auto py-2 border-t border-black/5 dark:border-white/10"
    >
      {items.map((item, i) => (
        <ResultItem
          key={`${i}:${item.id}`}
          item={item}
          selected={i === selectedIndex}
          shortcut={i < 9 ? `Ctrl+${i + 1}` : undefined}
          onClick={() => onExecute(item)}
          onHover={() => onHover(i)}
        />
      ))}
    </div>
  );
}
