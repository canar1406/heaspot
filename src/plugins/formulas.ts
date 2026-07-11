import type { ResultItemData } from "../types";

interface Formula {
  subject: "Toán" | "Vật lý" | "Hóa học";
  names: string[];
  title: string;
  expression: string;
  description: string;
  variables: string[];
  unit?: string;
}

const F: Formula[] = [
  { subject:"Toán", names:["pythagoras","pitago","tam giac vuong"], title:"Định lý Pythagoras", expression:"a² + b² = c²", description:"Liên hệ ba cạnh của tam giác vuông.", variables:["a, b: hai cạnh góc vuông","c: cạnh huyền"] },
  { subject:"Toán", names:["quadratic","phuong trinh bac hai","nghiem bac hai"], title:"Nghiệm phương trình bậc hai", expression:"x = (-b ± √(b² - 4ac)) / 2a", description:"Giải ax² + bx + c = 0, với a ≠ 0.", variables:["a, b, c: hệ số","Δ = b² - 4ac"] },
  { subject:"Toán", names:["circle area","dien tich hinh tron"], title:"Diện tích hình tròn", expression:"S = πr²", description:"Diện tích hình tròn bán kính r.", variables:["S: diện tích","r: bán kính"], unit:"m² (nếu r tính bằng m)" },
  { subject:"Toán", names:["circle circumference","chu vi hinh tron"], title:"Chu vi hình tròn", expression:"C = 2πr", description:"Chu vi đường tròn bán kính r.", variables:["C: chu vi","r: bán kính"], unit:"m" },
  { subject:"Toán", names:["distance","khoang cach hai diem"], title:"Khoảng cách hai điểm", expression:"d = √((x₂-x₁)² + (y₂-y₁)²)", description:"Khoảng cách Euclid trong mặt phẳng.", variables:["(x₁,y₁), (x₂,y₂): hai điểm","d: khoảng cách"] },
  { subject:"Vật lý", names:["newton","luc","newton 2"], title:"Định luật II Newton", expression:"F = ma", description:"Hợp lực bằng khối lượng nhân gia tốc.", variables:["F: lực","m: khối lượng","a: gia tốc"], unit:"F: N; m: kg; a: m/s²" },
  { subject:"Vật lý", names:["kinetic energy","dong nang"], title:"Động năng", expression:"Wđ = ½mv²", description:"Năng lượng của vật do chuyển động.", variables:["m: khối lượng","v: vận tốc"], unit:"J" },
  { subject:"Vật lý", names:["potential energy","the nang"], title:"Thế năng trọng trường", expression:"Wt = mgh", description:"Thế năng gần bề mặt Trái Đất.", variables:["m: khối lượng","g: gia tốc trọng trường","h: độ cao"], unit:"J" },
  { subject:"Vật lý", names:["momentum","dong luong"], title:"Động lượng", expression:"p = mv", description:"Động lượng của vật chuyển động.", variables:["p: động lượng","m: khối lượng","v: vận tốc"], unit:"kg·m/s" },
  { subject:"Vật lý", names:["ohm","dinh luat ohm","dien"], title:"Định luật Ohm", expression:"U = IR", description:"Liên hệ hiệu điện thế, dòng điện và điện trở.", variables:["U: hiệu điện thế","I: cường độ dòng điện","R: điện trở"], unit:"U: V; I: A; R: Ω" },
  { subject:"Vật lý", names:["electric power","cong suat dien"], title:"Công suất điện", expression:"P = UI = I²R = U²/R", description:"Công suất tiêu thụ của mạch điện.", variables:["P: công suất","U: hiệu điện thế","I: dòng điện","R: điện trở"], unit:"W" },
  { subject:"Vật lý", names:["wave","song","tan so"], title:"Phương trình vận tốc sóng", expression:"v = fλ", description:"Liên hệ vận tốc, tần số và bước sóng.", variables:["v: vận tốc sóng","f: tần số","λ: bước sóng"], unit:"v: m/s; f: Hz; λ: m" },
  { subject:"Vật lý", names:["density","khoi luong rieng"], title:"Khối lượng riêng", expression:"ρ = m/V", description:"Khối lượng trên một đơn vị thể tích.", variables:["ρ: khối lượng riêng","m: khối lượng","V: thể tích"], unit:"kg/m³" },
  { subject:"Vật lý", names:["pressure","ap suat"], title:"Áp suất", expression:"p = F/S", description:"Áp lực trên một đơn vị diện tích.", variables:["p: áp suất","F: áp lực","S: diện tích"], unit:"Pa" },
  { subject:"Vật lý", names:["heat","nhiet luong"], title:"Nhiệt lượng", expression:"Q = mcΔT", description:"Nhiệt lượng làm thay đổi nhiệt độ vật.", variables:["m: khối lượng","c: nhiệt dung riêng","ΔT: độ biến thiên nhiệt độ"], unit:"J" },
  { subject:"Hóa học", names:["mole","mol","so mol"], title:"Số mol theo khối lượng", expression:"n = m/M", description:"Tính số mol từ khối lượng chất.", variables:["n: số mol","m: khối lượng","M: khối lượng mol"], unit:"n: mol; m: g; M: g/mol" },
  { subject:"Hóa học", names:["molarity","nong do mol"], title:"Nồng độ mol", expression:"CM = n/V", description:"Số mol chất tan trong một lít dung dịch.", variables:["CM: nồng độ mol","n: số mol chất tan","V: thể tích dung dịch"], unit:"mol/L" },
  { subject:"Hóa học", names:["dilution","pha loang"], title:"Công thức pha loãng", expression:"C₁V₁ = C₂V₂", description:"Bảo toàn lượng chất tan khi pha loãng.", variables:["C₁,V₁: trước pha loãng","C₂,V₂: sau pha loãng"] },
  { subject:"Hóa học", names:["ph","do ph"], title:"Độ pH", expression:"pH = -log₁₀[H⁺]", description:"Độ axit của dung dịch.", variables:["[H⁺]: nồng độ ion hydro"], unit:"Không thứ nguyên" },
  { subject:"Hóa học", names:["ideal gas","khi ly tuong"], title:"Phương trình khí lý tưởng", expression:"PV = nRT", description:"Quan hệ trạng thái của khí lý tưởng.", variables:["P: áp suất","V: thể tích","n: số mol","R: hằng số khí","T: nhiệt độ tuyệt đối"], unit:"Dùng hệ đơn vị nhất quán" },
];

function norm(s: string) {
  return s.normalize("NFD").replace(/[\u0300-\u036f]/g, "").toLowerCase();
}

export function findFormulas(query: string): ResultItemData[] {
  const q = norm(query.trim());
  return F.filter((f) => !q || norm([f.title, f.subject, ...f.names].join(" ")).includes(q))
    .slice(0, 12)
    .map((f, i) => ({
      id: `formula:${i}:${f.title}`,
      title: `${f.title} — ${f.expression}`,
      subtitle: `${f.subject} · ${f.description}`,
      kind: "knowledge" as const,
      action: "formula",
      text: f.expression,
      preview: [
        `${f.title} (${f.subject})`, f.expression, f.description,
        `Biến số:\n${f.variables.map((v) => `• ${v}`).join("\n")}`,
        f.unit ? `Đơn vị: ${f.unit}` : "",
      ].filter(Boolean).join("\n\n"),
    }));
}
