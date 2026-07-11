/**
 * Từ điển hóa học (`chem`): tra nhanh nguyên tố (số proton, nguyên tử khối,
 * phân loại, tính chất) và tính PHÂN TỬ KHỐI của công thức (VD H2O, Ca(OH)2).
 */
import type { ResultItemData } from "../types";

interface Element {
  z: number;
  sym: string;
  en: string;
  vi: string;
  mass: number;
  cat: string;
}

// [z, sym, en, vi, mass, category]
const RAW: [number, string, string, string, number, string][] = [
  [1, "H", "Hydrogen", "Hydro", 1.008, "phi kim"],
  [2, "He", "Helium", "Heli", 4.0026, "khí hiếm"],
  [3, "Li", "Lithium", "Liti", 6.94, "kim loại kiềm"],
  [4, "Be", "Beryllium", "Beri", 9.0122, "kim loại kiềm thổ"],
  [5, "B", "Boron", "Bo", 10.81, "á kim"],
  [6, "C", "Carbon", "Cacbon", 12.011, "phi kim"],
  [7, "N", "Nitrogen", "Nitơ", 14.007, "phi kim"],
  [8, "O", "Oxygen", "Oxy", 15.999, "phi kim"],
  [9, "F", "Fluorine", "Flo", 18.998, "halogen"],
  [10, "Ne", "Neon", "Neon", 20.18, "khí hiếm"],
  [11, "Na", "Sodium", "Natri", 22.99, "kim loại kiềm"],
  [12, "Mg", "Magnesium", "Magie", 24.305, "kim loại kiềm thổ"],
  [13, "Al", "Aluminium", "Nhôm", 26.982, "kim loại"],
  [14, "Si", "Silicon", "Silic", 28.085, "á kim"],
  [15, "P", "Phosphorus", "Photpho", 30.974, "phi kim"],
  [16, "S", "Sulfur", "Lưu huỳnh", 32.06, "phi kim"],
  [17, "Cl", "Chlorine", "Clo", 35.45, "halogen"],
  [18, "Ar", "Argon", "Argon", 39.948, "khí hiếm"],
  [19, "K", "Potassium", "Kali", 39.098, "kim loại kiềm"],
  [20, "Ca", "Calcium", "Canxi", 40.078, "kim loại kiềm thổ"],
  [21, "Sc", "Scandium", "Scandi", 44.956, "kim loại chuyển tiếp"],
  [22, "Ti", "Titanium", "Titan", 47.867, "kim loại chuyển tiếp"],
  [23, "V", "Vanadium", "Vanadi", 50.942, "kim loại chuyển tiếp"],
  [24, "Cr", "Chromium", "Crom", 51.996, "kim loại chuyển tiếp"],
  [25, "Mn", "Manganese", "Mangan", 54.938, "kim loại chuyển tiếp"],
  [26, "Fe", "Iron", "Sắt", 55.845, "kim loại chuyển tiếp"],
  [27, "Co", "Cobalt", "Coban", 58.933, "kim loại chuyển tiếp"],
  [28, "Ni", "Nickel", "Niken", 58.693, "kim loại chuyển tiếp"],
  [29, "Cu", "Copper", "Đồng", 63.546, "kim loại chuyển tiếp"],
  [30, "Zn", "Zinc", "Kẽm", 65.38, "kim loại chuyển tiếp"],
  [31, "Ga", "Gallium", "Gali", 69.723, "kim loại"],
  [32, "Ge", "Germanium", "Gecmani", 72.63, "á kim"],
  [33, "As", "Arsenic", "Asen", 74.922, "á kim"],
  [34, "Se", "Selenium", "Selen", 78.971, "phi kim"],
  [35, "Br", "Bromine", "Brom", 79.904, "halogen"],
  [36, "Kr", "Krypton", "Krypton", 83.798, "khí hiếm"],
  [37, "Rb", "Rubidium", "Rubidi", 85.468, "kim loại kiềm"],
  [38, "Sr", "Strontium", "Stronti", 87.62, "kim loại kiềm thổ"],
  [39, "Y", "Yttrium", "Ytri", 88.906, "kim loại chuyển tiếp"],
  [40, "Zr", "Zirconium", "Ziriconi", 91.224, "kim loại chuyển tiếp"],
  [41, "Nb", "Niobium", "Niobi", 92.906, "kim loại chuyển tiếp"],
  [42, "Mo", "Molybdenum", "Molypden", 95.95, "kim loại chuyển tiếp"],
  [43, "Tc", "Technetium", "Tecneti", 98, "kim loại chuyển tiếp"],
  [44, "Ru", "Ruthenium", "Rutheni", 101.07, "kim loại chuyển tiếp"],
  [45, "Rh", "Rhodium", "Rhodi", 102.91, "kim loại chuyển tiếp"],
  [46, "Pd", "Palladium", "Paladi", 106.42, "kim loại chuyển tiếp"],
  [47, "Ag", "Silver", "Bạc", 107.87, "kim loại chuyển tiếp"],
  [48, "Cd", "Cadmium", "Cadimi", 112.41, "kim loại chuyển tiếp"],
  [49, "In", "Indium", "Indi", 114.82, "kim loại"],
  [50, "Sn", "Tin", "Thiếc", 118.71, "kim loại"],
  [51, "Sb", "Antimony", "Antimon", 121.76, "á kim"],
  [52, "Te", "Tellurium", "Telua", 127.6, "á kim"],
  [53, "I", "Iodine", "Iot", 126.9, "halogen"],
  [54, "Xe", "Xenon", "Xenon", 131.29, "khí hiếm"],
  [55, "Cs", "Caesium", "Xesi", 132.91, "kim loại kiềm"],
  [56, "Ba", "Barium", "Bari", 137.33, "kim loại kiềm thổ"],
  [57, "La", "Lanthanum", "Lantan", 138.91, "lantan"],
  [58, "Ce", "Cerium", "Xeri", 140.12, "lantan"],
  [59, "Pr", "Praseodymium", "Praseodymi", 140.91, "lantan"],
  [60, "Nd", "Neodymium", "Neodymi", 144.24, "lantan"],
  [61, "Pm", "Promethium", "Prometi", 145, "lantan"],
  [62, "Sm", "Samarium", "Samari", 150.36, "lantan"],
  [63, "Eu", "Europium", "Europi", 151.96, "lantan"],
  [64, "Gd", "Gadolinium", "Gadolini", 157.25, "lantan"],
  [65, "Tb", "Terbium", "Terbi", 158.93, "lantan"],
  [66, "Dy", "Dysprosium", "Dysprosi", 162.5, "lantan"],
  [67, "Ho", "Holmium", "Holmi", 164.93, "lantan"],
  [68, "Er", "Erbium", "Erbi", 167.26, "lantan"],
  [69, "Tm", "Thulium", "Thuli", 168.93, "lantan"],
  [70, "Yb", "Ytterbium", "Ytterbi", 173.05, "lantan"],
  [71, "Lu", "Lutetium", "Luteti", 174.97, "lantan"],
  [72, "Hf", "Hafnium", "Hafni", 178.49, "kim loại chuyển tiếp"],
  [73, "Ta", "Tantalum", "Tantan", 180.95, "kim loại chuyển tiếp"],
  [74, "W", "Tungsten", "Vonfram", 183.84, "kim loại chuyển tiếp"],
  [75, "Re", "Rhenium", "Rheni", 186.21, "kim loại chuyển tiếp"],
  [76, "Os", "Osmium", "Osmi", 190.23, "kim loại chuyển tiếp"],
  [77, "Ir", "Iridium", "Iridi", 192.22, "kim loại chuyển tiếp"],
  [78, "Pt", "Platinum", "Bạch kim", 195.08, "kim loại chuyển tiếp"],
  [79, "Au", "Gold", "Vàng", 196.97, "kim loại chuyển tiếp"],
  [80, "Hg", "Mercury", "Thủy ngân", 200.59, "kim loại chuyển tiếp"],
  [81, "Tl", "Thallium", "Tali", 204.38, "kim loại"],
  [82, "Pb", "Lead", "Chì", 207.2, "kim loại"],
  [83, "Bi", "Bismuth", "Bitmut", 208.98, "kim loại"],
  [84, "Po", "Polonium", "Poloni", 209, "kim loại"],
  [85, "At", "Astatine", "Astatin", 210, "halogen"],
  [86, "Rn", "Radon", "Radon", 222, "khí hiếm"],
  [87, "Fr", "Francium", "Franxi", 223, "kim loại kiềm"],
  [88, "Ra", "Radium", "Radi", 226, "kim loại kiềm thổ"],
  [89, "Ac", "Actinium", "Actini", 227, "actini"],
  [90, "Th", "Thorium", "Thori", 232.04, "actini"],
  [91, "Pa", "Protactinium", "Protactini", 231.04, "actini"],
  [92, "U", "Uranium", "Urani", 238.03, "actini"],
  [93, "Np", "Neptunium", "Neptuni", 237, "actini"],
  [94, "Pu", "Plutonium", "Plutoni", 244, "actini"],
  [95, "Am", "Americium", "Americi", 243, "actini"],
  [96, "Cm", "Curium", "Curi", 247, "actini"],
  [97, "Bk", "Berkelium", "Berkeli", 247, "actini"],
  [98, "Cf", "Californium", "Californi", 251, "actini"],
  [99, "Es", "Einsteinium", "Einsteini", 252, "actini"],
  [100, "Fm", "Fermium", "Fermi", 257, "actini"],
  [101, "Md", "Mendelevium", "Mendelevi", 258, "actini"],
  [102, "No", "Nobelium", "Nobeli", 259, "actini"],
  [103, "Lr", "Lawrencium", "Lawrenci", 262, "actini"],
  [104, "Rf", "Rutherfordium", "Rutherfordi", 267, "kim loại chuyển tiếp"],
  [105, "Db", "Dubnium", "Dubni", 268, "kim loại chuyển tiếp"],
  [106, "Sg", "Seaborgium", "Seaborgi", 269, "kim loại chuyển tiếp"],
  [107, "Bh", "Bohrium", "Bohri", 270, "kim loại chuyển tiếp"],
  [108, "Hs", "Hassium", "Hassi", 269, "kim loại chuyển tiếp"],
  [109, "Mt", "Meitnerium", "Meitneri", 278, "chưa rõ"],
  [110, "Ds", "Darmstadtium", "Darmstadti", 281, "chưa rõ"],
  [111, "Rg", "Roentgenium", "Roentgeni", 282, "chưa rõ"],
  [112, "Cn", "Copernicium", "Coperniki", 285, "kim loại chuyển tiếp"],
  [113, "Nh", "Nihonium", "Nihoni", 286, "chưa rõ"],
  [114, "Fl", "Flerovium", "Flerovi", 289, "chưa rõ"],
  [115, "Mc", "Moscovium", "Moscovi", 290, "chưa rõ"],
  [116, "Lv", "Livermorium", "Livermori", 293, "chưa rõ"],
  [117, "Ts", "Tennessine", "Tennessin", 294, "chưa rõ"],
  [118, "Og", "Oganesson", "Oganesson", 294, "khí hiếm"],
];

