//! The roadmap order of chains in the store: the migration keeps today's order, a new chain goes
//! to the end of its project, and every list follows `position`.

use super::*;

fn named(project: ProjectId, name: &str) -> Chain {
    Chain {
        name: name.to_owned(),
        ..chain(project)
    }
}

async fn names(store: &Store, project: Option<ProjectId>) -> Vec<String> {
    store
        .read(move |repo| repo.chains(project))
        .await
        .expect("chains")
        .into_iter()
        .map(|c| c.name)
        .collect()
}

#[tokio::test]
async fn the_migration_numbers_each_projects_chains_in_the_order_they_were_made() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("toknila.db");
    let (a, b) = (ProjectId::new(), ProjectId::new());
    {
        let conn = Connection::open(&path).expect("raw open");
        // The legacy files only: the roadmap column does not exist yet.
        migrations::old_store(&conn, 24);
        for (key, id) in [("AAA", a), ("BBB", b)] {
            conn.execute(
                "INSERT INTO projects (id, key, name, repo_path, work_branch, release_branch, \
                 default_profile, default_review_mode) VALUES (?1, ?2, ?2, NULL, 'develop', \
                 'main', 'sim', 'human')",
                params![id.to_string(), key],
            )
            .expect("project");
        }
        // Interleaved between the projects, as they are made over time.
        for (project, name) in [(a, "a1"), (b, "b1"), (a, "a2"), (a, "a3"), (b, "b2")] {
            conn.execute(
                "INSERT INTO chains (id, project_id, name, goal, autostart, review_mode) \
                 VALUES (?1, ?2, ?3, '', 1, 'both')",
                params![ChainId::new().to_string(), project.to_string(), name],
            )
            .expect("chain");
        }
    }
    let store = Store::open(&path).expect("migrates");
    assert_eq!(names(&store, Some(a)).await, ["a1", "a2", "a3"]);
    assert_eq!(names(&store, Some(b)).await, ["b1", "b2"]);
    let positions: Vec<(String, u32)> = store
        .read(move |repo| repo.chains(Some(a)))
        .await
        .expect("chains")
        .into_iter()
        .map(|c| (c.name, c.position))
        .collect();
    assert_eq!(
        positions,
        [
            ("a1".to_owned(), 0),
            ("a2".to_owned(), 1),
            ("a3".to_owned(), 2)
        ]
    );
}

#[tokio::test]
async fn a_saved_position_reorders_every_list_of_chains() {
    let (_dir, store) = open_temp();
    let p = project();
    let ids: Vec<Chain> = ["one", "two", "three"]
        .iter()
        .zip(0..)
        .map(|(n, position)| Chain {
            position,
            ..named(p.id, n)
        })
        .collect();
    let (p2, cs) = (p.clone(), ids.clone());
    store
        .write(Actor::Human, NOW, move |tx| {
            let repo = tx.repo();
            repo.insert_project(&p2)?;
            for c in &cs {
                repo.insert_chain(c)?;
            }
            Ok::<(), StoreError>(())
        })
        .await
        .expect("write");
    assert_eq!(names(&store, Some(p.id)).await, ["one", "two", "three"]);

    let late = Chain {
        position: 3,
        ..named(p.id, "four")
    };
    store
        .write(Actor::Human, NOW, move |tx| tx.repo().insert_chain(&late))
        .await
        .expect("insert");
    assert_eq!(names(&store, None).await, ["one", "two", "three", "four"]);

    // A saved position moves the chain in every list, including the one over all projects.
    let mut three = ids[2].clone();
    three.position = 0;
    let mut one = ids[0].clone();
    one.position = 1;
    store
        .write(Actor::Human, NOW, move |tx| {
            tx.repo().update_chain(&three)?;
            tx.repo().update_chain(&one)
        })
        .await
        .expect("move");
    assert_eq!(
        names(&store, Some(p.id)).await,
        ["three", "one", "two", "four"]
    );
    assert_eq!(names(&store, None).await, ["three", "one", "two", "four"]);
}
