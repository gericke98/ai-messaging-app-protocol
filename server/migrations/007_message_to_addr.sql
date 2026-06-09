-- Runtime A2A (P1): record the resolved recipient on each stored message so
-- threads replay with directionality (mirrors the iOS `toAddress`, SP3 migration v9).
ALTER TABLE messages ADD COLUMN IF NOT EXISTS to_addr TEXT NULL;
