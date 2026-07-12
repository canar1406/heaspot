import { useRef, useState, useCallback, useEffect } from "react";

/**
 * Khung xem ảnh có zoom: lăn chuột để phóng to/thu nhỏ (theo con trỏ),
 * kéo để di chuyển khi đã zoom, double-click để reset, nút +/− và Ctrl+0.
 */
export function ZoomableImage({ src, alt = "" }: { src: string; alt?: string }) {
  const wrapRef = useRef<HTMLDivElement>(null);
  const [scale, setScale] = useState(1);
  const [tx, setTx] = useState(0);
  const [ty, setTy] = useState(0);
  const drag = useRef<{ x: number; y: number; tx: number; ty: number } | null>(null);

  // Reset khi ảnh đổi
  useEffect(() => {
    setScale(1);
    setTx(0);
    setTy(0);
  }, [src]);

  const clampScale = (s: number) => Math.min(Math.max(s, 1), 10);

  const zoomAt = useCallback((factor: number, cx?: number, cy?: number) => {
    setScale((prev) => {
      const next = clampScale(prev * factor);
      if (next === prev) return prev;
      const el = wrapRef.current;
      if (el && cx != null && cy != null) {
        const rect = el.getBoundingClientRect();
        // Giữ điểm dưới con trỏ cố định khi zoom
        const px = cx - rect.left - rect.width / 2;
        const py = cy - rect.top - rect.height / 2;
        const ratio = next / prev;
        setTx((t) => px - (px - t) * ratio);
        setTy((t) => py - (py - t) * ratio);
      }
      if (next === 1) {
        setTx(0);
        setTy(0);
      }
      return next;
    });
  }, []);

  const onWheel = (e: React.WheelEvent) => {
    e.preventDefault();
    zoomAt(e.deltaY < 0 ? 1.15 : 1 / 1.15, e.clientX, e.clientY);
  };

  const onMouseDown = (e: React.MouseEvent) => {
    if (scale <= 1) return;
    drag.current = { x: e.clientX, y: e.clientY, tx, ty };
  };
  const onMouseMove = (e: React.MouseEvent) => {
    if (!drag.current) return;
    setTx(drag.current.tx + (e.clientX - drag.current.x));
    setTy(drag.current.ty + (e.clientY - drag.current.y));
  };
  const endDrag = () => {
    drag.current = null;
  };

  const reset = () => {
    setScale(1);
    setTx(0);
    setTy(0);
  };

  const btn =
    "w-7 h-7 flex items-center justify-center rounded-md bg-black/40 text-white text-[15px] leading-none hover:bg-black/60";

  return (
    <div className="relative flex-1 min-h-0 overflow-hidden">
      <div
        ref={wrapRef}
        onWheel={onWheel}
        onMouseDown={onMouseDown}
        onMouseMove={onMouseMove}
        onMouseUp={endDrag}
        onMouseLeave={endDrag}
        onDoubleClick={reset}
        className="w-full h-full flex items-center justify-center p-3 select-none"
        style={{ cursor: scale > 1 ? (drag.current ? "grabbing" : "grab") : "default" }}
      >
        <img
          src={src}
          alt={alt}
          draggable={false}
          className="max-w-full max-h-full object-contain rounded-lg border border-black/10 dark:border-white/10"
          style={{ transform: `translate(${tx}px, ${ty}px) scale(${scale})`, transition: drag.current ? "none" : "transform 0.08s" }}
        />
      </div>
      {/* Thanh điều khiển zoom */}
      <div className="absolute bottom-2 right-2 flex items-center gap-1">
        <button className={btn} title="Thu nhỏ" onClick={() => zoomAt(1 / 1.3)}>−</button>
        <span className="px-1.5 h-7 flex items-center rounded-md bg-black/40 text-white text-[11px] tabular-nums">
          {Math.round(scale * 100)}%
        </span>
        <button className={btn} title="Phóng to" onClick={() => zoomAt(1.3)}>+</button>
        {scale > 1 && (
          <button className={btn} title="Về mặc định (double-click)" onClick={reset}>⤢</button>
        )}
      </div>
    </div>
  );
}
