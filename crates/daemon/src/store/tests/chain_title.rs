//! An epic's title in the store: rows older than titles load without one and show the name made
//! readable; a saved title comes back, and clearing it goes back to the name.

use super::*;

#[tokio::test]
async fn the_migration_keeps_old_rows_and_they_show_their_name_made_readable() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("toknila.db");
    let project = ProjectId::new();
    {
        let conn = Connection::open(&path).expect("raw open");
        // The legacy files only: the title column does not exist yet.
        migrations::old_store(&conn, 24);
        conn.execute(
            "INSERT INTO projects (id, key, name, repo_path, work_branch, release_branch, \
             default_profile, default_review_mode) VALUES (?1, 'AAA', 'AAA', NULL, 'develop', \
             'main', 'sim', 'human')",
            params![project.to_string()],
        )
        .expect("project");
        conn.execute(
            "INSERT INTO chains (id, project_id, name, goal, autostart, review_mode) \
             VALUES (?1, ?2, '010-midi-controllers', '', 1, 'both')",
            params![ChainId::new().to_string(), project.to_string()],
        )
        .expect("old chain");
    }
    let store = Store::open(&path).expect("migrates");
    let chains = store
        .read(move |repo| repo.chains(Some(project)))
        .await
        .expect("chains");
    assert_eq!(chains.len(), 1);
    assert_eq!(chains[0].title, None);
    assert_eq!(chains[0].display_title(), "Midi controllers");

    let mut c = chains[0].clone();
    c.title = Some("MIDI controllers".to_owned());
    let id = c.id;
    store
        .write(Actor::Human, NOW, move |tx| tx.repo().update_chain(&c))
        .await
        .expect("set title");
    let back = store
        .read(move |repo| repo.chain(id))
        .await
        .expect("chain")
        .expect("exists");
    assert_eq!(back.title.as_deref(), Some("MIDI controllers"));
    assert_eq!(back.name, "010-midi-controllers");
    assert_eq!(back.display_title(), "MIDI controllers");
}