const ELEMENTS: Element[] = RAW.map(([z, sym, en, vi, mass, cat]) => ({ z, sym, en, vi, mass, cat }));
const BY_SYM = new Map(ELEMENTS.map((e) => [e.sym.toLowerCase(), e]));

function norm(s: string) {
  return s.normalize("NFD").replace(/[̀-ͯ]/g, "").toLowerCase();
}

function elementCard(e: Element, i: number): ResultItemData {
  const neutrons = Math.round(e.mass) - e.z;
  const preview = [
    `${e.vi} (${e.en}) — ${e.sym}`,
    `Số hiệu nguyên tử (proton): ${e.z}`,
    `Số electron: ${e.z} · Số neutron ≈ ${neutrons}`,
    `Nguyên tử khối: ${e.mass} u`,
    `Phân loại: ${e.cat}`,
  ].join("\n");
  return {
    id: `chem:${e.z}:${i}`,
    title: `${e.sym} — ${e.vi} (${e.en})`,
    subtitle: `Z=${e.z} · M=${e.mass} u · ${e.cat}`,
    kind: "knowledge",
    action: "chem",
    text: String(e.mass),
    preview,
  };
}

/** Parse công thức hóa học -> phân tử khối. Hỗ trợ ngoặc () [] và hydrate (dấu .) */
function molarMass(formula: string): { mass: number; breakdown: string } | null {
  const parts = formula.split(".");
  const total: Record<string, number> = {};
  for (let part of parts) {
    part = part.trim();
    if (!part) continue;
    const coefM = /^(\d+)(.+)$/.exec(part);
    let coef = 1;
    if (coefM) {
      coef = parseInt(coefM[1], 10);
      part = coefM[2];
    }
    const counts = parseUnit(part);
    if (!counts) return null;
    for (const [sym, n] of Object.entries(counts)) {
      total[sym] = (total[sym] || 0) + n * coef;
    }
  }
  const syms = Object.keys(total);
  if (syms.length === 0) return null;
  let mass = 0;
  const lines: string[] = [];
  for (const sym of syms) {
    const el = BY_SYM.get(sym.toLowerCase());
    if (!el) return null;
    const m = el.mass * total[sym];
    mass += m;
    lines.push(`${sym}×${total[sym]} = ${m.toFixed(3)}`);
  }
  return { mass, breakdown: lines.join("\n") };
}

