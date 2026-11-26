//! A repository checked out in the current directory must not be able to redirect the user's
//! API key. Each test runs the real binary with an empty home and two mock servers: the user's
//! Plane server, and the server named by the file in the current directory.

use mockito::{Matcher, Mock, Server, ServerGuard};
use std::path::Path;
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

const USER_KEY: &str = "plane_api_user_key_for_trust_test";
const MEMBER_KEY: &str = "plane_api_member_key_for_trust_test";
const REPO_KEY: &str = "plane_api_repo_key_for_trust_test";

struct Fixture {
    home: tempfile::TempDir,
    repo: tempfile::TempDir,
    user: ServerGuard,
    attacker: ServerGuard,
}

impl Fixture {
    fn new() -> Self {
        Fixture {
            home: tempfile::tempdir().unwrap(),
            repo: tempfile::tempdir().unwrap(),
            user: Server::new(),
            attacker: Server::new(),
        }
    }

    /// The global env file, pointing at the user's server.
    fn write_global(&self, with_key: bool) {
        let dir = self.home.path().join("config/plane-cli");
        std::fs::create_dir_all(&dir).unwrap();
        let key = if with_key {
            format!("PLANE_API_KEY={USER_KEY}\n")
        } else {
            String::new()
        };
        std::fs::write(
            dir.join(".plane-cli.env"),
            format!(
                "PLANE_API_BASE_URL={}\nPLANE_WORKSPACE_SLUG=acme\n{key}",
                self.user.url()
            ),
        )
        .unwrap();
    }

    fn write_repo_file(&self, name: &str, body: &str) {
        std::fs::write(self.repo.path().join(name), body).unwrap();
    }

    fn run(&self, envs: &[(&str, &str)], args: &[&str]) -> Output {
        self.run_in(self.repo.path(), envs, args)
    }

    fn command(&self, cwd: &Path, envs: &[(&str, &str)], args: &[&str]) -> Command {
        let home: &Path = self.home.path();
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_plane-cli"));
        cmd.env_clear()
            .current_dir(cwd)
            .env("HOME", home)
            .env("XDG_CONFIG_HOME", home.join("config"))
            .env("XDG_CACHE_HOME", home.join("cache"))
            .env("PLANE_CACHE", "0")
            .env("PLANE_CLI_ANON_TELEMETRY", "0")
            .args(args);
        for (k, v) in envs {
            cmd.env(k, v);
        }
        cmd
    }

    fn run_in(&self, cwd: &Path, envs: &[(&str, &str)], args: &[&str]) -> Output {
        self.command(cwd, envs, args).output().unwrap()
    }

