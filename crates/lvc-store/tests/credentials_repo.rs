//! `CredentialRepo` (P4c): CRUD + di tru schema tu DB P3 (0001..0003 -> 0004).

use lvc_store::{CredentialRepo, Db, NewCredential};

fn cred(id: &str, provider: &str) -> NewCredential {
    NewCredential {
        id: id.to_string(),
        provider: provider.to_string(),
        kind: "api_key".to_string(),
        label: format!("{provider}-1"),
        secret: b"CIPHERTEXT".to_vec(),
        meta: r#"{"base_url":"x"}"#.to_string(),
        proxy: String::new(),
    }
}

#[tokio::test]
async fn insert_list_get_theo_provider() {
    let db = Db::open_memory().await.unwrap();
    let repo = CredentialRepo::new(&db);
    repo.insert(&cred("c1", "openai")).await.unwrap();
    repo.insert(&cred("c2", "gemini")).await.unwrap();
    repo.insert(&cred("c3", "openai")).await.unwrap();

    assert_eq!(repo.list(None).await.unwrap().len(), 3);
    let oa = repo.list(Some("openai")).await.unwrap();
    assert_eq!(oa.len(), 2);
    assert!(oa.iter().all(|c| c.provider == "openai"));

    let g = repo.get("c2").await.unwrap().unwrap();
    assert_eq!(g.secret, b"CIPHERTEXT");
    assert!(g.enabled);
    assert_eq!(g.status, "unknown");
    assert!(repo.get("khong-co").await.unwrap().is_none());
}

#[tokio::test]
async fn update_secret_status_enabled_proxy_delete() {
    let db = Db::open_memory().await.unwrap();
    let repo = CredentialRepo::new(&db);
    repo.insert(&cred("c1", "dola")).await.unwrap();

    assert!(
        repo.update_secret("c1", b"NEW", r#"{"a":1}"#)
            .await
            .unwrap()
    );
    assert!(repo.set_status("c1", "ok").await.unwrap());
    assert!(repo.set_enabled("c1", false).await.unwrap());
    assert!(repo.set_proxy("c1", "http://1.2.3.4:8080").await.unwrap());

    let c = repo.get("c1").await.unwrap().unwrap();
    assert_eq!(c.secret, b"NEW");
    assert_eq!(c.meta, r#"{"a":1}"#);
    assert_eq!(c.status, "ok");
    assert!(!c.enabled);
    assert!(c.last_verified_at > 0.0);
    assert_eq!(c.proxy, "http://1.2.3.4:8080");

    assert!(repo.exists_for("dola", "dola-1").await.unwrap());
    assert!(!repo.exists_for("dola", "khac").await.unwrap());

    assert!(repo.delete("c1").await.unwrap());
    assert!(!repo.delete("c1").await.unwrap());
    assert!(repo.get("c1").await.unwrap().is_none());
}

#[tokio::test]
async fn debug_khong_lo_secret() {
    let db = Db::open_memory().await.unwrap();
    let repo = CredentialRepo::new(&db);
    repo.insert(&cred("c1", "openai")).await.unwrap();
    let c = repo.get("c1").await.unwrap().unwrap();
    let dbg = format!("{c:?}");
    assert!(!dbg.contains("CIPHERTEXT"), "Debug lo secret: {dbg}");
    assert!(dbg.contains("bytes"));
}
