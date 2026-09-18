//! Windows Job Object (Task 13, review P1 I-7): gan tien trinh Chrome vao mot Job co
//! `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` de khi tien trinh NAY chet bat ngo (crash, `process::exit`,
//! bi kill) thi he dieu hanh dong handle Job → giet ca cay Chrome → khong con Chrome mo coi giu
//! `lockfile` cua profile (Python/patchright dua vao pipe stdio; ta noi CDP qua cong TCP nen khong
//! co co che do).
//!
//! Vong doi: [`JobHandle`] song trong `ChromiumSession`. `close()` dong Chrome NHE NHANG truoc
//! (`Browser.close` + doi thoat) roi moi tha handle — luc do Job da rong, khong giet gi. Chi khi
//! Chrome khong thoat kip / tien trinh nay chet thi Job moi la luoi cuoi.
//!
//! Gioi han da biet: chromiumoxide chi tra `Child` SAU khi Chrome in websocket URL, luc do Chrome
//! da spawn vai tien trinh con (GPU, network, renderer dau) — chung KHONG nam trong Job (Windows chi
//! tu gan con sinh SAU khi cha vao Job). Thuc te khong sao: tien trinh con cua Chrome tu thoat khi
//! mat kenh mojo voi browser process.
//!
//! Ho tro Job long nhau (Windows 8+): tien trinh nay co the dang nam trong Job cua terminal/CI;
//! `AssignProcessToJobObject` van thanh cong (Job con). Loi → caller `tracing::warn`, khong fatal.

use std::io;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};

use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
    SetInformationJobObject,
};
use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_SET_QUOTA, PROCESS_TERMINATE};

/// Handle Job Object; `Drop` (qua `OwnedHandle` → `CloseHandle`) dong Job → giet moi tien trinh
/// con lai trong Job (`KILL_ON_JOB_CLOSE`). Khong `Clone` — dung mot handle duy nhat de "dong
/// handle cuoi" xay ra dung luc `ChromiumSession` bi drop.
#[derive(Debug)]
pub struct JobHandle {
    _job: OwnedHandle,
}

/// Tao Job `KILL_ON_JOB_CLOSE` va gan tien trinh `pid` vao. Handle Job KHONG ke thua
/// (`lpJobAttributes = null`) de tien trinh con khac khong vo tinh giu Job song.
///
/// # Errors
/// Loi Win32 (`GetLastError`) cua `CreateJobObjectW` / `SetInformationJobObject` / `OpenProcess`
/// (pid khong ton tai hoac thieu quyen `PROCESS_SET_QUOTA | PROCESS_TERMINATE`) /
/// `AssignProcessToJobObject` (tien trinh da o trong Job khong cho long nhau).
// Ly do unsafe: FFI Win32 qua windows-sys; moi handle boc `OwnedHandle` ngay sau khi nhan de
// dong dung ca tren duong loi. Workspace dat `unsafe_code = "warn"` (gate -D warnings) nen phai
// allow hep o day (PF-3).
#[allow(unsafe_code)]
pub fn assign_kill_on_close(pid: u32) -> io::Result<JobHandle> {
    // SAFETY: goi Win32 voi tham so null hop le (khong ten, khong SECURITY_ATTRIBUTES); tra HANDLE
    // moi thuoc ve ta (kiem null truoc khi boc).
    let raw = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
    if raw.is_null() {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: `raw` la handle hop le vua nhan, chua ai khac so huu.
    let job = unsafe { OwnedHandle::from_raw_handle(raw) };

    // SAFETY: struct POD Win32, gia tri toan 0 hop le; chi dat LimitFlags.
    let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { std::mem::zeroed() };
    info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
    let len = u32::try_from(std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>())
        .map_err(io::Error::other)?;
    // SAFETY: `info` song suot loi goi; `len` dung kich thuoc struct; handle Job hop le.
    let ok = unsafe {
        SetInformationJobObject(
            job.as_raw_handle(),
            JobObjectExtendedLimitInformation,
            (&raw const info).cast(),
            len,
        )
    };
    if ok == 0 {
        return Err(io::Error::last_os_error());
    }

    // SAFETY: OpenProcess voi quyen toi thieu de gan vao Job; kiem null truoc khi boc.
    let raw = unsafe { OpenProcess(PROCESS_SET_QUOTA | PROCESS_TERMINATE, 0, pid) };
    if raw.is_null() {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: handle tien trinh hop le vua nhan; dong khi ra khoi ham (Job giu tham chieu rieng).
    let process = unsafe { OwnedHandle::from_raw_handle(raw) };
    // SAFETY: ca hai handle deu hop le va con song.
    let ok = unsafe { AssignProcessToJobObject(job.as_raw_handle(), process.as_raw_handle()) };
    if ok == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(JobHandle { _job: job })
}

#[cfg(test)]
mod tests {
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    use super::*;

    /// Tien trinh con song ~30 s (`ping -n 31 127.0.0.1`, khong can stdin nhu `timeout`); gan vao
    /// Job roi DROP handle → con phai chet trong 2 s. Khong can Chrome.
    #[test]
    fn drop_handle_giet_tien_trinh_con() {
        let mut child = Command::new("ping")
            .args(["-n", "31", "127.0.0.1"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn ping");
        let job = assign_kill_on_close(child.id()).expect("assign_kill_on_close");
        assert!(
            child.try_wait().expect("try_wait").is_none(),
            "con phai con song truoc khi drop Job"
        );

        drop(job);

        let t0 = Instant::now();
        let mut status = None;
        while t0.elapsed() < Duration::from_secs(2) {
            status = child.try_wait().expect("try_wait");
            if status.is_some() {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        if status.is_none() {
            let _ = child.kill(); // don rac truoc khi panic
            let _ = child.wait();
            panic!("tien trinh con van song 2 s sau khi dong Job");
        }
    }

    /// pid 0 (System Idle) khong mo duoc voi `PROCESS_SET_QUOTA|TERMINATE` → Err, khong panic.
    #[test]
    fn pid_khong_hop_le_tra_err() {
        let err = assign_kill_on_close(0).expect_err("pid 0 phai loi");
        assert!(err.raw_os_error().is_some(), "{err}");
    }
}
