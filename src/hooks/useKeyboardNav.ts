import { useEffect, useState } from "react";

/** Điều hướng danh sách bằng bàn phím: index chọn + di chuyển vòng tròn */
export function useKeyboardNav<T>(items: T[]) {
  const [index, setIndex] = useState(0);

  // Reset về đầu danh sách mỗi khi kết quả thay đổi
  useEffect(() => {
    setIndex(0);
  }, [items]);

  const move = (delta: number) => {
    setIndex((i) => {
      if (items.length === 0) return 0;
      return (i + delta + items.length) % items.length;
    });
  };

  return { index, setIndex, move };
}
