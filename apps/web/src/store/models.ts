// The Claude model aliases the project settings offer: the aliases of Claude Code's model
// configuration page (code.claude.com/docs/en/model-config). Any other text is passed as it is
// (a full model name); an empty setting leaves the choice to the person's Claude Code. The same
// list is `MODEL_ALIASES` in core, which the assistant's choice for a card is held to.

export const MODEL_ALIASES = ["sonnet", "opus", "haiku", "fable", "opusplan", "sonnet[1m]", "opus[1m]"];