    /// Fails instead of hanging the test run if the binary blocks.
    fn run_bounded(&self, args: &[&str]) -> Output {
        let mut child = self
            .command(self.repo.path(), &[], args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(15);
        while child.try_wait().unwrap().is_none() {
            if Instant::now() > deadline {
                child.kill().unwrap();
                panic!("plane-cli hung");
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        child.wait_with_output().unwrap()
    }
}

fn never(server: &mut ServerGuard) -> Mock {
    server
        .mock("GET", Matcher::Any)
        .with_status(200)
        .with_body("{}")
        .expect(0)
        .create()
}

fn whoami_with_key(server: &mut ServerGuard, key: &str) -> Mock {
    server
        // `auth whoami` reads users/me and then the workspace's projects.
        .mock("GET", Matcher::Regex("^/api/v1/".into()))
        .match_header("x-api-key", key)
        .with_status(200)
        .with_body(
            r#"{"id":"11111111-1111-4111-8111-111111111111","display_name":"bob","results":[],"next_page_results":false}"#,
        )
        .expect_at_least(1)
        .create()
}

fn json(out: &Output) -> serde_json::Value {
    serde_json::from_slice(&out.stdout).unwrap_or_else(|e| {
        panic!(
            "not JSON ({e}): {} / {}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        )
    })
}

#[test]
fn repo_url_never_receives_the_global_key() {
    let mut f = Fixture::new();
    f.write_global(true);
    let url = f.attacker.url();
    f.write_repo_file(".plane-cli.env", &format!("PLANE_API_BASE_URL={url}\n"));
    let bad = never(&mut f.attacker);
    let good = whoami_with_key(&mut f.user, USER_KEY);

    let v = json(&f.run(&[], &["--json", "auth", "whoami"]));

    assert_eq!(v["ok"], true, "{v}");
    bad.assert();
    good.assert();
    assert!(v["warnings"].to_string().contains("was ignored"), "{v}");
}

#[test]
fn repo_url_with_a_repo_key_never_receives_a_process_key() {
    let mut f = Fixture::new();
    f.write_global(false);
    let url = f.attacker.url();
    f.write_repo_file(
        ".plane-cli.env",
        &format!("PLANE_API_BASE_URL={url}\nPLANE_API_KEY={REPO_KEY}\n"),
    );
    let bad = never(&mut f.attacker);
    let good = whoami_with_key(&mut f.user, USER_KEY);

    let v = json(&f.run(
        &[("PLANE_API_KEY", USER_KEY)],
        &["--json", "auth", "whoami"],
    ));

    assert_eq!(v["ok"], true, "{v}");
    bad.assert();
    good.assert();
}

#[test]
fn repo_url_never_receives_a_member_key() {
    let mut f = Fixture::new();
    f.write_global(false);
    let url = f.attacker.url();
    f.write_repo_file(
        ".plane-cli.env",
        &format!("PLANE_API_BASE_URL={url}\nPLANE_API_KEY={REPO_KEY}\n"),
    );
    let bad = never(&mut f.attacker);

    let member_env = [("PLANE_API_KEY_BOB", MEMBER_KEY)];
    let v = json(&f.run(&member_env, &["--json", "--as", "bob", "auth", "whoami"]));
    assert_eq!(v["ok"], false, "{v}");
    assert_eq!(v["error"]["code"], "member_key_untrusted_url", "{v}");

    let d = json(&f.run(&member_env, &["--json", "--as", "bob", "config", "doctor"]));
    assert_eq!(
        d["data"]["identity"]["error"]["code"], "member_key_untrusted_url",
        "{d}"
    );
    bad.assert();
}

#[cfg(all(unix, not(target_os = "macos")))]
#[test]
fn repo_url_never_receives_a_key_file_member_key() {
    use std::os::unix::fs::PermissionsExt;
    let mut f = Fixture::new();
    f.write_global(false);
    let keys = f.home.path().join("config/plane-cli/keys");
    std::fs::create_dir_all(&keys).unwrap();
    std::fs::set_permissions(&keys, std::fs::Permissions::from_mode(0o700)).unwrap();
    let key_file = keys.join("bob");
    std::fs::write(&key_file, MEMBER_KEY).unwrap();
    std::fs::set_permissions(&key_file, std::fs::Permissions::from_mode(0o600)).unwrap();
    let url = f.attacker.url();
    f.write_repo_file(
        ".plane-cli.env",
        &format!("PLANE_API_BASE_URL={url}\nPLANE_API_KEY={REPO_KEY}\n"),
    );
    let bad = never(&mut f.attacker);

    let v = json(&f.run(&[], &["--json", "--as", "bob", "auth", "whoami"]));

    assert_eq!(v["error"]["code"], "member_key_untrusted_url", "{v}");
    bad.assert();
}

#[test]
fn dotenv_url_never_receives_the_global_key() {
    let mut f = Fixture::new();
    f.write_global(true);
    let url = f.attacker.url();
    f.write_repo_file(".env", &format!("PLANE_API_BASE_URL={url}\n"));
    let bad = never(&mut f.attacker);
    let good = whoami_with_key(&mut f.user, USER_KEY);

    let v = json(&f.run(&[("PLANE_DOTENV", "1")], &["--json", "auth", "whoami"]));

    assert_eq!(v["ok"], true, "{v}");
    bad.assert();
    good.assert();
}

#[test]
fn repo_file_cannot_set_a_proxy_identity_or_telemetry() {
    let mut f = Fixture::new();
    f.write_global(true);
    let url = f.attacker.url();
    f.write_repo_file(
        ".plane-cli.env",
        &format!(
            "HTTP_PROXY={url}\nhttp_proxy={url}\nALL_PROXY={url}\nPLANE_CLI_AS=mallory\nPLANE_CLI_ANON_TELEMETRY=1\nPLANE_DOTENV=1\n"
        ),
    );
    f.write_repo_file(".env", &format!("PLANE_API_BASE_URL={url}\n"));
    let bad = never(&mut f.attacker);
    let good = whoami_with_key(&mut f.user, USER_KEY);

    // No telemetry variable in the process env for this run.
    let cmd_env = [("PLANE_CLI_ANON_TELEMETRY", "")];
    let v = json(&f.run(&cmd_env, &["--json", "auth", "whoami"]));
    assert_eq!(v["ok"], true, "{v}");
    bad.assert();
    good.assert();

    let d = json(&f.run(&cmd_env, &["--json", "config", "doctor", "--offline"]));
    assert_eq!(d["data"]["anon_telemetry"]["enabled"], false, "{d}");
    assert!(d["data"]["identity"].is_null(), "{d}");
}

#[test]
fn a_repo_file_with_its_own_url_and_key_still_works() {
    let mut f = Fixture::new();
    let url = f.attacker.url();
    f.write_repo_file(
        ".plane-cli.env",
        &format!("PLANE_API_BASE_URL={url}\nPLANE_API_KEY={REPO_KEY}\nPLANE_WORKSPACE_SLUG=acme\n"),
    );
    let own = whoami_with_key(&mut f.attacker, REPO_KEY);

    let v = json(&f.run(&[], &["--json", "auth", "whoami"]));

    assert_eq!(v["ok"], true, "{v}");
    own.assert();
}

/// Every way of writing a variable reference in an env file.
fn expansion_forms(var: &str) -> Vec<String> {
    vec![
        format!("${{{var}}}"),
        format!("${var}"),
        format!("\"${{{var}}}\""),
        format!("\"${var}\""),
        format!("'${{{var}}}'"),
        format!("'${var}'"),
        format!("x${{{var}}}y"),
    ]
}

#[test]
fn repo_file_cannot_expand_a_secret_into_its_key() {
    for form in expansion_forms("PLANE_API_KEY_BOB") {
        for (file, envs) in [
            (".plane-cli.env", vec![("PLANE_API_KEY_BOB", MEMBER_KEY)]),
            (
                ".env",
                vec![("PLANE_API_KEY_BOB", MEMBER_KEY), ("PLANE_DOTENV", "1")],
            ),
        ] {
            let mut f = Fixture::new();
            f.write_global(true);
            let url = f.attacker.url();
            f.write_repo_file(
                file,
                &format!("PLANE_API_BASE_URL={url}\nPLANE_API_KEY={form}\n"),
            );
            let bad = never(&mut f.attacker);
            // Fails closed: not the repo's server, and no fallback to the user's either.
            let fallback = never(&mut f.user);

            let v = json(&f.run(&envs, &["--json", "auth", "whoami"]));

            assert_eq!(v["error"]["code"], "env_file_unsafe", "{file} {form}: {v}");
            assert!(v["error"]["message"].to_string().contains(file), "{v}");
            assert!(!v.to_string().contains(MEMBER_KEY), "{v}");
            bad.assert();
            fallback.assert();
        }
    }
}

#[test]
fn repo_file_cannot_expand_a_secret_into_its_url() {
    for form in expansion_forms("PLANE_API_KEY") {
        let mut f = Fixture::new();
        f.write_global(false);
        let attacker = f.attacker.url();
        let inner = form.trim_matches(|c| c == '"' || c == '\'');
        let quote = form.chars().next().filter(|c| *c == '"' || *c == '\'');
        let url = match quote {
            Some(q) => format!("{q}{attacker}/{inner}{q}"),
            None => format!("{attacker}/{inner}"),
        };
        f.write_repo_file(
            ".plane-cli.env",
            &format!("PLANE_API_BASE_URL={url}\nPLANE_API_KEY=repo-key-value\n"),
        );
        let bad = never(&mut f.attacker);
        let fallback = never(&mut f.user);

        let v = json(&f.run(
            &[("PLANE_API_KEY", USER_KEY)],
            &["--json", "auth", "whoami"],
        ));

        assert_eq!(v["error"]["code"], "env_file_unsafe", "{form}: {v}");
        assert!(!v.to_string().contains(USER_KEY), "{v}");
        bad.assert();
        fallback.assert();
    }
}

#[test]
fn config_update_never_writes_an_expanded_secret_into_a_repo_file() {
    let f = Fixture::new();
    let body = "PLANE_API_BASE_URL=https://plane.example.com\nPLANE_API_KEY=${PLANE_API_KEY_BOB}\nPLANE_WORKSPACE_SLUG=acme\n";
    f.write_repo_file(".plane-cli.env", body);
    let out = f.run(&[("PLANE_API_KEY_BOB", MEMBER_KEY)], &["config", "update"]);
    assert!(!out.status.success());
    let after = std::fs::read_to_string(f.repo.path().join(".plane-cli.env")).unwrap();
    assert_eq!(after, body);
}

#[test]
fn a_repo_url_in_use_is_named_in_a_warning() {
    let mut f = Fixture::new();
    let url = f.attacker.url();
    f.write_repo_file(
        ".plane-cli.env",
        &format!("PLANE_API_BASE_URL={url}\nPLANE_API_KEY={REPO_KEY}\nPLANE_WORKSPACE_SLUG=acme\n"),
    );
    let _own = whoami_with_key(&mut f.attacker, REPO_KEY);
    let v = json(&f.run(&[], &["--json", "auth", "whoami"]));
    let warnings = v["warnings"].to_string();
    let host = url.trim_start_matches("http://");
    assert!(
        warnings.contains(host) && warnings.contains("untrusted"),
        "{v}"
    );
}

#[test]
fn the_global_file_stays_trusted_when_it_is_the_current_directory() {
    let mut f = Fixture::new();
    f.write_global(false);
    let member = whoami_with_key(&mut f.user, MEMBER_KEY);
    let config_dir = f.home.path().join("config/plane-cli");

    let v = json(&f.run_in(
        &config_dir,
        &[("PLANE_API_KEY_BOB", MEMBER_KEY)],
        &["--json", "--as", "bob", "auth", "whoami"],
    ));

    assert_eq!(v["ok"], true, "{v}");
    member.assert();
}

#[test]
fn the_key_is_not_sent_across_a_redirect_to_another_port() {
    let mut f = Fixture::new();
    f.write_global(true);
    let target = format!("{}/api/v1/users/me/", f.attacker.url());
    let bad = never(&mut f.attacker);
    let redirect = f
        .user
        .mock("GET", "/api/v1/users/me/")
        .with_status(302)
        .with_header("location", &target)
        .expect(1)
        .create();
    let _projects = f
        .user
        .mock("GET", Matcher::Regex("^/api/v1/workspaces/".into()))
        .with_status(200)
        .with_body(r#"{"results":[],"next_page_results":false}"#)
        .create();

    let v = json(&f.run(&[], &["--json", "auth", "whoami"]));

    assert_eq!(v["ok"], false, "{v}");
    assert!(
        v["error"]["message"]
            .to_string()
            .contains("different origin"),
        "{v}"
    );
    bad.assert();
    redirect.assert();
}

#[test]
fn a_scheme_change_is_not_followed() {
    let mut f = Fixture::new();
    f.write_global(true);
    let same_host = f.user.url().replacen("http://", "https://", 1);
    let _redirect = f
        .user
        .mock("GET", "/api/v1/users/me/")
        .with_status(301)
        .with_header("location", &format!("{same_host}/api/v1/users/me/"))
        .create();
    let _projects = f
        .user
        .mock("GET", Matcher::Regex("^/api/v1/workspaces/".into()))
        .with_status(200)
        .with_body(r#"{"results":[],"next_page_results":false}"#)
        .create();

    let v = json(&f.run(&[], &["--json", "auth", "whoami"]));

    // Followed, this would be a TLS failure (`network_error`) rather than a refused redirect.
    assert_eq!(v["error"]["http"]["status"], 301, "{v}");
    assert!(
        v["error"]["message"]
            .to_string()
            .contains("different origin"),
        "{v}"
    );
}

#[test]
fn a_same_origin_redirect_keeps_working() {
    let mut f = Fixture::new();
    f.write_global(true);
    let _redirect = f
        .user
        .mock("GET", "/api/v1/users/me/")
        .with_status(301)
        .with_header("location", "/api/v1/users/me-moved/")
        .create();
    let moved = f
        .user
        .mock("GET", "/api/v1/users/me-moved/")
        .match_header("x-api-key", USER_KEY)
        .with_status(200)
        .with_body(r#"{"id":"11111111-1111-4111-8111-111111111111","display_name":"bob"}"#)
        .expect(1)
        .create();
    let _projects = f
        .user
        .mock("GET", Matcher::Regex("^/api/v1/workspaces/".into()))
        .with_status(200)
        .with_body(r#"{"results":[],"next_page_results":false}"#)
        .create();

    let v = json(&f.run(&[], &["--json", "auth", "whoami"]));

    assert_eq!(v["ok"], true, "{v}");
    moved.assert();
}

#[test]
fn presigned_downloads_follow_redirects_without_the_key() {
    let mut first = Server::new();
    let mut storage = Server::new();
    let target = format!("{}/object", storage.url());
    let _redirect = first
        .mock("GET", "/signed")
        .match_header("x-api-key", Matcher::Missing)
        .with_status(302)
        .with_header("location", &target)
        .create();
    let object = storage
        .mock("GET", "/object")
        .match_header("x-api-key", Matcher::Missing)
        .with_status(200)
        .with_body("file-bytes")
        .expect(1)
        .create();
    let config = plane_core::Config {
        api_base: url::Url::parse(&first.url()).unwrap(),
        api_key: USER_KEY.into(),
        workspace_slug: "acme".into(),
        cache_enabled: false,
        json_output: true,
        acting_as: None,
    };
    let client = plane_core::HttpClient::new(&config).unwrap();

    let bytes = client
        .download_presigned(&format!("{}/signed", first.url()))
        .unwrap();

    assert_eq!(bytes, b"file-bytes");
    object.assert();
}

#[test]
fn a_dollar_in_a_comment_beside_a_setting_fails_closed() {
    let mut f = Fixture::new();
    f.write_global(true);
    f.write_repo_file(".plane-cli.env", "# costs $5\nPLANE_WORKSPACE_SLUG=other\n");
    let fallback = never(&mut f.user);
    let v = json(&f.run(&[], &["--json", "auth", "whoami"]));
    assert_eq!(v["error"]["code"], "env_file_unsafe", "{v}");
    fallback.assert();
}

#[test]
fn a_dollar_only_in_unrelated_keys_is_ignored_with_a_warning() {
    let mut f = Fixture::new();
    f.write_global(true);
    f.write_repo_file(".env", "DATABASE_URL=postgres://$DB_HOST/app\n");
    let good = whoami_with_key(&mut f.user, USER_KEY);
    let v = json(&f.run(&[("PLANE_DOTENV", "1")], &["--json", "auth", "whoami"]));
    assert_eq!(v["ok"], true, "{v}");
    assert!(v["warnings"].to_string().contains("`$`"), "{v}");
    good.assert();
}

#[cfg(unix)]
#[test]
fn a_repo_symlink_is_ignored_and_never_echoes_its_target() {
    let mut f = Fixture::new();
    f.write_global(true);
    let outside = tempfile::tempdir().unwrap();
    let target = outside.path().join("private.txt");
    std::fs::write(&target, "private-line-without-an-equals-sign\n").unwrap();
    std::os::unix::fs::symlink(&target, f.repo.path().join(".plane-cli.env")).unwrap();
    let good = whoami_with_key(&mut f.user, USER_KEY);

    let out = f.run_bounded(&["--json", "auth", "whoami"]);

    let all = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(!all.contains("private-line"), "{all}");
    let v = json(&out);
    assert_eq!(v["ok"], true, "{v}");
    assert!(v["warnings"].to_string().contains("regular file"), "{v}");
    good.assert();

    let w = f.run_bounded(&["--json", "config", "where"]);
    assert!(!String::from_utf8_lossy(&w.stdout).contains("private-line"));
    assert!(!String::from_utf8_lossy(&w.stderr).contains("private-line"));
}

#[cfg(unix)]
#[test]
fn a_fifo_or_device_link_cannot_hang_or_exhaust_memory() {
    let mut f = Fixture::new();
    f.write_global(true);
    let fifo = f.repo.path().join(".plane-cli.env");
    let c = std::ffi::CString::new(fifo.to_str().unwrap()).unwrap();
    // SAFETY: `c` is a valid NUL-terminated path.
    assert_eq!(unsafe { libc::mkfifo(c.as_ptr(), 0o600) }, 0);
    std::os::unix::fs::symlink("/dev/zero", f.repo.path().join(".env")).unwrap();
    let _good = whoami_with_key(&mut f.user, USER_KEY);

    let v = json(&f.run_bounded(&["--json", "--dotenv", "auth", "whoami"]));
    assert_eq!(v["ok"], true, "{v}");

    let w = json(&f.run_bounded(&["--json", "--dotenv", "config", "where"]));
    assert_eq!(w["ok"], true, "{w}");
}

#[test]
fn an_oversized_or_malformed_repo_file_fails_closed_without_echo() {
    let mut f = Fixture::new();
    f.write_global(true);
    let fallback = never(&mut f.user);
    f.write_repo_file(".plane-cli.env", &"#".repeat(64 * 1024 + 1));
    let v = json(&f.run(&[], &["--json", "auth", "whoami"]));
    assert_eq!(v["error"]["code"], "env_file_too_large", "{v}");

    f.write_repo_file(".plane-cli.env", "private-line-without-an-equals-sign\n");
    let v = json(&f.run(&[], &["--json", "auth", "whoami"]));
    assert_eq!(v["error"]["code"], "env_file_invalid", "{v}");
    assert!(!v.to_string().contains("private-line"), "{v}");
    fallback.assert();
}

#[cfg(unix)]
#[test]
fn a_symlinked_global_file_works_from_the_config_directory() {
    let mut f = Fixture::new();
    f.write_global(true);
    let config_dir = f.home.path().join("config/plane-cli");
    let dotfiles = f.home.path().join("dotfiles");
    std::fs::create_dir_all(&dotfiles).unwrap();
    let real = dotfiles.join("plane.env");
    std::fs::rename(config_dir.join(".plane-cli.env"), &real).unwrap();
    std::os::unix::fs::symlink(&real, config_dir.join(".plane-cli.env")).unwrap();
    let good = whoami_with_key(&mut f.user, USER_KEY);

    let v = json(&f.run_in(&config_dir, &[], &["--json", "auth", "whoami"]));
    assert_eq!(v["ok"], true, "{v}");
    good.assert();

    let w = json(&f.run_in(&config_dir, &[], &["--json", "config", "where"]));
    assert_eq!(w["data"]["base_url_source"]["layer"], "global", "{w}");
    let kinds = w["data"]["layers"].to_string();
    assert!(!kinds.contains(r#""kind":"local""#), "{w}");
}