function parseUnit(str: string): Record<string, number> | null {
  const stack: Record<string, number>[] = [{}];
  let i = 0;
  while (i < str.length) {
    const c = str[i];
    if (c === "(" || c === "[") {
      stack.push({});
      i++;
    } else if (c === ")" || c === "]") {
      i++;
      let num = "";
      while (i < str.length && /\d/.test(str[i])) num += str[i++];
      const mult = num ? parseInt(num, 10) : 1;
      const top = stack.pop()!;
      const below = stack[stack.length - 1];
      for (const [sym, n] of Object.entries(top)) below[sym] = (below[sym] || 0) + n * mult;
    } else if (/[A-Z]/.test(c)) {
      let sym = c;
      i++;
      while (i < str.length && /[a-z]/.test(str[i])) sym += str[i++];
      let num = "";
      while (i < str.length && /\d/.test(str[i])) num += str[i++];
      const n = num ? parseInt(num, 10) : 1;
      const top = stack[stack.length - 1];
      top[sym] = (top[sym] || 0) + n;
    } else {
      return null; // ký tự không hợp lệ
    }
  }
  return stack.length === 1 ? stack[0] : null;
}

function looksLikeFormula(q: string): boolean {
  // Có ít nhất 1 chữ hoa + (có số hoặc ngoặc hoặc >=2 ký hiệu) và toàn ký tự công thức
  if (!/^[A-Za-z0-9()\[\].]+$/.test(q)) return false;
  if (!/[A-Z]/.test(q)) return false;
  return /\d|\(|\[|\./.test(q) || (q.match(/[A-Z]/g)?.length ?? 0) >= 2;
}

export function findChemistry(query: string): ResultItemData[] {
  const q = query.trim();
  if (!q) {
    return [{
      id: "chem:hint",
      title: "Từ điển hóa học — gõ ký hiệu, tên, số proton hoặc công thức",
      subtitle: "VD: chem Fe · chem oxy · chem 26 · chem H2O · chem Ca(OH)2",
      kind: "knowledge",
      action: "chem",
      text: "",
      preview: "Tra nguyên tố: chem Fe / chem sắt / chem 26\nTính phân tử khối: chem H2O / chem C6H12O6 / chem Ca(OH)2",
    }];
  }

  const results: ResultItemData[] = [];

  // 1. Phân tử khối cho công thức
  if (looksLikeFormula(q)) {
    const mm = molarMass(q);
    if (mm) {
      results.push({
        id: `chem:mm:${q}`,
        title: `${q} — Phân tử khối ${mm.mass.toFixed(3)} u`,
        subtitle: "Từ điển hóa học · Enter để copy",
        kind: "knowledge",
        action: "chem",
        text: mm.mass.toFixed(3),
        preview: `Phân tử khối ${q} = ${mm.mass.toFixed(3)} u (g/mol)\n\n${mm.breakdown}`,
      });
    }
  }

  // 2. Tra nguyên tố (ký hiệu / số Z / tên EN-VI)
  const nq = norm(q);
  const asNum = parseInt(q, 10);
  const matched = ELEMENTS.filter((e) => {
    if (e.sym.toLowerCase() === q.toLowerCase()) return true;
    if (!isNaN(asNum) && e.z === asNum) return true;
    if (q.length >= 2 && (norm(e.en).includes(nq) || norm(e.vi).includes(nq))) return true;
    return false;
  });
  // ưu tiên khớp ký hiệu/số chính xác lên đầu
  matched.sort((a, b) => {
    const ax = a.sym.toLowerCase() === q.toLowerCase() || a.z === asNum ? 0 : 1;
    const bx = b.sym.toLowerCase() === q.toLowerCase() || b.z === asNum ? 0 : 1;
    return ax - bx || a.z - b.z;
  });
  results.push(...matched.slice(0, 8).map((e, i) => elementCard(e, i)));

  return results;
}
