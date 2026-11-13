-- name: toknila-117-lost-resume
-- when a card's agent last came back after its runner was lost (the app restarted): a second loss
-- soon after is a loop, so the card goes to a person instead of resuming again.
ALTER TABLE tickets ADD COLUMN lost_resume_at INTEGER;
