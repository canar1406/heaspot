import type { ResultItemData } from "../types";

type EmojiEntry = readonly [emoji: string, name: string, category: string, keywords: string];

// Curated launcher-sized index: common emoji first, with Vietnamese + English
// aliases. Keeping it local makes every keystroke instant and works offline.
const EMOJIS: readonly EmojiEntry[] = [
  ["😀", "Cười tươi", "Mặt cười", "cuoi vui happy smile grin"],
  ["😂", "Cười ra nước mắt", "Mặt cười", "cuoi nuoc mat tears joy lol funny hai"],
  ["🤣", "Cười lăn", "Mặt cười", "cuoi lan rofl funny"],
  ["😊", "Mỉm cười", "Mặt cười", "mim cuoi vui blush smile happy"],
  ["😍", "Mắt trái tim", "Mặt cười", "yeu thich love heart eyes crush"],
  ["🥰", "Đang yêu", "Mặt cười", "yeu thuong love hearts adore"],
  ["😘", "Hôn", "Mặt cười", "hon kiss love"],
  ["😎", "Ngầu", "Mặt cười", "ngau cool sunglasses"],
  ["🤔", "Suy nghĩ", "Mặt cười", "suy nghi think thinking hmm"],
  ["🫡", "Chào nghiêm", "Mặt cười", "chao nghiem salute respect"],
  ["😅", "Cười ngại", "Mặt cười", "cuoi ngai sweat nervous relief"],
  ["😭", "Khóc lớn", "Mặt cười", "khoc buon cry sob tears"],
  ["😢", "Buồn", "Mặt cười", "buon khoc sad cry"],
  ["😡", "Tức giận", "Mặt cười", "tuc gian angry mad rage"],
  ["😱", "Sợ hãi", "Mặt cười", "so hai shock scream scared"],
  ["🥳", "Ăn mừng", "Mặt cười", "an mung tiec party celebrate birthday"],
  ["🤯", "Nổ não", "Mặt cười", "no nao mind blown shock"],
  ["🤫", "Im lặng", "Mặt cười", "im lang quiet shush secret"],
  ["🫠", "Tan chảy", "Mặt cười", "tan chay melting awkward hot"],
  ["💀", "Đầu lâu", "Mặt cười", "dau lau chet skull dead funny"],
  ["👍", "Thích", "Cử chỉ", "thich dong y yes like thumbs up ok tot"],
  ["👎", "Không thích", "Cử chỉ", "khong thich dislike thumbs down no"],
  ["👌", "OK", "Cử chỉ", "ok duoc perfect dong y"],
  ["✌️", "Chiến thắng", "Cử chỉ", "chien thang victory peace two"],
  ["🤞", "Chúc may mắn", "Cử chỉ", "chuc may man luck fingers crossed"],
  ["👏", "Vỗ tay", "Cử chỉ", "vo tay clap applause congrats"],
  ["🙏", "Cảm ơn", "Cử chỉ", "cam on cau nguyen please thanks pray"],
  ["💪", "Mạnh mẽ", "Cử chỉ", "manh me co bap strong muscle fight"],
  ["👋", "Vẫy tay", "Cử chỉ", "vay tay chao hello goodbye wave"],
  ["🤝", "Bắt tay", "Cử chỉ", "bat tay hop tac handshake deal"],
  ["🫶", "Tay trái tim", "Cử chỉ", "tay trai tim yeu love heart hands"],
  ["🤌", "Chụm ngón tay", "Cử chỉ", "chum ngon tay italian perfect"],
  ["❤️", "Trái tim đỏ", "Biểu tượng", "trai tim do yeu love heart red"],
  ["🧡", "Trái tim cam", "Biểu tượng", "trai tim cam love orange"],
  ["💛", "Trái tim vàng", "Biểu tượng", "trai tim vang love yellow"],
  ["💚", "Trái tim xanh lá", "Biểu tượng", "trai tim xanh la love green"],
  ["💙", "Trái tim xanh", "Biểu tượng", "trai tim xanh love blue"],
  ["💜", "Trái tim tím", "Biểu tượng", "trai tim tim love purple"],
  ["🖤", "Trái tim đen", "Biểu tượng", "trai tim den love black"],
  ["💔", "Tan vỡ", "Biểu tượng", "tan vo that tinh broken heart"],
  ["🔥", "Lửa", "Biểu tượng", "lua hot fire trend xuat sac"],
  ["✨", "Lấp lánh", "Biểu tượng", "lap lanh sparkle magic new"],
  ["⭐", "Ngôi sao", "Biểu tượng", "ngoi sao star favorite"],
  ["✅", "Hoàn thành", "Biểu tượng", "hoan thanh xong dung check done yes"],
  ["❌", "Sai", "Biểu tượng", "sai huy khong cross cancel no"],
  ["⚠️", "Cảnh báo", "Biểu tượng", "canh bao warning danger attention"],
  ["❓", "Câu hỏi", "Biểu tượng", "cau hoi question help"],
  ["💡", "Ý tưởng", "Đồ vật", "y tuong sang kien idea light bulb"],
  ["🎉", "Pháo giấy", "Sự kiện", "phao giay chuc mung celebrate party congrats"],
  ["🎂", "Bánh sinh nhật", "Sự kiện", "banh sinh nhat birthday cake"],
  ["🎁", "Quà tặng", "Sự kiện", "qua tang gift present birthday"],
  ["🚀", "Tên lửa", "Du lịch", "ten lua rocket launch nhanh startup"],
  ["✈️", "Máy bay", "Du lịch", "may bay plane flight travel"],
  ["🚗", "Ô tô", "Du lịch", "o to xe hoi car drive"],
  ["🏠", "Ngôi nhà", "Địa điểm", "ngoi nha home house"],
  ["📍", "Vị trí", "Địa điểm", "vi tri dia diem pin location map"],
  ["🌍", "Trái đất", "Thiên nhiên", "trai dat earth world globe"],
  ["☀️", "Mặt trời", "Thiên nhiên", "mat troi nang sunny sun weather"],
  ["🌙", "Mặt trăng", "Thiên nhiên", "mat trang dem moon night"],
  ["🌧️", "Mưa", "Thiên nhiên", "mua rain weather"],
  ["🌈", "Cầu vồng", "Thiên nhiên", "cau vong rainbow pride"],
  ["🌸", "Hoa anh đào", "Thiên nhiên", "hoa anh dao flower blossom spring"],
  ["🐶", "Chó", "Động vật", "cho dog puppy pet"],
  ["🐱", "Mèo", "Động vật", "meo cat kitten pet"],
  ["🐼", "Gấu trúc", "Động vật", "gau truc panda cute"],
  ["🍕", "Pizza", "Đồ ăn", "pizza do an food"],
  ["🍔", "Hamburger", "Đồ ăn", "hamburger burger do an food"],
  ["🍜", "Mì", "Đồ ăn", "mi pho noodle ramen food"],
  ["☕", "Cà phê", "Đồ uống", "ca phe coffee drink morning"],
  ["🍺", "Bia", "Đồ uống", "bia beer drink cheers"],
  ["💻", "Laptop", "Công nghệ", "laptop may tinh computer code work"],
  ["⌨️", "Bàn phím", "Công nghệ", "ban phim keyboard type code"],
  ["🐛", "Lỗi phần mềm", "Công nghệ", "loi phan mem bug insect debug"],
  ["🔧", "Công cụ", "Đồ vật", "cong cu sua chua tool fix wrench"],
  ["📌", "Ghim", "Đồ vật", "ghim pin important"],
  ["📎", "Đính kèm", "Đồ vật", "dinh kem attachment paperclip"],
  ["📝", "Ghi chú", "Đồ vật", "ghi chu note write memo"],
  ["📅", "Lịch", "Đồ vật", "lich calendar date schedule"],
  ["⏰", "Báo thức", "Đồ vật", "bao thuc dong ho alarm clock time"],
  ["🔒", "Khóa", "Đồ vật", "khoa bao mat lock secure private"],
  ["🔍", "Tìm kiếm", "Đồ vật", "tim kiem search find magnify"],
];

function normalize(value: string): string {
  return value.normalize("NFD").replace(/[\u0300-\u036f]/g, "").toLowerCase().trim();
}

export function findEmojis(query: string): ResultItemData[] {
  const q = normalize(query);
  const terms = q.split(/\s+/).filter(Boolean);
  const matches: Array<{ score: number; item: ResultItemData }> = [];
  EMOJIS.forEach(([emoji, name, category, keywords], index) => {
    const haystack = normalize(`${name} ${category} ${keywords}`);
    if (terms.some((term) => !haystack.includes(term))) return;
    let score = 10_000 - index;
    if (q && normalize(name) === q) score += 10_000;
    else if (q && normalize(name).startsWith(q)) score += 5_000;
    else if (q && haystack.split(/\s+/).some((word) => word === q)) score += 3_000;
    matches.push({ score, item: {
      id: `emoji:${emoji}`, title: `${emoji}  ${name}`,
      subtitle: `${category} · Enter để dán`, kind: "emoji" as const, text: emoji,
    }});
  });
  return matches.sort((a, b) => b.score - a.score)
    .slice(0, 30)
    .map(({ item }) => item);
}
