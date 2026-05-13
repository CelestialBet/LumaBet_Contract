-- seed_db.sql — LumaBet initial database schema
-- Run with: psql $DATABASE_URL -f scripts/seed_db.sql

-- ── Extensions ────────────────────────────────────────────────────────────────
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
CREATE EXTENSION IF NOT EXISTS "pgcrypto";

-- ── Enums ─────────────────────────────────────────────────────────────────────
DO $$
BEGIN
  IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'game_type') THEN
    CREATE TYPE game_type AS ENUM ('DICE', 'COIN_FLIP', 'SLOTS');
  END IF;
  IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'bet_status') THEN
    CREATE TYPE bet_status AS ENUM ('PENDING', 'WON', 'LOST', 'WITHDRAWN', 'EXPIRED');
  END IF;
  IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'tx_status') THEN
    CREATE TYPE tx_status AS ENUM ('PENDING', 'SUCCESS', 'FAILED');
  END IF;
END
$$;

-- ── Players ───────────────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS players (
  public_key         VARCHAR(56) PRIMARY KEY,
  display_name       VARCHAR(64),
  total_bets         INTEGER       NOT NULL DEFAULT 0,
  total_wagered_xlm  NUMERIC(20,7) NOT NULL DEFAULT 0,
  total_won_xlm      NUMERIC(20,7) NOT NULL DEFAULT 0,
  total_lost_xlm     NUMERIC(20,7) NOT NULL DEFAULT 0,
  created_at         TIMESTAMPTZ   NOT NULL DEFAULT NOW(),
  updated_at         TIMESTAMPTZ   NOT NULL DEFAULT NOW()
);

-- ── Bets ──────────────────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS bets (
  id                 UUID         PRIMARY KEY DEFAULT uuid_generate_v4(),
  player_public_key  VARCHAR(56)  NOT NULL REFERENCES players(public_key)
                                  ON DELETE RESTRICT,
  game_type          game_type    NOT NULL,
  prediction         SMALLINT     NOT NULL,
  outcome            SMALLINT,
  amount_xlm         NUMERIC(20,7) NOT NULL,
  payout_xlm         NUMERIC(20,7),
  status             bet_status   NOT NULL DEFAULT 'PENDING',
  transaction_hash   VARCHAR(64),
  resolve_tx_hash    VARCHAR(64),
  contract_id        VARCHAR(64),
  ledger_sequence    BIGINT,
  created_at         TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
  resolved_at        TIMESTAMPTZ,

  CONSTRAINT bets_amount_positive CHECK (amount_xlm > 0),
  CONSTRAINT bets_payout_non_negative CHECK (payout_xlm IS NULL OR payout_xlm >= 0)
);

CREATE INDEX IF NOT EXISTS idx_bets_player ON bets(player_public_key);
CREATE INDEX IF NOT EXISTS idx_bets_status ON bets(status);
CREATE INDEX IF NOT EXISTS idx_bets_game_type ON bets(game_type);
CREATE INDEX IF NOT EXISTS idx_bets_created_at ON bets(created_at DESC);

-- ── Transactions ──────────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS transactions (
  id                UUID        PRIMARY KEY DEFAULT uuid_generate_v4(),
  hash              VARCHAR(64) NOT NULL UNIQUE,
  type              VARCHAR(16) NOT NULL,
  player_public_key VARCHAR(56),
  amount_xlm        NUMERIC(20,7),
  status            tx_status   NOT NULL DEFAULT 'PENDING',
  ledger_sequence   BIGINT,
  fee_stroops       BIGINT,
  memo              TEXT,
  created_at        TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_tx_player ON transactions(player_public_key);
CREATE INDEX IF NOT EXISTS idx_tx_status ON transactions(status);

-- ── Trigger: auto-update players.updated_at ───────────────────────────────────
CREATE OR REPLACE FUNCTION update_updated_at()
RETURNS TRIGGER AS $$
BEGIN
  NEW.updated_at = NOW();
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS players_updated_at ON players;
CREATE TRIGGER players_updated_at
  BEFORE UPDATE ON players
  FOR EACH ROW EXECUTE FUNCTION update_updated_at();

-- ── Trigger: update player stats on bet resolution ───────────────────────────
CREATE OR REPLACE FUNCTION update_player_stats()
RETURNS TRIGGER AS $$
BEGIN
  IF NEW.status IN ('WON', 'LOST') AND OLD.status = 'PENDING' THEN
    INSERT INTO players (public_key, total_bets, total_wagered_xlm, total_won_xlm, total_lost_xlm)
    VALUES (
      NEW.player_public_key, 1, NEW.amount_xlm,
      CASE WHEN NEW.status = 'WON' THEN COALESCE(NEW.payout_xlm, 0) ELSE 0 END,
      CASE WHEN NEW.status = 'LOST' THEN NEW.amount_xlm ELSE 0 END
    )
    ON CONFLICT (public_key) DO UPDATE SET
      total_bets        = players.total_bets + 1,
      total_wagered_xlm = players.total_wagered_xlm + NEW.amount_xlm,
      total_won_xlm     = players.total_won_xlm +
                          CASE WHEN NEW.status = 'WON' THEN COALESCE(NEW.payout_xlm, 0) ELSE 0 END,
      total_lost_xlm    = players.total_lost_xlm +
                          CASE WHEN NEW.status = 'LOST' THEN NEW.amount_xlm ELSE 0 END;
  END IF;
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS bet_resolved_stats ON bets;
CREATE TRIGGER bet_resolved_stats
  AFTER UPDATE ON bets
  FOR EACH ROW EXECUTE FUNCTION update_player_stats();

-- ── Seed data (testnet demo players) ─────────────────────────────────────────
INSERT INTO players (public_key, display_name) VALUES
  ('GAAZI4TCR3TY5OJHCTJC2A4QSY6CJWJH5IAJTGKIN2ER7LBNVKOCCWN', 'Demo Player 1'),
  ('GBDVKE33GNUQE5QKPGBLZWLHPQJHYNKBATBMH3KAQCPBZ7V4L3SJA5E', 'Demo Player 2')
ON CONFLICT DO NOTHING;
