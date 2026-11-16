//! An epic's release node shipping inside a tag another epic made.

use toknila_core::events::DomainEvent;
use toknila_core::graph::GraphError;

use super::ChainTx;

impl ChainTx {
    /// Marks the epic's planned release released with a tag another epic made (or a person named):
    /// its Merged stories move to Released and the epic's version follows the tag when it was
    /// planned higher.
    pub fn release_with(&mut self, tag: &str, by: Option<&str>) -> Result<(), GraphError> {
        let was = self.view.chain.version.clone();
        let applied = self.step(|v, now| v.release_with(tag, by, now))?;
        self.keep_effects(applied);
        if self.view.chain.version != was {
            self.events.push(DomainEvent::ChainUpdated {
                chain: self.view.chain.clone(),
            });
        }
        Ok(())
    }
}
