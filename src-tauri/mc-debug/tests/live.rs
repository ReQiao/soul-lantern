use soul_lantern_mc_debug::{Cancellation, Engine, ProcessControl};

#[test]
#[ignore = "仅由手动 GitHub Actions 启动真实 Minecraft 服务端"]
fn official_server_validates_and_cleans_up() {
    let key = std::env::var("MC_DEBUG_VERSION").expect("MC_DEBUG_VERSION");
    let cache = tempfile::Builder::new()
        .prefix("Soul Lantern debug ")
        .tempdir()
        .unwrap();
    let control = ProcessControl::default();
    let cancel = Cancellation::default();
    let mut engine = Engine::new(cache.path().to_path_buf(), control.clone());
    let progress = |text: &str| println!("{text}");
    let good = engine
        .validate(&key, "/give @s minecraft:stone", &cancel, &progress)
        .unwrap();
    assert_eq!(good.status, "syntax_ok", "{good:?}");
    let bad = engine
        .validate(
            &key,
            "/give @s minecraft:this_item_does_not_exist",
            &cancel,
            &progress,
        )
        .unwrap();
    assert_eq!(bad.status, "syntax_error", "{bad:?}");
    let long = format!(
        "give @s minecraft:stone[minecraft:custom_name='\"{}\"']",
        "x".repeat(12000)
    );
    let long = engine.validate(&key, &long, &cancel, &progress).unwrap();
    assert_eq!(long.status, "syntax_ok", "{long:?}");
    // 关闭后取消旧任务，不复用旧世界；再次开启应重新启动并正常验证。
    cancel.cancel();
    control.stop();
    assert!(
        engine
            .validate(&key, "say old_session", &cancel, &progress)
            .is_err()
    );
    let fresh = Cancellation::default();
    assert_eq!(
        engine
            .validate(&key, "say fresh_session", &fresh, &progress)
            .unwrap()
            .status,
        "syntax_ok"
    );
    engine.stop();
    assert!(
        !std::fs::read_dir(cache.path())
            .unwrap()
            .flatten()
            .any(|p| p.file_name().to_string_lossy().starts_with("world-"))
    );
}
