//! Lỗi của client giao thức Muse — port từ `errors.py` (tham khảo, CHỈ ĐỌC).

#[derive(Debug, thiserror::Error)]
pub enum MuseError {
    #[error("loi xac thuc Muse: {0}")]
    Auth(String),
    #[error("loi giao thuc Muse: {0}")]
    Protocol(String),
    #[error("Hatch RPC loi: {message} (status={status:?})")]
    Rpc {
        message: String,
        status: Option<u16>,
        body: Vec<u8>,
    },
}
