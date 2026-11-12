use super::*;

const SEC: i64 = 1000;

fn cfg() -> Nudge {
    Nudge {
        idle_secs: 120,
        max_nudges: 3,
        window_secs: 900,
        message: "report".to_owned(),
    }
}

fn at(s: i64) -> Timestamp {
    Timestamp(s * SEC)
}

fn signals() -> Signals {
    Signals {
        started: at(0),
        mcp: None,
        // The first hook came with the start, so the run has shown life.
        hook: Some(at(0)),
        output: None,
        delivered: None,
        screen_open: false,
    }
}

#[test]
fn a_run_that_is_quiet_for_the_threshold_is_nudged() {
    let s = signals();
    assert_eq!(decide(&cfg(), &s, false, &[], at(119)), Decision::Wait);
    assert_eq!(decide(&cfg(), &s, false, &[], at(120)), Decision::Nudge);
}

#[test]
fn a_run_with_no_sign_of_life_is_never_nudged() {
    // No hook, no MCP call: only output (a first-run screen prints too), however long it is quiet.
    let s = Signals {
        hook: None,
        output: Some(at(100)),
        ..signals()
    };
    assert!(!s.alive());
    let sent = [at(120), at(240), at(360)];
    for now in [at(120), at(5000), at(100_000)] {
        assert_eq!(decide(&cfg(), &s, false, &[], now), Decision::Wait);
        assert_eq!(decide(&cfg(), &s, false, &sent, now), Decision::Wait);
    }
    // A delivered instruction is not a sign of life either.
    let s = Signals {
        delivered: Some(at(50)),
        ..s
    };
    assert_eq!(decide(&cfg(), &s, false, &[], at(5000)), Decision::Wait);
}

#[test]
fn the_silence_counts_from_the_first_sign_of_life_not_from_the_start() {
    // Started at 0, the first hook at 1000: the nudge is due at 1120, not at 120.
    let s = Signals {
        hook: Some(at(1000)),
        ..signals()
    };
    assert_eq!(decide(&cfg(), &s, false, &[], at(1119)), Decision::Wait);
    assert_eq!(decide(&cfg(), &s, false, &[], at(1120)), Decision::Nudge);
    // Output (a spinner, say) does not postpone it.
    let s = Signals {
        output: Some(at(1100)),
        ..s
    };
    assert_eq!(decide(&cfg(), &s, false, &[], at(1120)), Decision::Nudge);
}

#[test]
fn an_open_screen_question_is_never_nudged_or_given_up_on() {
    let s = Signals {
        screen_open: true,
        ..signals()
    };
    let sent = [at(120), at(240), at(360)];
    assert_eq!(decide(&cfg(), &s, false, &[], at(5000)), Decision::Wait);
    assert_eq!(decide(&cfg(), &s, false, &sent, at(5000)), Decision::Wait);
    // Control: the same run without the question is nudged.
    let open = Signals {
        screen_open: false,
        ..s
    };
    assert_eq!(decide(&cfg(), &open, false, &[], at(5000)), Decision::Nudge);
}

#[test]
fn the_freshest_of_mcp_and_hook_counts() {
    let s = Signals {
        mcp: Some(at(50)),
        hook: Some(at(200)),
        ..signals()
    };
    assert_eq!(s.last(), at(200));
    assert_eq!(decide(&cfg(), &s, false, &[], at(319)), Decision::Wait);
    assert_eq!(decide(&cfg(), &s, false, &[], at(320)), Decision::Nudge);
}

#[test]
fn a_live_child_process_suppresses_nudge_and_give_up() {
    let s = signals();
    assert_eq!(decide(&cfg(), &s, true, &[], at(5000)), Decision::Wait);
    let sent = [at(120), at(240), at(360)];
    assert_eq!(decide(&cfg(), &s, true, &sent, at(5000)), Decision::Wait);
}

#[test]
fn a_nudge_restarts_the_silence_timer() {
    let s = signals();
    let sent = [at(120)];
    assert_eq!(decide(&cfg(), &s, false, &sent, at(239)), Decision::Wait);
    assert_eq!(decide(&cfg(), &s, false, &sent, at(240)), Decision::Nudge);
}

#[test]
fn a_delivered_instruction_counts_as_a_sign_of_life() {
    let s = Signals {
        delivered: Some(at(500)),
        ..signals()
    };
    assert_eq!(decide(&cfg(), &s, false, &[], at(619)), Decision::Wait);
    assert_eq!(decide(&cfg(), &s, false, &[], at(620)), Decision::Nudge);
}

#[test]
fn after_the_limit_within_the_window_the_run_is_given_up_on() {
    let s = signals();
    let sent = [at(120), at(240), at(360)];
    assert_eq!(decide(&cfg(), &s, false, &sent, at(479)), Decision::Wait);
    assert_eq!(decide(&cfg(), &s, false, &sent, at(480)), Decision::GiveUp);
}

#[test]
fn nudges_older_than_the_window_do_not_count() {
    let s = Signals {
        mcp: Some(at(1500)),
        ..signals()
    };
    let sent = [at(120), at(240), at(360)];
    // At 1620 the nudges are 1260 s or more old, beyond the 900 s window.
    assert_eq!(decide(&cfg(), &s, false, &sent, at(1620)), Decision::Nudge);
}

#[test]
fn signals_are_told_apart_by_their_names() {
    assert_eq!(source_of("api"), Some(Source::Mcp));
    assert_eq!(source_of("hook:stop"), Some(Source::Hook));
    assert_eq!(source_of("start"), None);
    assert!(resumes("hook:post_tool_use") && resumes("hook:user_prompt_submit"));
    assert!(!resumes("hook:stop") && !resumes("hook:notification") && !resumes("api"));
}
