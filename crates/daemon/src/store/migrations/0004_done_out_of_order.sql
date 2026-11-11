-- name: legacy-04-done-out-of-order
-- cards imported as done behind open cards: the earlier cards that were open
-- (JSON), kept for good so a view can say "done out of order".
ALTER TABLE tickets ADD COLUMN done_out_of_order_json TEXT;
