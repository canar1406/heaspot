/**
 * Search nhanh công thức/ký hiệu LaTeX (keyword `latex`).
 * Gõ tên (VN hoặc EN) -> ra code LaTeX, Enter để copy.
 */
import type { ResultItemData } from "../types";

interface Snip {
  label: string;
  code: string;
  names: string[];
}

const L: Snip[] = [
  { label: "Căn bậc hai", code: "\\sqrt{x}", names: ["can bac hai", "sqrt", "square root", "can"] },
  { label: "Căn bậc n", code: "\\sqrt[n]{x}", names: ["can bac n", "nth root", "can bac ba", "cube root"] },
  { label: "Phân số", code: "\\frac{a}{b}", names: ["phan so", "fraction", "frac"] },
  { label: "Phân số lồng", code: "\\dfrac{a}{b}", names: ["phan so lon", "dfrac", "display fraction"] },
  { label: "Lũy thừa", code: "x^{n}", names: ["luy thua", "power", "mu", "exponent"] },
  { label: "Chỉ số dưới", code: "x_{i}", names: ["chi so duoi", "subscript", "index"] },
  { label: "Tổng Sigma", code: "\\sum_{i=1}^{n} a_i", names: ["tong", "sum", "sigma", "tong sigma"] },
  { label: "Tích", code: "\\prod_{i=1}^{n} a_i", names: ["tich", "product", "prod", "pi tich"] },
  { label: "Tích phân", code: "\\int_{a}^{b} f(x)\\,dx", names: ["tich phan", "integral", "int"] },
  { label: "Tích phân bội hai", code: "\\iint_{D} f(x,y)\\,dA", names: ["tich phan boi", "double integral", "iint"] },
  { label: "Đạo hàm", code: "\\frac{d}{dx}f(x)", names: ["dao ham", "derivative", "d/dx"] },
  { label: "Đạo hàm riêng", code: "\\frac{\\partial f}{\\partial x}", names: ["dao ham rieng", "partial derivative", "partial"] },
  { label: "Giới hạn", code: "\\lim_{x \\to \\infty} f(x)", names: ["gioi han", "limit", "lim"] },
  { label: "Vô cực", code: "\\infty", names: ["vo cuc", "infinity", "infty"] },
  { label: "Hệ phương trình", code: "\\begin{cases} a_1x + b_1y = c_1 \\\\ a_2x + b_2y = c_2 \\end{cases}", names: ["he phuong trinh", "system of equations", "cases", "hpt"] },
  { label: "Ma trận", code: "\\begin{pmatrix} a & b \\\\ c & d \\end{pmatrix}", names: ["ma tran", "matrix", "pmatrix"] },
  { label: "Định thức", code: "\\begin{vmatrix} a & b \\\\ c & d \\end{vmatrix}", names: ["dinh thuc", "determinant", "vmatrix"] },
  { label: "Vector", code: "\\vec{v}", names: ["vector", "vecto", "vec"] },
  { label: "Giá trị tuyệt đối", code: "\\left| x \\right|", names: ["gia tri tuyet doi", "absolute value", "abs", "tri tuyet doi"] },
  { label: "Nghiệm bậc hai", code: "x = \\frac{-b \\pm \\sqrt{b^2-4ac}}{2a}", names: ["nghiem bac hai", "quadratic formula", "phuong trinh bac hai"] },
  { label: "Hệ số nhị thức", code: "\\binom{n}{k}", names: ["nhi thuc", "binomial", "to hop", "combination", "binom"] },
  { label: "Giai thừa", code: "n!", names: ["giai thua", "factorial"] },
  { label: "Cộng trừ", code: "\\pm", names: ["cong tru", "plus minus", "pm"] },
  { label: "Nhân", code: "\\times", names: ["nhan", "times", "multiply"] },
  { label: "Chia", code: "\\div", names: ["chia", "divide", "div"] },
  { label: "Xấp xỉ", code: "\\approx", names: ["xap xi", "approx", "gan bang"] },
  { label: "Không bằng", code: "\\neq", names: ["khong bang", "not equal", "neq"] },
  { label: "Nhỏ hơn hoặc bằng", code: "\\leq", names: ["nho hon bang", "less equal", "leq"] },
  { label: "Lớn hơn hoặc bằng", code: "\\geq", names: ["lon hon bang", "greater equal", "geq"] },
  { label: "Thuộc", code: "\\in", names: ["thuoc", "belongs", "in element"] },
  { label: "Tập con", code: "\\subset", names: ["tap con", "subset"] },
  { label: "Hợp", code: "\\cup", names: ["hop", "union", "cup"] },
  { label: "Giao", code: "\\cap", names: ["giao", "intersection", "cap"] },
  { label: "Với mọi", code: "\\forall", names: ["voi moi", "for all", "forall"] },
  { label: "Tồn tại", code: "\\exists", names: ["ton tai", "exists", "exist"] },
  { label: "Suy ra", code: "\\Rightarrow", names: ["suy ra", "implies", "rightarrow"] },
  { label: "Tương đương", code: "\\Leftrightarrow", names: ["tuong duong", "iff", "leftrightarrow"] },
  { label: "Mũi tên", code: "\\to", names: ["mui ten", "arrow", "to"] },
  { label: "Pi", code: "\\pi", names: ["pi", "so pi"] },
  { label: "Theta", code: "\\theta", names: ["theta"] },
  { label: "Alpha Beta Gamma", code: "\\alpha, \\beta, \\gamma", names: ["alpha", "beta", "gamma", "chu cai hy lap"] },
  { label: "Delta (biến thiên)", code: "\\Delta", names: ["delta", "bien thien"] },
  { label: "Độ", code: "^{\\circ}", names: ["do", "degree", "do goc"] },
  { label: "Lượng giác", code: "\\sin\\theta, \\cos\\theta, \\tan\\theta", names: ["luong giac", "trig", "sin cos tan"] },
  { label: "Logarit", code: "\\log_{a}{x}", names: ["logarit", "log"] },
  { label: "Ln", code: "\\ln{x}", names: ["ln", "logarit tu nhien", "natural log"] },
  { label: "Số phức", code: "z = a + bi", names: ["so phuc", "complex number"] },
  { label: "Trung bình / gạch ngang", code: "\\bar{x}", names: ["trung binh", "mean", "bar", "gach ngang"] },
  { label: "Nón mũ", code: "\\hat{x}", names: ["mu non", "hat"] },
];

function norm(s: string): string {
  return s.normalize("NFD").replace(/[̀-ͯ]/g, "").toLowerCase();
}

export function findLatex(query: string): ResultItemData[] {
  const q = norm(query.trim());
  const list = q
    ? L.filter((s) => norm([s.label, ...s.names].join(" ")).includes(q))
    : L;
  if (list.length === 0) {
    return [{
      id: "latex:none", title: `Không có mẫu LaTeX cho "${query}"`,
      subtitle: "Thử: căn, phân số, hệ phương trình, ma trận, tích phân, tổng…",
      kind: "generated", text: "",
    }];
  }
  return list.slice(0, 15).map((s, i) => ({
    id: `latex:${i}:${s.label}`,
    title: s.code,
    subtitle: `${s.label} — Enter để copy LaTeX`,
    kind: "generated",
    text: s.code,
  }));
}
