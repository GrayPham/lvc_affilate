// Đếm số cảnh từ một khối prompt — NGUỒN DUY NHẤT trong cả app.
//
// Trước 2026-10-14 có HAI bản: một trong `nodes/PromptNode.tsx` (bỏ qua dòng `#`) và một trong
// `components/AddWorkflowDialog.tsx` (không bỏ). Cùng một khối text cho ra hai con số khác nhau —
// hộp thoại "Thêm workflow" nói "5 cảnh" trong khi node chạy đúng 3. Gộp về đây để điều đó không
// lặp lại được nữa.
//
// Đây là ƯỚC LƯỢNG để hiển thị, KHÔNG phải nguồn sự thật: `workflow_validate` (Rust) mới là nơi
// chốt số cảnh thật khi bấm "Kiểm tra"/"Chạy".

/**
 * Bỏ qua dòng trống VÀ dòng bắt đầu bằng `#` (sau khi `trim`) — khớp đúng
 * `scene_txt::parse_txt` (crates/lvc-core/src/scene_txt.rs), nơi cả hai loại dòng đó đều KHÔNG
 * phải một cảnh.
 */
export function countScenes(text: string): number {
  return text.split("\n").filter((line) => {
    const trimmed = line.trim();
    return trimmed.length > 0 && !trimmed.startsWith("#");
  }).length;
}
