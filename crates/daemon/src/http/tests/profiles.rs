//! `GET /v1/profiles` through the full router: built-in profiles, the folder's files, and the
//! files that are not usable, named with their line.

use serde_json::json;

use super::Api;

fn write(api: &Api, name: &str, text: &str) {
    let dir = api._dir.path().join("profiles");
    std::fs::create_dir_all(&dir).expect("mkdir");
    std::fs::write(dir.join(name), text).expect("write profile");
}

#[tokio::test]
async fn without_a_folder_only_the_built_in_profiles_are_listed() {
    let api = Api::new();
    let list = api.get("/v1/profiles").await;
    let names: Vec<_> = list["profiles"]
        .as_array()
        .expect("profiles")
        .iter()
        .map(|p| p["profile"]["name"].clone())
        .collect();
    assert_eq!(
        names,
        [
            json!("sim"),
            json!("claude-code"),
            json!("sim-reviewer"),
            json!("claude-code-reviewer")
        ]
    );
    assert_eq!(list["problems"], json!([]));
    assert!(
        list["folder"]
            .as_str()
            .expect("folder")
            .ends_with("profiles"),
        "{list}"
    );
}

#[tokio::test]
async fn files_are_listed_and_bad_ones_are_named_with_their_line() {
    let api = Api::new();
    write(&api, "my-agent.toml", "command = [\"my-agent\", \"--x\"]\n");
    write(&api, "broken.toml", "command = [\"a\"]\nnope = 1\n");
    let list = api.get("/v1/profiles").await;
    let mine = &list["profiles"][4];
    assert_eq!(mine["file"], "my-agent.toml");
    assert_eq!(mine["profile"]["command"], json!(["my-agent", "--x"]));
    assert_eq!(list["profiles"].as_array().expect("profiles").len(), 5);
    let problem = &list["problems"][0];
    assert_eq!(
        (&problem["file"], &problem["line"], &problem["column"]),
        (&json!("broken.toml"), &json!(2), &json!(1)),
        "{problem}"
    );
    assert!(
        problem["message"]
            .as_str()
            .expect("message")
            .contains("nope")
    );
}
