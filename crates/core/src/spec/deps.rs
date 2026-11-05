//! The dependency rule between phases.

/// For each phase (given as "is it a user story"), the phases it waits for. A story waits for
/// the last non-story phase before it, so stories run in parallel. A non-story phase waits for
/// the stories since the previous non-story phase, or for that phase itself when there are none.
pub fn phase_deps(is_story: &[bool]) -> Vec<Vec<usize>> {
    let mut deps = Vec::with_capacity(is_story.len());
    let mut last_plain: Option<usize> = None;
    let mut stories: Vec<usize> = Vec::new();
    for (i, story) in is_story.iter().enumerate() {
        if *story {
            deps.push(last_plain.into_iter().collect());
            stories.push(i);
        } else {
            if stories.is_empty() {
                deps.push(last_plain.into_iter().collect());
            } else {
                deps.push(std::mem::take(&mut stories));
            }
            last_plain = Some(i);
        }
    }
    deps
}
