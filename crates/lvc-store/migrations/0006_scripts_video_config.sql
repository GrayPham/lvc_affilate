-- Chon engine tao video (dola_video/veo3_video/google_video/seedance_video...) + phan loai
-- cho tung kich ban. video_kind/video_params rong (video_params = '') nghia la "chua dat":
-- neu video_kind='dola_video' thi dung AppSettings mac dinh (hanh vi cu, xem lvc-engine::scripts
-- effective_video_params); kind khac thi lay default_params tu NodeManifest (lvc-app).
-- category la text tu do, khong rang buoc enum (giong cach templates gan category tinh).
ALTER TABLE scripts ADD COLUMN video_kind TEXT NOT NULL DEFAULT 'dola_video';
ALTER TABLE scripts ADD COLUMN video_params TEXT NOT NULL DEFAULT '';
ALTER TABLE scripts ADD COLUMN category TEXT NOT NULL DEFAULT '';
