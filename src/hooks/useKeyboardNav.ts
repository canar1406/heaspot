import { useEffect, useState } from "react";

/** Điều hướng danh sách bằng bàn phím: index chọn + di chuyển vòng tròn */
export function useKeyboardNav<T>(items: T[]) {
  const [index, setIndex] = useState(0);

  // Array kết quả thường được tạo lại sau mỗi render. Không phụ thuộc trực tiếp
  // vào `items`, nếu không ArrowUp/ArrowDown vừa đổi index xong sẽ bị reset về 0.
  // Chỉ kẹp index khi độ dài danh sách thay đổi; query mới tự reset ở App.
  useEffect(() => {
    setIndex((current) => {
      if (items.length === 0) return 0;
      return Math.min(current, items.length - 1);
    });
  }, [items.length]);

  const move = (delta: number) => {
    setIndex((i) => {
      if (items.length === 0) return 0;
      return (i + delta + items.length) % items.length;
    });
  };

  return { index, setIndex, move };
}
