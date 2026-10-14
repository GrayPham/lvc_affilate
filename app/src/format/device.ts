// Hien thi ma thiet bi (`commands::device`) — THUAN, khong React/store, de test truc tiep.
// Dat o `format/` theo dung quy uoc cac ham hien thi khac (`format/task.ts`, `format/credential.ts`).

/**
 * `A1B2···9E422A` — dang rut gon khop ban thiet ke, vua be ngang thanh ben.
 *
 * Ma KHONG phai bi mat (chi la dinh danh mot lan cai dat), rut gon chi de gon mat; ban day du van
 * doc duoc qua tooltip va nut "Chép". Ma NGAN hon 10 ky tu thi giu nguyen — rut gon mot chuoi von
 * da ngan chi lam mat thong tin ma khong gon hon.
 */
export function maskDeviceId(id: string): string {
  if (id.length <= 10) {
    return id;
  }
  return `${id.slice(0, 4)}···${id.slice(-6)}`;
}
