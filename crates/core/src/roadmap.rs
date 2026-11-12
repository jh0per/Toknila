//! The roadmap: the order of a project's chains (epics) and the version a new epic takes.

use crate::ChainId;
use crate::release_run::{clean_version, suggest_version};

/// Where a chain goes relative to another one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Place {
    /// Right before the chain.
    Before(ChainId),
    /// Right after the chain.
    After(ChainId),
}

/// Why a move was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum MoveError {
    /// The chain is not in the project's roadmap.
    #[error("the chain is not in this project's roadmap")]
    UnknownChain,
    /// The chain it should go next to is not in the roadmap.
    #[error("the chain to move next to is not in this project's roadmap")]
    UnknownAnchor,
    /// A chain cannot go next to itself.
    #[error("a chain cannot be moved next to itself")]
    SameChain,
}

/// The roadmap order (`order`, first to last) with `chain` taken out and put at `place`.
pub fn reorder(order: &[ChainId], chain: ChainId, place: Place) -> Result<Vec<ChainId>, MoveError> {
    let anchor = match place {
        Place::Before(a) | Place::After(a) => a,
    };
    if !order.contains(&chain) {
        return Err(MoveError::UnknownChain);
    }
    if anchor == chain {
        return Err(MoveError::SameChain);
    }
    let mut rest: Vec<ChainId> = order.iter().copied().filter(|c| *c != chain).collect();
    let at = rest
        .iter()
        .position(|c| *c == anchor)
        .ok_or(MoveError::UnknownAnchor)?;
    rest.insert(
        if matches!(place, Place::After(_)) {
            at + 1
        } else {
            at
        },
        chain,
    );
    Ok(rest)
}

/// A release version as numbers; `None` for a tag that is not a plain version or is a
/// pre-release.
fn numbers(text: &str) -> Option<Vec<u64>> {
    let v = clean_version(text).ok()?;
    if v.contains('-') {
        return None;
    }
    let mut parts: Vec<u64> = v.split('.').map(|n| n.parse().unwrap_or(0)).collect();
    while parts.len() < 3 {
        parts.push(0);
    }
    Some(parts)
}

/// The version a new epic takes: the patch after the greatest of the newest version tag (`tags`,
/// newest first) and the versions the project's chains plan, or `0.1.0` when there is none.
pub fn next_version<T: AsRef<str>, P: AsRef<str>>(tags: &[T], planned: &[P]) -> String {
    let latest = tags.iter().find(|t| numbers(t.as_ref()).is_some());
    let greatest = latest
        .map(AsRef::as_ref)
        .into_iter()
        .chain(planned.iter().map(AsRef::as_ref))
        .filter_map(|v| numbers(v).map(|n| (n, v)))
        .max_by(|a, b| a.0.cmp(&b.0))
        .map(|(_, v)| v);
    suggest_version(&greatest.into_iter().collect::<Vec<_>>())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(n: usize) -> Vec<ChainId> {
        (0..n).map(|_| ChainId::new()).collect()
    }

    #[test]
    fn a_chain_goes_before_or_after_another() {
        let o = ids(4);
        let r = reorder(&o, o[3], Place::Before(o[1])).expect("move");
        assert_eq!(r, vec![o[0], o[3], o[1], o[2]]);
        let r = reorder(&o, o[0], Place::After(o[2])).expect("move");
        assert_eq!(r, vec![o[1], o[2], o[0], o[3]]);
        let r = reorder(&o, o[0], Place::After(o[3])).expect("move");
        assert_eq!(r, vec![o[1], o[2], o[3], o[0]]);
    }

    #[test]
    fn a_move_with_a_stranger_or_itself_is_refused() {
        let o = ids(2);
        let x = ChainId::new();
        assert_eq!(
            reorder(&o, x, Place::Before(o[0])),
            Err(MoveError::UnknownChain)
        );
        assert_eq!(
            reorder(&o, o[0], Place::Before(x)),
            Err(MoveError::UnknownAnchor)
        );
        assert_eq!(
            reorder(&o, o[0], Place::After(o[0])),
            Err(MoveError::SameChain)
        );
    }

    #[test]
    fn the_next_version_follows_the_greatest_of_the_tag_and_the_plan() {
        let none: [&str; 0] = [];
        assert_eq!(next_version(&none, &none), "0.1.0");
        assert_eq!(next_version(&["v0.4.2", "v0.4.1"], &none), "0.4.3");
        assert_eq!(next_version(&["v0.4.2"], &["0.6.0", "0.5.0"]), "0.6.1");
        assert_eq!(next_version(&["v1.0.0"], &["0.9.0"]), "1.0.1");
        assert_eq!(next_version(&none, &["0.2.0"]), "0.2.1");
        // 0.10 is greater than 0.9: numbers, not text.
        assert_eq!(next_version(&["v0.9.0"], &["0.10.0"]), "0.10.1");
        assert_eq!(
            next_version(&["nightly", "v2.0.0-rc.1", "v1.1.1"], &none),
            "1.1.2"
        );
    }
}
